//! Durable gateway admission with explicit recorded, pending and refused outcomes.
use std::{
    fmt,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use adoc_core::{
    GatewaySensitiveAccessCommand, GatewaySensitiveAccessInput, ReadAccess, RetrievalPolicy,
    build_gateway_sensitive_access, gateway_policy_digest,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{
    McpAdapterError, McpAdapterResult,
    gateway_spool::{Binding, Header, Pending, Spool},
};

const RESPONSE_LIMIT: u64 = 64 * 1024;
const MAX_SEQUENCE: u64 = 9_007_199_254_740_991;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuditConfig {
    cloud_url: String,
    workspace_id: String,
    repository_id: String,
    bearer_token_file: PathBuf,
    #[serde(default)]
    delivery_policy: DeliveryPolicy,
}

#[derive(Serialize)]
struct SetupRequest {
    repository_id: String,
    setup_request_id: String,
    local_policy_digest: String,
}

#[derive(Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Transmission {
    audit_metadata: bool,
    egress_policy_digest: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupResponse {
    gateway_session_id: String,
    workspace_id: String,
    repository_id: String,
    principal_id: String,
    auth_session_id: String,
    local_policy_digest: String,
    transmission: Transmission,
}

impl SetupResponse {
    fn same_binding(&self, other: &Self) -> bool {
        self.gateway_session_id == other.gateway_session_id
            && self.workspace_id == other.workspace_id
            && self.repository_id == other.repository_id
            && self.principal_id == other.principal_id
            && self.auth_session_id == other.auth_session_id
            && self.local_policy_digest == other.local_policy_digest
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    event_id: String,
    event_digest: String,
    gateway_session_id: String,
    sequence: u64,
    recorded: bool,
    replayed: bool,
}

pub(crate) enum Delivery {
    Recorded { event_id: String },
    SpooledPending { event_id: String },
}

#[derive(Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum DeliveryPolicy {
    #[default]
    Spool,
    Synchronous,
}

pub(crate) struct GatewayAuditor {
    pub(crate) root: PathBuf,
    agent: ureq::Agent,
    endpoint: String,
    token_path: PathBuf,
    policy: DeliveryPolicy,
    state: Mutex<Spool>,
}

// Credentials and upstream responses deliberately have no Debug representation.
impl fmt::Debug for GatewayAuditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GatewayAuditor { .. }")
    }
}

fn unavailable<T>() -> McpAdapterResult<T> {
    Err(McpAdapterError::AuditSinkUnavailable)
}

fn canonical_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}

fn digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn origin(value: &str) -> McpAdapterResult<String> {
    let uri: ureq::http::Uri = value
        .parse()
        .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
    let host = uri.host().ok_or(McpAdapterError::AuditSinkUnavailable)?;
    let loopback = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<std::net::IpAddr>()
        .is_ok_and(|ip| ip.is_loopback());
    if value.contains(['@', '?', '#'])
        || uri.authority().is_none()
        || !matches!(uri.path(), "" | "/")
        || !(uri.scheme_str() == Some("https") || (uri.scheme_str() == Some("http") && loopback))
    {
        return unavailable();
    }
    Ok(value.trim_end_matches('/').to_string())
}

fn read_bounded(path: &Path, limit: u64) -> McpAdapterResult<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(limit + 1).read_to_end(&mut bytes))
        .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
    if bytes.len() as u64 > limit {
        return unavailable();
    }
    Ok(bytes)
}

