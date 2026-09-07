//! Append-only gateway metadata journal. No response content or credentials are stored.
use crate::{McpAdapterError, McpAdapterResult};
use adoc_core::{GatewaySensitiveAccessEvent, validate_gateway_sensitive_access};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_RECORD: u64 = 8 * 1024 * 1024;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Header {
    pub format_version: u8,
    pub root: String,
    pub cloud_origin: String,
    pub workspace_id: String,
    pub repository_id: String,
    pub local_policy_digest: String,
    pub setup_request_id: String,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub gateway_session_id: String,
    pub principal_id: String,
    pub auth_session_id: String,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Frame {
    Header(Header),
    Binding(Binding),
    Event {
        event_bytes: String,
        event_digest: String,
    },
    Ack {
        gateway_session_id: String,
        event_id: String,
        event_digest: String,
        sequence: u64,
    },
}
#[derive(Clone)]
pub(crate) struct Pending {
    pub event: GatewaySensitiveAccessEvent,
    pub bytes: String,
    pub digest: String,
}
pub(crate) struct State {
    pub header: Header,
    pub bindings: BTreeMap<String, Binding>,
    pub sequences: BTreeMap<String, u64>,
    pub pending: Vec<Pending>,
}
fn corrupt<T>() -> McpAdapterResult<T> {
    Err(McpAdapterError::AuditSpoolCorrupt)
}
fn unavailable<T>() -> McpAdapterResult<T> {
    Err(McpAdapterError::AuditSinkUnavailable)
}
fn canonical_uuid(s: &str) -> bool {
    Uuid::parse_str(s).is_ok_and(|v| v.to_string() == s)
}

impl State {
    fn read(file: &mut File) -> McpAdapterResult<Self> {
        file.seek(SeekFrom::Start(0))
            .map_err(|_| McpAdapterError::AuditSpoolCorrupt)?;
        let mut reader = BufReader::new(file);
        let mut state: Option<Self> = None;
        let mut ids = BTreeSet::new();
        loop {
            let mut line = Vec::new();
            let n = reader
                .by_ref()
                .take(MAX_RECORD + 1)
                .read_until(b'\n', &mut line)
                .map_err(|_| McpAdapterError::AuditSpoolCorrupt)?;
            if n == 0 {
                break;
            }
            if n as u64 > MAX_RECORD || line.last() != Some(&b'\n') {
                return corrupt();
            }
            let shape: serde_json::Value =
                serde_json::from_slice(&line).map_err(|_| McpAdapterError::AuditSpoolCorrupt)?;
            if !shape.is_object() {
                return corrupt();
            }
            let frame: Frame =
                serde_json::from_slice(&line).map_err(|_| McpAdapterError::AuditSpoolCorrupt)?;
            match (&mut state, frame) {
                (None, Frame::Header(header))
                    if header.format_version == 1 && canonical_uuid(&header.setup_request_id) =>
                {
                    state = Some(Self {
                        header,
                        bindings: BTreeMap::new(),
                        sequences: BTreeMap::new(),
                        pending: Vec::new(),
                    });
                }
                (Some(s), Frame::Binding(b)) => {
                    if [&b.gateway_session_id, &b.principal_id, &b.auth_session_id]
                        .iter()
                        .any(|v| !canonical_uuid(v))
                        || s.bindings.contains_key(&b.gateway_session_id)
                        || s.bindings.values().any(|old| {
                            old.principal_id != b.principal_id
                                || old.auth_session_id == b.auth_session_id
                        })
                    {
                        return corrupt();
                    }
                    s.sequences.insert(b.gateway_session_id.clone(), 0);
                    s.bindings.insert(b.gateway_session_id.clone(), b);
                }
                (
                    Some(s),
                    Frame::Event {
                        event_bytes,
                        event_digest,
                    },
                ) => {
                    let event = validate_gateway_sensitive_access(event_bytes.as_bytes())
                        .map_err(|_| McpAdapterError::AuditSpoolCorrupt)?;
                    let Some(binding) = s.bindings.get(event.gateway_session_id()) else {
                        return corrupt();
                    };
                    if event.workspace_id() != s.header.workspace_id
                        || event.repository_id() != s.header.repository_id
                        || event.local_policy_digest() != s.header.local_policy_digest
                        || event.principal_id() != binding.principal_id
                        || event.auth_session_id() != binding.auth_session_id
                        || event
                            .to_canonical_json()
                            .map_err(|_| McpAdapterError::AuditSpoolCorrupt)?
                            != event_bytes
                        || event
                            .content_digest()
                            .map_err(|_| McpAdapterError::AuditSpoolCorrupt)?
                            != event_digest
                        || !ids.insert(event.event_id().to_string())
                        || s.sequences
                            .get(event.gateway_session_id())
                            .copied()
                            .unwrap_or(0)
                            + 1
                            != event.sequence()
                    {
                        return corrupt();
                    }
                    s.sequences
                        .insert(event.gateway_session_id().to_string(), event.sequence());
                    s.pending.push(Pending {
                        event,
                        bytes: event_bytes,
                        digest: event_digest,
                    });
                }
                (
                    Some(s),
                    Frame::Ack {
                        gateway_session_id,
                        event_id,
                        event_digest,
                        sequence,
                    },
                ) => {
                    let Some(first) = s.pending.first() else {
                        return corrupt();
                    };
                    if first.event.gateway_session_id() != gateway_session_id
                        || first.event.event_id() != event_id
                        || first.digest != event_digest
                        || first.event.sequence() != sequence
                    {
                        return corrupt();
                    }
                    s.pending.remove(0);
                }
                _ => return corrupt(),
            }
        }
        state.ok_or(McpAdapterError::AuditSpoolCorrupt)
    }
}

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};

