//! Connected gateway admission. Never release sensitive bytes without a matching receipt.
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

use crate::{McpAdapterError, McpAdapterResult};

const RESPONSE_LIMIT: u64 = 64 * 1024;
const MAX_SEQUENCE: u64 = 9_007_199_254_740_991;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuditConfig {
    cloud_url: String,
    workspace_id: String,
    repository_id: String,
    bearer_token_file: PathBuf,
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

struct PendingEvent {
    event_id: String,
    digest: String,
    bytes: String,
}

struct RecordingState {
    sequence: u64,
    pending: Option<PendingEvent>,
}

pub(crate) struct GatewayAuditor {
    pub(crate) root: PathBuf,
    agent: ureq::Agent,
    endpoint: String,
    authorization: ureq::http::HeaderValue,
    setup: SetupRequest,
    binding: SetupResponse,
    state: Mutex<RecordingState>,
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
        let endpoint = format!(
            "{}/api/v1/workspaces/{}/gateway-audit-sessions",
            origin(&config.cloud_url)?,
            config.workspace_id
        );
        let token_path = if config.bearer_token_file.is_absolute() {
            config.bearer_token_file
        } else {
            path.parent()
                .unwrap_or(Path::new("."))
                .join(config.bearer_token_file)
        };
        let token = String::from_utf8(read_bounded(&token_path, 16 * 1024)?)
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        let token = token.trim_end_matches(['\r', '\n']);
        if token.is_empty() || !token.bytes().all(|b| b.is_ascii_graphic()) {
            return unavailable();
        }
        let mut authorization = ureq::http::HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        authorization.set_sensitive(true);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .max_redirects(0)
            .proxy(None)
            .timeout_global(Some(Duration::from_secs(10)))
            .max_response_header_size(RESPONSE_LIMIT as usize)
            .build()
            .into();
        let setup = SetupRequest {
            repository_id: config.repository_id,
            setup_request_id: Uuid::new_v4().to_string(),
            local_policy_digest: gateway_policy_digest(policy)
                .map_err(|_| McpAdapterError::AuditSinkUnavailable)?,
        };
        let binding: SetupResponse = post(
            &agent,
            &authorization,
            &endpoint,
            &serde_json::to_vec(&setup)?,
            None,
        )?;
        if binding.workspace_id != config.workspace_id
            || binding.repository_id != setup.repository_id
            || binding.local_policy_digest != setup.local_policy_digest
            || ![
                &binding.gateway_session_id,
                &binding.principal_id,
                &binding.auth_session_id,
            ]
            .into_iter()
            .all(|s| canonical_uuid(s))
            || !binding.transmission.audit_metadata
            || !digest(&binding.transmission.egress_policy_digest)
        {
            return unavailable();
        }
        Ok(Self {
            root: root
                .canonicalize()
                .map_err(|_| McpAdapterError::AuditSinkUnavailable)?,
            agent,
            endpoint,
            authorization,
            setup,
            binding,
            state: Mutex::new(RecordingState {
                sequence: 1,
                pending: None,
            }),
        })
    }

    pub(crate) fn record(
        &self,
        command: GatewaySensitiveAccessCommand,
        access: &ReadAccess,
    ) -> McpAdapterResult<()> {
        // ponytail: serialize connected admission; per-session queues only if measured throughput requires them.
        let mut state = self
            .state
            .lock()
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        if state.pending.is_some() {
            self.transmit(&mut state)?;
        }
        if state.sequence > MAX_SEQUENCE {
            return unavailable();
        }
        let event_id = Uuid::new_v4().to_string();
        let event = build_gateway_sensitive_access(GatewaySensitiveAccessInput {
            event_id: event_id.clone(),
            workspace_id: self.binding.workspace_id.clone(),
            repository_id: self.binding.repository_id.clone(),
            principal_id: self.binding.principal_id.clone(),
            auth_session_id: self.binding.auth_session_id.clone(),
            gateway_session_id: self.binding.gateway_session_id.clone(),
            local_policy_digest: self.setup.local_policy_digest.clone(),
            command,
            sequence: state.sequence,
            objects: access.sensitive_objects(),
        })
        .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        state.pending = Some(PendingEvent {
            event_id,
            digest: event
                .content_digest()
                .map_err(|_| McpAdapterError::AuditSinkUnavailable)?,
            bytes: event
                .to_canonical_json()
                .map_err(|_| McpAdapterError::AuditSinkUnavailable)?,
        });
        self.transmit(&mut state)
    }

    fn transmit(&self, state: &mut RecordingState) -> McpAdapterResult<()> {
        // Setup replay is a fresh preflight, never a cached transmission permit.
        let fresh: SetupResponse = post(
            &self.agent,
            &self.authorization,
            &self.endpoint,
            &serde_json::to_vec(&self.setup)?,
            None,
        )?;
        if !self.binding.same_binding(&fresh)
            || !fresh.transmission.audit_metadata
            || !digest(&fresh.transmission.egress_policy_digest)
        {
            return unavailable();
        }
        let pending = state
            .pending
            .as_ref()
            .ok_or(McpAdapterError::AuditSinkUnavailable)?;
        let receipt: Receipt = post(
            &self.agent,
            &self.authorization,
            &format!(
                "{}/{}/events",
                self.endpoint, self.binding.gateway_session_id
            ),
            pending.bytes.as_bytes(),
            Some(&fresh.transmission.egress_policy_digest),
        )?;
        if !receipt.recorded
            || receipt.event_id != pending.event_id
            || receipt.event_digest != pending.digest
            || receipt.gateway_session_id != self.binding.gateway_session_id
            || receipt.sequence != state.sequence
        {
            return unavailable();
        }
        // Both fresh recording and exact replay acknowledge this same event.
        let _ = receipt.replayed;
        state.pending = None;
        state.sequence += 1;
        Ok(())
    }
}

fn post<T: DeserializeOwned>(
    agent: &ureq::Agent,
    authorization: &ureq::http::HeaderValue,
    url: &str,
    bytes: &[u8],
    egress: Option<&str>,
) -> McpAdapterResult<T> {
    let mut request = agent
        .post(url)
        .header("authorization", authorization.clone())
        .header("content-type", "application/json");
    if let Some(digest) = egress {
        request = request.header("x-agentdoc-egress-policy-digest", digest);
    }
    let mut response = request
        .send(bytes)
        .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
    if !response.status().is_success() {
        return unavailable();
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(RESPONSE_LIMIT)
        .read_to_vec()
        .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
    decode_object(&bytes)
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