impl GatewayAuditor {
    pub(crate) fn open(
        root: &Path,
        policy: &RetrievalPolicy,
        path: &Path,
    ) -> McpAdapterResult<Self> {
        let config: AuditConfig = decode_object(&read_bounded(path, RESPONSE_LIMIT)?)?;
        if !canonical_uuid(&config.workspace_id) || !canonical_uuid(&config.repository_id) {
            return unavailable();
        }
        let cloud_origin = origin(&config.cloud_url)?;
        let root = root
            .canonicalize()
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        let header = Header {
            format_version: 1,
            root: root
                .to_str()
                .ok_or(McpAdapterError::AuditSinkUnavailable)?
                .to_string(),
            cloud_origin: cloud_origin.clone(),
            workspace_id: config.workspace_id.clone(),
            repository_id: config.repository_id,
            local_policy_digest: gateway_policy_digest(policy)
                .map_err(|_| McpAdapterError::AuditSinkUnavailable)?,
            setup_request_id: Uuid::new_v4().to_string(),
        };
        let token_path = if config.bearer_token_file.is_absolute() {
            config.bearer_token_file
        } else {
            path.parent()
                .unwrap_or(Path::new("."))
                .join(config.bearer_token_file)
        };
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .max_redirects(0)
            .proxy(None)
            .timeout_global(Some(Duration::from_secs(10)))
            .max_response_header_size(RESPONSE_LIMIT as usize)
            .build()
            .into();
        let spool = Spool::open(header)?;
        Ok(Self {
            root,
            agent,
            endpoint: format!(
                "{cloud_origin}/api/v1/workspaces/{}/gateway-audit-sessions",
                config.workspace_id
            ),
            token_path,
            policy: config.delivery_policy,
            state: Mutex::new(spool),
        })
    }

    fn authorization(&self) -> McpAdapterResult<ureq::http::HeaderValue> {
        let token = String::from_utf8(read_bounded(&self.token_path, 16 * 1024)?)
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        let token = token.trim_end_matches(['\r', '\n']);
        if token.is_empty() || !token.bytes().all(|b| b.is_ascii_graphic()) {
            return unavailable();
        }
        let mut value = ureq::http::HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        value.set_sensitive(true);
        Ok(value)
    }

    pub(crate) fn check_integrity(&self) -> McpAdapterResult<()> {
        self.state
            .lock()
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?
            .read()
            .map(|_| ())
    }

    pub(crate) fn drain_pending(&self) -> McpAdapterResult<()> {
        let mut spool = self
            .state
            .lock()
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        self.drain(&mut spool).map(|_| ())
    }

    fn current(&self, spool: &mut Spool) -> McpAdapterResult<SetupResponse> {
        let header = spool.read()?.header;
        let setup = SetupRequest {
            repository_id: header.repository_id.clone(),
            setup_request_id: header.setup_request_id.clone(),
            local_policy_digest: header.local_policy_digest.clone(),
        };
        let fresh: SetupResponse = post(
            &self.agent,
            &self.authorization()?,
            &self.endpoint,
            &serde_json::to_vec(&setup)?,
            None,
        )
        .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        if !valid_binding(&fresh, &header) {
            return unavailable();
        }
        spool.bind(Binding {
            gateway_session_id: fresh.gateway_session_id.clone(),
            principal_id: fresh.principal_id.clone(),
            auth_session_id: fresh.auth_session_id.clone(),
        })?;
        Ok(fresh)
    }

    pub(crate) fn record(
        &self,
        command: GatewaySensitiveAccessCommand,
        access: &ReadAccess,
    ) -> McpAdapterResult<Delivery> {
        // ponytail: one serialized journal; partition only if measured throughput requires it.
        let mut spool = self
            .state
            .lock()
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        let current = self.current(&mut spool)?;
        // An outage may leave older entries queued. A denial is never pending.
        let drained = self.drain(&mut spool)?;
        let state = spool.read()?;
        let sequence = state
            .sequences
            .get(&current.gateway_session_id)
            .copied()
            .unwrap_or(0)
            + 1;
        if sequence > MAX_SEQUENCE {
            return unavailable();
        }
        let event_id = Uuid::new_v4().to_string();
        let event = build_gateway_sensitive_access(GatewaySensitiveAccessInput {
            event_id: event_id.clone(),
            workspace_id: current.workspace_id.clone(),
            repository_id: current.repository_id.clone(),
            principal_id: current.principal_id.clone(),
            auth_session_id: current.auth_session_id.clone(),
            gateway_session_id: current.gateway_session_id.clone(),
            local_policy_digest: current.local_policy_digest.clone(),
            command,
            sequence,
            objects: access.sensitive_objects(),
        })
        .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        spool.enqueue(&event)?;
        if drained && self.drain(&mut spool)? {
            return Ok(Delivery::Recorded { event_id });
        }
        if self.policy == DeliveryPolicy::Synchronous {
            return unavailable();
        }
        // A recording outage never promotes the earlier preflight into a cached grant.
        let final_binding = self.current(&mut spool)?;
        if !current.same_binding(&final_binding) {
            return unavailable();
        }
        Ok(Delivery::SpooledPending { event_id })
    }