/// Held descriptor locks survive path lookups; the private sandbox is operator controlled.
/// Path revalidation does not promise protection from hostile same-UID rename races.
pub(crate) struct Spool {
    root: PathBuf,
    directory: PathBuf,
    path: PathBuf,
    root_file: File,
    directory_file: File,
    file: File,
    header: Header,
    poisoned: bool,
}
impl Spool {
    #[cfg(unix)]
    pub fn open(mut header: Header) -> McpAdapterResult<Self> {
        use adoc_local::{PathPolicy, ProjectRootPathPolicy};
        let root = PathBuf::from(&header.root);
        let policy = ProjectRootPathPolicy::new(root.clone())
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        let directory = root.join(".adoc-audit");
        let path = directory.join("spool.log");
        policy
            .resolve_write_path(Path::new(".adoc-audit/spool.log"))
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        let root_file = File::open(&root).map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        let root_meta = root_file
            .metadata()
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        let created_dir = match fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(_) => return unavailable(),
        };
        let dm =
            fs::symlink_metadata(&directory).map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        if !dm.is_dir() || dm.mode() & 0o077 != 0 || dm.uid() != root_meta.uid() {
            return unavailable();
        }
        let directory_file =
            File::open(&directory).map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        if !same_inode(
            &dm,
            &directory_file
                .metadata()
                .map_err(|_| McpAdapterError::AuditSinkUnavailable)?,
        ) {
            return unavailable();
        }
        if created_dir {
            sync_durable(&root_file, true).map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        }
        let ignore = directory.join(".gitignore");
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&ignore)
        {
            Ok(mut f) => {
                f.write_all(b"*\n")
                    .and_then(|_| sync_durable(&f, false))
                    .and_then(|_| sync_durable(&directory_file, true))
                    .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let m = fs::symlink_metadata(&ignore)
                    .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
                if !m.is_file()
                    || m.nlink() != 1
                    || m.mode() & 0o077 != 0
                    || m.uid() != root_meta.uid()
                {
                    return unavailable();
                }
                let f = File::open(&ignore).map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
                if !same_inode(
                    &m,
                    &f.metadata()
                        .map_err(|_| McpAdapterError::AuditSinkUnavailable)?,
                ) {
                    return unavailable();
                }
                let mut contents = Vec::new();
                f.take(4)
                    .read_to_end(&mut contents)
                    .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
                if contents != b"*\n" {
                    return unavailable();
                }
            }
            Err(_) => return unavailable(),
        }
        let (file, created) = match OpenOptions::new()
            .read(true)
            .append(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(f) => (f, true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let m = fs::symlink_metadata(&path)
                    .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
                if !m.is_file()
                    || m.nlink() != 1
                    || m.mode() & 0o077 != 0
                    || m.uid() != root_meta.uid()
                {
                    return unavailable();
                }
                let f = OpenOptions::new()
                    .read(true)
                    .append(true)
                    .open(&path)
                    .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
                if !same_inode(
                    &m,
                    &f.metadata()
                        .map_err(|_| McpAdapterError::AuditSinkUnavailable)?,
                ) {
                    return unavailable();
                }
                (f, false)
            }
            Err(_) => return unavailable(),
        };
        file.try_lock()
            .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        let mut spool = Self {
            root,
            directory,
            path,
            root_file,
            directory_file,
            file,
            header: header.clone(),
            poisoned: false,
        };
        spool.verify_paths()?;
        if created {
            spool.append(&Frame::Header(header))?;
            sync_durable(&spool.directory_file, true)
                .map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        } else {
            let state = State::read(&mut spool.file)?;
            header.setup_request_id = state.header.setup_request_id.clone();
            if header != state.header {
                return unavailable();
            }
            spool.header = header;
        }
        Ok(spool)
    }
    #[cfg(not(unix))]
    pub fn open(_header: Header) -> McpAdapterResult<Self> {
        unavailable()
    }

    #[cfg(unix)]
    fn verify_paths(&self) -> McpAdapterResult<()> {
        let root_meta = self
            .root_file
            .metadata()
            .map_err(|_| McpAdapterError::AuditSpoolCorrupt)?;
        for (path, handle, directory) in [
            (&self.root, &self.root_file, true),
            (&self.directory, &self.directory_file, true),
            (&self.path, &self.file, false),
        ] {
            let m = fs::symlink_metadata(path).map_err(|_| McpAdapterError::AuditSpoolCorrupt)?;
            let h = handle
                .metadata()
                .map_err(|_| McpAdapterError::AuditSpoolCorrupt)?;
            if !same_inode(&m, &h)
                || (directory && !m.is_dir())
                || (!directory && (!m.is_file() || m.nlink() != 1))
                || (path != &self.root && (m.mode() & 0o077 != 0 || m.uid() != root_meta.uid()))
                || path
                    .canonicalize()
                    .map_err(|_| McpAdapterError::AuditSpoolCorrupt)?
                    != *path
            {
                return corrupt();
            }
        }
        Ok(())
    }
    #[cfg(not(unix))]
    fn verify_paths(&self) -> McpAdapterResult<()> {
        unavailable()
    }

    pub fn read(&mut self) -> McpAdapterResult<State> {
        if self.poisoned {
            return corrupt();
        }
        let result = self
            .verify_paths()
            .and_then(|_| State::read(&mut self.file));
        match result {
            Ok(state) if state.header == self.header => Ok(state),
            _ => {
                self.poisoned = true;
                corrupt()
            }
        }
    }
    fn append(&mut self, frame: &Frame) -> McpAdapterResult<()> {
        if self.poisoned {
            return corrupt();
        }
        self.verify_paths()?;
        let mut bytes =
            serde_json::to_vec(frame).map_err(|_| McpAdapterError::AuditSinkUnavailable)?;
        bytes.push(b'\n');
        if bytes.len() as u64 > MAX_RECORD {
            return unavailable();
        }
        if append_bytes(&mut self.file, &bytes)
            .and_then(|_| sync_durable(&self.file, false))
            .is_err()
        {
            self.poisoned = true;
            return unavailable();
        }
        Ok(())
    }
    pub fn bind(&mut self, binding: Binding) -> McpAdapterResult<()> {
        let state = self.read()?;
        if let Some(existing) = state.bindings.get(&binding.gateway_session_id) {
            return if existing == &binding {
                Ok(())
            } else {
                unavailable()
            };
        }
        if state.bindings.values().any(|old| {
            old.principal_id != binding.principal_id
                || old.auth_session_id == binding.auth_session_id
        }) {
            return unavailable();
        }
        self.append(&Frame::Binding(binding))
    }
    pub fn enqueue(&mut self, event: &GatewaySensitiveAccessEvent) -> McpAdapterResult<()> {
        self.append(&Frame::Event {
            event_bytes: event
                .to_canonical_json()
                .map_err(|_| McpAdapterError::AuditSinkUnavailable)?,
            event_digest: event
                .content_digest()
                .map_err(|_| McpAdapterError::AuditSinkUnavailable)?,
        })
    }
    pub fn acknowledge(&mut self, pending: &Pending) -> McpAdapterResult<()> {
        self.append(&Frame::Ack {
            gateway_session_id: pending.event.gateway_session_id().into(),
            event_id: pending.event.event_id().into(),
            event_digest: pending.digest.clone(),
            sequence: pending.event.sequence(),
        })
    }
}
#[cfg(unix)]
fn same_inode(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}