    /// False means only recording/ack transport is unavailable; every other failure refuses.
    fn drain(&self, spool: &mut Spool) -> McpAdapterResult<bool> {
        loop {
            let state = spool.read()?;
            let Some(pending) = state.pending.first() else {
                return Ok(true);
            };
            let binding = state
                .bindings
                .get(pending.event.gateway_session_id())
                .ok_or(McpAdapterError::AuditSpoolCorrupt)?;
            match self.transmit(&state.header, binding, pending) {
                Ok(()) => spool.acknowledge(pending)?,
                Err(PostFailure::Outage) => return Ok(false),
                Err(PostFailure::Refused) => return unavailable(),
            }
        }
    }

    fn transmit(
        &self,
        header: &Header,
        binding: &Binding,
        pending: &Pending,
    ) -> Result<(), PostFailure> {
        let authorization = self.authorization().map_err(|_| PostFailure::Refused)?;
        let request = serde_json::json!({"repository_id": header.repository_id, "local_policy_digest": header.local_policy_digest});
        let fresh: SetupResponse = post(
            &self.agent,
            &authorization,
            &format!(
                "{}/{}/transmission",
                self.endpoint, binding.gateway_session_id
            ),
            &serde_json::to_vec(&request).map_err(|_| PostFailure::Refused)?,
            None,
        )
        .map_err(|_| PostFailure::Refused)?;
        if !valid_binding(&fresh, header)
            || fresh.gateway_session_id != binding.gateway_session_id
            || fresh.principal_id != binding.principal_id
            || fresh.auth_session_id != binding.auth_session_id
        {
            return Err(PostFailure::Refused);
        }
        let receipt: Receipt = post(
            &self.agent,
            &authorization,
            &format!("{}/{}/events", self.endpoint, binding.gateway_session_id),
            pending.bytes.as_bytes(),
            Some(&fresh.transmission.egress_policy_digest),
        )?;
        if !receipt.recorded
            || receipt.event_id != pending.event.event_id()
            || receipt.event_digest != pending.digest
            || receipt.gateway_session_id != binding.gateway_session_id
            || receipt.sequence != pending.event.sequence()
        {
            return Err(PostFailure::Refused);
        }
        let _ = receipt.replayed;
        Ok(())
    }
}

fn valid_binding(binding: &SetupResponse, header: &Header) -> bool {
    binding.workspace_id == header.workspace_id
        && binding.repository_id == header.repository_id
        && binding.local_policy_digest == header.local_policy_digest
        && [
            &binding.gateway_session_id,
            &binding.principal_id,
            &binding.auth_session_id,
        ]
        .into_iter()
        .all(|s| canonical_uuid(s))
        && binding.transmission.audit_metadata
        && digest(&binding.transmission.egress_policy_digest)
}

#[derive(Debug)]
enum PostFailure {
    Outage,
    Refused,
}
fn post_failure(error: ureq::Error) -> PostFailure {
    match error {
        ureq::Error::StatusCode(500..=599)
        | ureq::Error::Io(_)
        | ureq::Error::Timeout(_)
        | ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed => PostFailure::Outage,
        _ => PostFailure::Refused,
    }
}

fn post<T: DeserializeOwned>(
    agent: &ureq::Agent,
    authorization: &ureq::http::HeaderValue,
    url: &str,
    bytes: &[u8],
    egress: Option<&str>,
) -> Result<T, PostFailure> {
    let mut request = agent
        .post(url)
        .header("authorization", authorization.clone())
        .header("content-type", "application/json");
    if let Some(digest) = egress {
        request = request.header("x-agentdoc-egress-policy-digest", digest);
    }
    let mut response = request.send(bytes).map_err(post_failure)?;
    if !response.status().is_success() {
        return Err(PostFailure::Refused);
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(RESPONSE_LIMIT)
        .read_to_vec()
        .map_err(post_failure)?;
    decode_object(&bytes).map_err(|_| PostFailure::Refused)
}

fn decode_object<T: DeserializeOwned>(bytes: &[u8]) -> McpAdapterResult<T> {
    let shape: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
    if !shape.is_object()
        || shape
            .get("transmission")
            .is_some_and(|value| !value.is_object())
    {
        return unavailable();
    }
    // Deserialize original bytes to reject duplicate fields as well as unknown fields.
    serde_json::from_slice(bytes).map_err(|_| McpAdapterError::AuditSinkUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_prepares_durable_spool_without_network_authority() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("audit.json");
        std::fs::write(root.path().join("token"), "valid-token").unwrap();
        std::fs::write(
            &config,
            serde_json::json!({
                "cloud_url":"http://127.0.0.1:9",
                "workspace_id":"00000000-0000-4000-8000-000000000001",
                "repository_id":"00000000-0000-4000-8000-000000000002",
                "bearer_token_file":"token"
            })
            .to_string(),
        )
        .unwrap();
        let auditor = GatewayAuditor::open(
            root.path(),
            &RetrievalPolicy {
                audience: "public".into(),
                allowed_visibilities: ["public".into()].into(),
                excluded_object_ids: Default::default(),
            },
            &config,
        );
        assert!(
            auditor.is_ok(),
            "local startup must persist setup without granting authority"
        );
        let bytes = std::fs::read(root.path().join(".adoc-audit/spool.log")).unwrap();
        assert!(bytes.ends_with(b"\n"));
        assert!(!String::from_utf8(bytes).unwrap().contains("valid-token"));
    }

    #[test]
    fn startup_and_receipts_reject_arrays_unknown_keys_and_duplicate_fields() {
        for bytes in [
            br#"["https://cloud.example","workspace","repository","token"]"#.as_slice(),
            br#"{"cloud_url":"https://cloud.example","workspace_id":"workspace","repository_id":"repository","bearer_token_file":"token","principal_id":"spoof"}"#,
            br#"{"cloud_url":"https://cloud.example","cloud_url":"https://other.example","workspace_id":"workspace","repository_id":"repository","bearer_token_file":"token"}"#,
        ] { assert!(decode_object::<AuditConfig>(bytes).is_err()); }
        assert!(decode_object::<Receipt>(br#"["event","digest","session",1,true,false]"#).is_err());
    }

    #[test]
    fn trusted_origin_is_https_or_literal_loopback_and_never_contains_secrets_or_routes() {
        for url in [
            "https://cloud.example",
            "https://cloud.example/",
            "http://127.0.0.1:8080",
            "http://[::1]:8080",
        ] {
            assert!(origin(url).is_ok(), "{url}");
        }
        for url in [
            "http://cloud.example",
            "http://localhost",
            "https://user:secret@cloud.example",
            "https://cloud.example/path",
            "https://cloud.example?secret",
            "https://cloud.example#secret",
            "file:///tmp/token",
        ] {
            assert!(origin(url).is_err(), "{url}");
        }
    }
}