fn sync_durable(file: &File, directory: bool) -> std::io::Result<()> {
    #[cfg(test)]
    if take_fault(if directory {
        Fault::DirectorySync
    } else {
        Fault::FileSync
    }) {
        return Err(std::io::Error::other("injected durable sync failure"));
    }
    #[cfg(not(test))]
    let _ = directory;
    file.sync_all()
}
fn append_bytes(file: &mut File, bytes: &[u8]) -> std::io::Result<()> {
    #[cfg(test)]
    if take_fault(Fault::PartialWrite) {
        file.write_all(&bytes[..bytes.len() / 2])?;
        return Err(std::io::Error::other("injected partial append"));
    }
    file.write_all(bytes)
}
#[cfg(test)]
#[derive(Clone, Copy, PartialEq)]
enum Fault {
    FileSync,
    DirectorySync,
    PartialWrite,
}
#[cfg(test)]
thread_local! { static IO_FAULT: std::cell::Cell<Option<Fault>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
fn take_fault(point: Fault) -> bool {
    IO_FAULT.with(|fault| {
        if fault.get() == Some(point) {
            fault.set(None);
            true
        } else {
            false
        }
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use adoc_core::{
        GatewaySensitiveAccessCommand, GatewaySensitiveAccessInput, GatewaySensitiveAccessObject,
        SensitiveClassification, build_gateway_sensitive_access,
    };
    fn header(root: &Path) -> Header {
        Header {
            format_version: 1,
            root: root.canonicalize().unwrap().to_str().unwrap().into(),
            cloud_origin: "https://cloud.example".into(),
            workspace_id: "00000000-0000-4000-8000-000000000001".into(),
            repository_id: "00000000-0000-4000-8000-000000000002".into(),
            local_policy_digest: format!("sha256:{}", "a".repeat(64)),
            setup_request_id: Uuid::new_v4().to_string(),
        }
    }
    fn binding() -> Binding {
        Binding {
            gateway_session_id: "00000000-0000-4000-8000-000000000003".into(),
            principal_id: "00000000-0000-4000-8000-000000000004".into(),
            auth_session_id: "00000000-0000-4000-8000-000000000005".into(),
        }
    }
    fn event(header: &Header, binding: &Binding, sequence: u64) -> GatewaySensitiveAccessEvent {
        build_gateway_sensitive_access(GatewaySensitiveAccessInput {
            event_id: Uuid::new_v4().to_string(),
            workspace_id: header.workspace_id.clone(),
            repository_id: header.repository_id.clone(),
            principal_id: binding.principal_id.clone(),
            auth_session_id: binding.auth_session_id.clone(),
            gateway_session_id: binding.gateway_session_id.clone(),
            local_policy_digest: header.local_policy_digest.clone(),
            command: GatewaySensitiveAccessCommand::Why,
            sequence,
            objects: vec![GatewaySensitiveAccessObject {
                object_id: "billing.ready".into(),
                content_hash: format!("sha256:{}", "b".repeat(64)),
                classification: SensitiveClassification::Internal,
            }],
        })
        .unwrap()
    }
    #[test]
    fn restart_retains_exact_pending_bytes_and_sequences_then_appends_acks() {
        let temp = tempfile::tempdir().unwrap();
        let h = header(temp.path());
        let b = binding();
        let mut spool = Spool::open(h.clone()).unwrap();
        spool.bind(b.clone()).unwrap();
        let a = event(&h, &b, 1);
        let c = event(&h, &b, 2);
        spool.enqueue(&a).unwrap();
        spool.enqueue(&c).unwrap();
        let original = fs::read(&spool.path).unwrap();
        drop(spool);
        for _ in 0..5 {
            let mut opened = Spool::open(header(temp.path())).unwrap();
            let s = opened.read().unwrap();
            assert_eq!(s.header.setup_request_id, h.setup_request_id);
            assert_eq!(s.pending.len(), 2);
            assert_eq!(s.pending[0].bytes, a.to_canonical_json().unwrap());
            assert_eq!(s.pending[1].bytes, c.to_canonical_json().unwrap());
            assert_eq!(s.sequences[&b.gateway_session_id], 2);
            assert_eq!(fs::read(&opened.path).unwrap(), original);
        }
        let mut opened = Spool::open(h).unwrap();
        for _ in 0..2 {
            let first = opened.read().unwrap().pending.remove(0);
            opened.acknowledge(&first).unwrap();
        }
        assert!(opened.read().unwrap().pending.is_empty());
        assert!(fs::read(&opened.path).unwrap().starts_with(&original));
    }
    #[test]
    fn malformed_journal_is_sticky_and_is_never_truncated_or_reset() {
        let temp = tempfile::tempdir().unwrap();
        let h = header(temp.path());
        let mut spool = Spool::open(h.clone()).unwrap();
        let valid = fs::read(&spool.path).unwrap();
        OpenOptions::new()
            .append(true)
            .open(&spool.path)
            .unwrap()
            .write_all(b"{\"kind\":")
            .unwrap();
        let broken = fs::read(&spool.path).unwrap();
        assert!(matches!(
            spool.read(),
            Err(McpAdapterError::AuditSpoolCorrupt)
        ));
        assert_eq!(fs::read(&spool.path).unwrap(), broken);
        fs::write(&spool.path, &valid).unwrap();
        assert!(matches!(
            spool.read(),
            Err(McpAdapterError::AuditSpoolCorrupt)
        ));
        drop(spool);
        fs::write(temp.path().join(".adoc-audit/spool.log"), &broken).unwrap();
        assert!(matches!(
            Spool::open(h),
            Err(McpAdapterError::AuditSpoolCorrupt)
        ));
    }
    #[test]
    fn invalid_frames_hashes_sequences_and_bindings_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        let h = header(temp.path());
        let b = binding();
        let mut spool = Spool::open(h.clone()).unwrap();
        spool.bind(b.clone()).unwrap();
        spool.enqueue(&event(&h, &b, 1)).unwrap();
        let path = spool.path.clone();
        let original = fs::read(&path).unwrap();
        drop(spool);
        let mut lines: Vec<serde_json::Value> = String::from_utf8(original.clone())
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        let mut fixtures = vec![
            Vec::new(),
            b"[]\n".to_vec(),
            [
                original.clone(),
                b"{\"kind\":\"binding\",\"gateway_session_id\":\"bad\"}\n".to_vec(),
            ]
            .concat(),
            [
                original.clone(),
                b"{\"kind\":\"header\",\"kind\":\"header\"}\n".to_vec(),
            ]
            .concat(),
        ];
        let mut changed = lines.clone();
        changed[2]["event_digest"] = serde_json::json!(format!("sha256:{}", "c".repeat(64)));
        fixtures.push(json_lines(&changed));
        let mut changed = lines.clone();
        changed[1]["auth_session_id"] = serde_json::json!("00000000-0000-4000-8000-000000000006");
        fixtures.push(json_lines(&changed));
        let mut changed = lines.clone();
        changed[1]["extra"] = serde_json::json!(true);
        fixtures.push(json_lines(&changed));
        let mut changed = lines.clone();
        changed.push(changed[2].clone());
        fixtures.push(json_lines(&changed));
        lines[2]["event_bytes"] = serde_json::json!(event(&h, &b, 3).to_canonical_json().unwrap());
        lines[2]["event_digest"] = serde_json::json!(
            adoc_core::validate_gateway_sensitive_access(
                lines[2]["event_bytes"].as_str().unwrap().as_bytes()
            )
            .unwrap()
            .content_digest()
            .unwrap()
        );
        fixtures.push(json_lines(&lines));
        for broken in fixtures {
            fs::write(&path, &broken).unwrap();
            assert!(matches!(
                Spool::open(h.clone()),
                Err(McpAdapterError::AuditSpoolCorrupt)
            ));
            assert_eq!(fs::read(&path).unwrap(), broken);
        }
    }
    fn json_lines(values: &[serde_json::Value]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|v| format!("{v}\n").into_bytes())
            .collect()
    }
    #[test]
    fn same_process_lock_and_context_rebinding_refuse_without_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let h = header(temp.path());
        let spool = Spool::open(h.clone()).unwrap();
        let original = fs::read(&spool.path).unwrap();
        assert!(matches!(
            Spool::open(h.clone()),
            Err(McpAdapterError::AuditSinkUnavailable)
        ));
        drop(spool);
        let mut changed = h.clone();
        changed.repository_id = "00000000-0000-4000-8000-000000000007".into();
        assert!(matches!(
            Spool::open(changed),
            Err(McpAdapterError::AuditSinkUnavailable)
        ));
        assert_eq!(
            fs::read(temp.path().join(".adoc-audit/spool.log")).unwrap(),
            original
        );
        assert!(Spool::open(h).is_ok());
    }
    #[test]
    fn symlink_hardlink_permissions_and_inode_replacement_refuse() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let h = header(temp.path());
        symlink(outside.path(), temp.path().join(".adoc-audit")).unwrap();
        assert!(Spool::open(h.clone()).is_err());
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
        fs::remove_file(temp.path().join(".adoc-audit")).unwrap();
        let mut spool = Spool::open(h.clone()).unwrap();
        let path = spool.path.clone();
        fs::rename(&path, path.with_extension("old")).unwrap();
        fs::write(&path, b"outside").unwrap();
        assert!(matches!(
            spool.read(),
            Err(McpAdapterError::AuditSpoolCorrupt)
        ));
        drop(spool);
        fs::remove_file(&path).unwrap();
        fs::rename(path.with_extension("old"), &path).unwrap();
        fs::hard_link(&path, outside.path().join("alias")).unwrap();
        assert!(Spool::open(h.clone()).is_err());
        fs::remove_file(outside.path().join("alias")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(Spool::open(h.clone()).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::rename(&path, path.with_extension("old")).unwrap();
        symlink(path.with_extension("old"), &path).unwrap();
        assert!(Spool::open(h).is_err());
    }
    #[test]
    fn append_failure_poison_prevents_later_false_success() {
        let temp = tempfile::tempdir().unwrap();
        let h = header(temp.path());
        let b = binding();
        let mut spool = Spool::open(h.clone()).unwrap();
        spool.bind(b.clone()).unwrap();
        spool.file = File::open(&spool.path).unwrap(); // Real read-only descriptor: append fails.
        assert!(matches!(
            spool.enqueue(&event(&h, &b, 1)),
            Err(McpAdapterError::AuditSinkUnavailable)
        ));
        assert!(matches!(
            spool.read(),
            Err(McpAdapterError::AuditSpoolCorrupt)
        ));
    }
    #[test]
    fn durable_sync_and_partial_append_failures_never_release_success() {
        for fault in [Fault::FileSync, Fault::PartialWrite] {
            let temp = tempfile::tempdir().unwrap();
            let h = header(temp.path());
            let b = binding();
            let mut spool = Spool::open(h.clone()).unwrap();
            spool.bind(b.clone()).unwrap();
            IO_FAULT.with(|p| p.set(Some(fault)));
            assert!(matches!(
                spool.enqueue(&event(&h, &b, 1)),
                Err(McpAdapterError::AuditSinkUnavailable)
            ));
            assert!(matches!(
                spool.read(),
                Err(McpAdapterError::AuditSpoolCorrupt)
            ));
            let path = spool.path.clone();
            let bytes = fs::read(&path).unwrap();
            drop(spool);
            if fault == Fault::PartialWrite {
                assert!(matches!(
                    Spool::open(h),
                    Err(McpAdapterError::AuditSpoolCorrupt)
                ));
            } else {
                assert_eq!(Spool::open(h).unwrap().read().unwrap().pending.len(), 1);
            }
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        let temp = tempfile::tempdir().unwrap();
        IO_FAULT.with(|p| p.set(Some(Fault::DirectorySync)));
        assert!(matches!(
            Spool::open(header(temp.path())),
            Err(McpAdapterError::AuditSinkUnavailable)
        ));
    }
    #[test]
    fn ack_sync_failure_is_not_false_recorded_and_restart_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let h = header(temp.path());
        let b = binding();
        let mut spool = Spool::open(h.clone()).unwrap();
        spool.bind(b.clone()).unwrap();
        spool.enqueue(&event(&h, &b, 1)).unwrap();
        let first = spool.read().unwrap().pending.remove(0);
        IO_FAULT.with(|p| p.set(Some(Fault::FileSync)));
        assert!(matches!(
            spool.acknowledge(&first),
            Err(McpAdapterError::AuditSinkUnavailable)
        ));
        assert!(matches!(
            spool.read(),
            Err(McpAdapterError::AuditSpoolCorrupt)
        ));
        drop(spool);
        // A complete ack observed after a failed sync is safe only because a native receipt preceded it.
        let mut opened = Spool::open(h).unwrap();
        assert!(opened.read().unwrap().pending.is_empty());
    }
    #[test]
    fn lock_child_process() {
        let Ok(root) = std::env::var("ADOC_SPOOL_TEST_CHILD_ROOT") else {
            return;
        };
        let opened = Spool::open(header(Path::new(&root)));
        if std::env::var("ADOC_SPOOL_TEST_CHILD_MODE").unwrap() == "deny" {
            assert!(matches!(opened, Err(McpAdapterError::AuditSinkUnavailable)));
            return;
        }
        let _held = opened.unwrap();
        fs::write(Path::new(&root).join("ready"), b"ready").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
    #[test]
    fn cross_process_exclusive_lock_is_released_after_abrupt_exit() {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        let temp = tempfile::tempdir().unwrap();
        let h = header(temp.path());
        let spool = Spool::open(h.clone()).unwrap();
        let child = |mode: &str| {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "gateway_spool::tests::lock_child_process",
                    "--nocapture",
                ])
                .env("ADOC_SPOOL_TEST_CHILD_ROOT", temp.path())
                .env("ADOC_SPOOL_TEST_CHILD_MODE", mode)
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            command
        };
        assert!(child("deny").status().unwrap().success());
        drop(spool);
        let mut held = child("hold").spawn().unwrap();
        let start = Instant::now();
        while !temp.path().join("ready").exists() && start.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let ready = temp.path().join("ready").exists();
        let denied = matches!(
            Spool::open(h.clone()),
            Err(McpAdapterError::AuditSinkUnavailable)
        );
        held.kill().unwrap();
        held.wait().unwrap();
        assert!(ready);
        assert!(denied);
        assert!(Spool::open(h).is_ok());
    }
    #[test]
    fn renamed_root_is_detected_before_append() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("project");
        fs::create_dir(&root).unwrap();
        let h = header(&root);
        let mut spool = Spool::open(h).unwrap();
        fs::rename(&root, parent.path().join("moved")).unwrap();
        fs::create_dir(&root).unwrap();
        assert!(matches!(
            spool.read(),
            Err(McpAdapterError::AuditSpoolCorrupt)
        ));
    }
}
