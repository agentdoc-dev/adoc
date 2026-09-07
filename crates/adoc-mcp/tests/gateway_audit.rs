//! Real stdio and bounded HTTP recording tests; Cloud/native admission is tested separately.
use adoc_mcp::{AgentDocMcpServer, WhyParams};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

const WORKSPACE: &str = "10000000-0000-4000-8000-000000000001";
const REPOSITORY: &str = "10000000-0000-4000-8000-000000000002";
const SESSION: &str = "10000000-0000-4000-8000-000000000003";
const PRINCIPAL: &str = "10000000-0000-4000-8000-000000000004";
const AUTH: &str = "10000000-0000-4000-8000-000000000005";
const TOKEN: &str = "OPERATOR_TOKEN_MUST_NEVER_ESCAPE";

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Ok,
    Deny,
    BadReceipt,
    DropOnce,
    Redirect,
    Chunked,
    Timeout,
}
struct Request {
    path: String,
    headers: String,
    bytes: Vec<u8>,
}
struct FixtureState {
    requests: Vec<Request>,
    mode: Mode,
    egress: char,
}
struct Sink {
    url: String,
    state: Arc<Mutex<FixtureState>>,
    stop: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Sink {
    fn new(mode: Mode) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(FixtureState {
            requests: vec![],
            mode,
            egress: 'a',
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(true));
        let (s, done, go) = (state.clone(), stop.clone(), release.clone());
        let thread = thread::spawn(move || {
            while !done.load(Ordering::SeqCst) {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let request = read_request(&mut stream);
                let is_event = request.path.ends_with("/events");
                let body: Value = serde_json::from_slice(&request.bytes).unwrap();
                assert!(request.headers.to_ascii_lowercase().contains(&format!(
                    "authorization: bearer {}",
                    TOKEN.to_ascii_lowercase()
                )));
                let mut state = s.lock().unwrap();
                let mode = state.mode;
                let egress = state.egress;
                state.requests.push(request);
                if mode == Mode::DropOnce && is_event {
                    state.mode = Mode::Ok;
                    continue;
                }
                drop(state);
                if mode == Mode::Timeout {
                    while !done.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(10));
                    }
                    continue;
                }
                if mode == Mode::Redirect {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/credential-leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    continue;
                }
                if mode == Mode::Chunked {
                    let data = "x".repeat(65 * 1024);
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
                        data.len(),
                        data
                    );
                    continue;
                }
                if mode == Mode::Deny && !is_event {
                    reply(&mut stream, 403, &json!({"secret":"UPSTREAM_DETAILS"}));
                    continue;
                }
                if is_event {
                    let event = adoc_core::validate_gateway_sensitive_access(
                        &s.lock().unwrap().requests.last().unwrap().bytes,
                    )
                    .unwrap();
                    while !go.load(Ordering::SeqCst) && !done.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(5));
                    }
                    let mut receipt = json!({"event_id":body["event_id"], "event_digest":event.content_digest().unwrap(), "gateway_session_id":SESSION, "sequence":body["sequence"], "recorded":true, "replayed":false});
                    if mode == Mode::BadReceipt {
                        receipt["unexpected"] = json!("UPSTREAM_DETAILS");
                    }
                    reply(&mut stream, 200, &receipt);
                } else {
                    reply(
                        &mut stream,
                        200,
                        &json!({"gateway_session_id": SESSION, "workspace_id":WORKSPACE, "repository_id":body["repository_id"], "principal_id":PRINCIPAL, "auth_session_id":AUTH, "local_policy_digest":body["local_policy_digest"], "transmission":{"audit_metadata":true,"egress_policy_digest":format!("sha256:{}", egress.to_string().repeat(64))}}),
                    );
                }
            }
        });
        Self {
            url,
            state,
            stop,
            release,
            thread: Some(thread),
        }
    }
    fn events(&self) -> Vec<Value> {
        self.state
            .lock()
            .unwrap()
            .requests
            .iter()
            .filter(|r| r.path.ends_with("/events"))
            .map(|r| serde_json::from_slice(&r.bytes).unwrap())
            .collect()
    }
}
impl Drop for Sink {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.release.store(true, Ordering::SeqCst);
        let joined = self.thread.take().unwrap().join();
        if !thread::panicking() {
            joined.unwrap();
        }
    }
}
fn read_request(stream: &mut TcpStream) -> Request {
    let mut reader = BufReader::new(stream);
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        headers.push_str(&line);
        if line == "\r\n" {
            break;
        }
    }
    let path = headers
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_owned();
    let length: usize = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse().unwrap())
        })
        .unwrap();
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).unwrap();
    Request {
        path,
        headers,
        bytes,
    }
}
fn reply(stream: &mut TcpStream, status: u16, value: &Value) {
    let bytes = value.to_string();
    let _ = write!(
        stream,
        "HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        bytes.len(),
        bytes
    );
}

fn policy() -> adoc_core::RetrievalPolicy {
    serde_json::from_value(json!({"audience":"restricted", "allowed_visibilities":["public", "internal", "restricted"], "excluded_object_ids":[]})).unwrap()
}
fn fixture(root: &Path, sink: &Sink) {
    fs::write(root.join("token"), TOKEN).unwrap();
    fs::write(root.join("audit.json"), json!({"cloud_url":sink.url,"workspace_id":WORKSPACE,"repository_id":REPOSITORY,"bearer_token_file":"token"}).to_string()).unwrap();
    fs::write(
        root.join("gateway.yaml"),
        json!({"version":1,"mode":"strict","docs_path":"docs","retrieval_policy":policy()})
            .to_string(),
    )
    .unwrap();
    let node = |id: &str, class: &str| json!({"type":"knowledge_object","id":id,"kind":"claim","status":"draft","visibility":class,"content_hash":format!("sha256:{}", "b".repeat(64)),"body":"LOCAL_SENSITIVE_BODY", "page_id":"billing.page","source_span":{"path":"docs/billing.adoc","line":1,"column":1},"fields":{},"relations":{"depends_on":[],"supersedes":[],"related_to":[]}});
    let mut internal = node("billing.internal", "internal");
    internal["body"] = json!("INTERNAL_ONLY_QUERY LOCAL_SENSITIVE_BODY");
    internal["status"] = json!("verified");
    internal["impacts"] = json!(["src/billing.rs"]);
    let mut stale = node("billing.stale", "internal");
    stale["fields"] = json!({"expires_at":"2000-01-01"});
    let mut conflict = node("billing.conflict", "restricted");
    conflict["kind"] = json!("contradiction");
    conflict["status"] = json!("unresolved");
    conflict["severity"] = json!("high");
    conflict["contradiction_claims"] = json!(["billing.stale", "billing.public"]);
    let mut public = node("billing.public", "public");
    public["body"] = json!("ORDINARY_PUBLIC_BODY");
    fs::write(root.join("graph.json"), json!({"schema_version":"adoc.graph.v6","repository_identity":null,"nodes":[internal,stale,conflict,public],"edges":[],"diagnostics":[]}).to_string()).unwrap();
}
fn bound(root: &Path) -> AgentDocMcpServer {
    AgentDocMcpServer::new(root.into())
        .with_retrieval_policy(policy())
        .with_audit_config(&root.join("audit.json"))
        .unwrap()
}
fn why() -> WhyParams {
    WhyParams {
        project_root: None,
        object_id: "billing.internal".into(),
        artifact: Some("graph.json".into()),
    }
}

struct StdioGateway {
    child: Child,
    stdin: ChildStdin,
    responses: mpsc::Receiver<Value>,
}
impl StdioGateway {
    fn start(root: &Path) -> Self {
        Self::start_with_audit(root, true)
    }
    fn start_with_audit(root: &Path, audited: bool) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_adoc-mcp"));
        command.current_dir(root).args(["--config", "gateway.yaml"]);
        if audited {
            command.args(["--audit-config", "audit.json"]);
        }
        let mut child = command
            .env("ADOC_LOG", "ureq=trace,ureq::run=trace,adoc_mcp=debug")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, responses) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if send
                    .send(serde_json::from_str(&line.unwrap()).unwrap())
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut server = Self {
            child,
            stdin,
            responses,
        };
        server.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"SPOOFED_IDENTITY","version":"0"}}}));
        assert_eq!(server.receive()["id"], 1);
        server.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        server
    }
    fn send(&mut self, value: Value) {
        writeln!(self.stdin, "{value}").unwrap();
        self.stdin.flush().unwrap();
    }
    fn call(&mut self, id: u64, tool: &str, mut args: Value) {
        args["artifact"] = json!("graph.json");
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":format!("adoc_{tool}"),"arguments":args}}));
    }
    fn receive(&self) -> Value {
        self.responses
            .recv_timeout(Duration::from_secs(15))
            .unwrap()
    }
}
impl Drop for StdioGateway {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        assert!(!stderr.contains(TOKEN), "credential leaked in logs");
    }
}

#[test]
fn stdio_six_tools_record_exact_subjects_before_output_and_label_both_classes() {
    let sink = Sink::new(Mode::Ok);
    let root = tempfile::tempdir().unwrap();
    fixture(root.path(), &sink);
    let mut server = StdioGateway::start(root.path());
    let cases = [
        (
            "why",
            json!({"object_id":"billing.internal"}),
            vec!["billing.internal"],
            "internal",
        ),
        (
            "search",
            json!({"query":"INTERNAL_ONLY_QUERY","objects_only":true,"lexical":true}),
            vec!["billing.internal"],
            "internal",
        ),
        (
            "graph",
            json!({"object_id":"billing.internal"}),
            vec!["billing.internal"],
            "internal",
        ),
        ("stale", json!({}), vec!["billing.stale"], "internal"),
        (
            "contradictions",
            json!({}),
            vec!["billing.conflict", "billing.stale"],
            "restricted",
        ),
        (
            "impacted_by",
            json!({"paths":["src/billing.rs"]}),
            vec!["billing.internal"],
            "internal",
        ),
    ];
    for (i, (command, args, subjects, class)) in cases.into_iter().enumerate() {
        sink.release.store(false, Ordering::SeqCst);
        server.call(i as u64 + 2, command, args);
        for _ in 0..500 {
            if sink.events().len() > i {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(sink.events().len(), i + 1, "missing event for {command}");
        assert!(
            server
                .responses
                .recv_timeout(Duration::from_millis(50))
                .is_err(),
            "sensitive output preceded receipt"
        );
        sink.release.store(true, Ordering::SeqCst);
        let response = server.receive();
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(
            response["result"]["content"][1]["text"],
            format!("Sensitive ({class})."),
            "{command}: {}",
            sink.events().last().unwrap()
        );
        let events = sink.events();
        let event = &events[i];
        assert_eq!(event["command"], command);
        assert_eq!(event["sequence"], i + 1);
        assert_eq!(
            event["caller"],
            json!({"principal_id":PRINCIPAL,"auth_session_id":AUTH,"gateway_session_id":SESSION})
        );
        assert_eq!(
            event["objects"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["object_id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            subjects
        );
        assert!(!event.to_string().contains("LOCAL_SENSITIVE_BODY"));
        assert!(!event.to_string().contains("SPOOFED_IDENTITY"));
    }
    let state = sink.state.lock().unwrap();
    assert_eq!(state.requests.len(), 13);
    let setups: Vec<_> = state
        .requests
        .iter()
        .filter(|r| !r.path.ends_with("/events"))
        .map(|r| &r.bytes)
        .collect();
    assert!(
        setups.iter().all(|s| *s == setups[0]),
        "setup identity must replay exactly"
    );
}

#[test]
fn failed_fresh_preflight_sends_no_subjects_and_new_egress_is_used() {
    let sink = Sink::new(Mode::Ok);
    let root = tempfile::tempdir().unwrap();
    fixture(root.path(), &sink);
    let server = bound(root.path());
    sink.state.lock().unwrap().mode = Mode::Deny;
    assert!(
        server
            .run_why(why())
            .unwrap_err()
            .to_string()
            .contains("retrieval.audit_sink_unavailable")
    );
    assert!(sink.events().is_empty());
    {
        let mut state = sink.state.lock().unwrap();
        state.mode = Mode::Ok;
        state.egress = 'c';
    }
    assert!(server.run_why(why()).is_ok());
    let state = sink.state.lock().unwrap();
    assert!(
        state
            .requests
            .iter()
            .filter(|r| r.path.ends_with("/events"))
            .all(|r| r.headers.contains(&format!("sha256:{}", "c".repeat(64))))
    );
}

#[test]
fn ambiguous_ack_retries_identical_bytes_then_records_new_response_with_shared_sequence() {
    let sink = Sink::new(Mode::Ok);
    let root = tempfile::tempdir().unwrap();
    fixture(root.path(), &sink);
    let server = bound(root.path());
    sink.state.lock().unwrap().mode = Mode::DropOnce;
    assert!(server.run_why(why()).is_err());
    let clone = server.clone();
    let first = thread::spawn(move || clone.run_why(why()));
    let second = thread::spawn(move || server.run_why(why()));
    assert!(first.join().unwrap().is_ok());
    assert!(second.join().unwrap().is_ok());
    let state = sink.state.lock().unwrap();
    let events: Vec<_> = state
        .requests
        .iter()
        .filter(|r| r.path.ends_with("/events"))
        .collect();
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].bytes, events[1].bytes);
    let parsed: Vec<Value> = events
        .iter()
        .map(|r| serde_json::from_slice(&r.bytes).unwrap())
        .collect();
    assert_eq!(
        parsed
            .iter()
            .map(|v| v["sequence"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [1, 1, 2, 3]
    );
    assert_ne!(parsed[1]["event_id"], parsed[2]["event_id"]);
    for pair in state.requests[1..].chunks_exact(2) {
        assert!(!pair[0].path.ends_with("/events"));
        assert!(pair[1].path.ends_with("/events"));
    }
}

#[test]
fn caller_root_policy_and_unknown_arguments_cannot_replace_binding() {
    let sink = Sink::new(Mode::Ok);
    let root = tempfile::tempdir().unwrap();
    fixture(root.path(), &sink);
    let server = bound(root.path());
    let other = tempfile::tempdir().unwrap();
    fixture(other.path(), &sink);
    let mut request = why();
    request.project_root = Some(other.path().into());
    assert!(server.run_why(request).is_err());
    assert!(sink.events().is_empty());
    let mut request = why();
    request.project_root = Some(root.path().join("."));
    assert!(server.run_why(request).is_ok());
    let server = server.with_retrieval_policy(policy());
    assert!(server.run_why(why()).is_err());
    let mut stdio = StdioGateway::start(root.path());
    stdio.call(
        2,
        "why",
        json!({"object_id":"billing.internal","principal_id":PRINCIPAL}),
    );
    let refused = stdio.receive();
    assert!(
        refused.get("error").is_some() || refused["result"]["isError"] == true,
        "{refused}"
    );
    assert_eq!(sink.events().len(), 1);
}

#[test]
fn public_no_hit_and_denied_queries_do_not_emit() {
    let sink = Sink::new(Mode::Ok);
    let root = tempfile::tempdir().unwrap();
    fixture(root.path(), &sink);
    let mut server = StdioGateway::start(root.path());
    for (i, object) in ["billing.public", "billing.absent"].into_iter().enumerate() {
        server.call(i as u64 + 2, "why", json!({"object_id":object}));
        let response = server.receive();
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(response["result"]["content"].as_array().unwrap().len(), 1);
    }
    server.call(
        4,
        "search",
        json!({"query":"NEVER_MATCHES_ANY_SUBJECT", "lexical":true}),
    );
    let absent = server.receive();
    assert_eq!(absent["result"]["structuredContent"]["records"], json!([]));
    assert_eq!(absent["result"]["content"].as_array().unwrap().len(), 1);
    assert!(sink.events().is_empty());
    let public = AgentDocMcpServer::new(root.path().into())
        .with_audit_config(&root.path().join("audit.json"))
        .unwrap();
    assert_eq!(public.run_why(why()).unwrap()["records"], json!([]));
    assert!(sink.events().is_empty());
}

#[test]
fn malformed_receipt_is_closed_and_never_releases_sensitive_stdio_output() {
    let sink = Sink::new(Mode::Ok);
    let root = tempfile::tempdir().unwrap();
    fixture(root.path(), &sink);
    let mut server = StdioGateway::start(root.path());
    sink.state.lock().unwrap().mode = Mode::BadReceipt;
    server.call(2, "why", json!({"object_id":"billing.internal"}));
    let response = server.receive();
    assert_eq!(
        response["error"]["data"]["code"],
        "retrieval.audit_sink_unavailable"
    );
    assert!(!response.to_string().contains("LOCAL_SENSITIVE_BODY"));
    assert!(!response.to_string().contains("UPSTREAM_DETAILS"));
}

#[test]
fn setup_redirect_chunked_overflow_and_timeout_refuse_without_credential_error_details() {
    for mode in [Mode::Redirect, Mode::Chunked, Mode::Timeout] {
        let sink = Sink::new(mode);
        let root = tempfile::tempdir().unwrap();
        fixture(root.path(), &sink);
        let start = std::time::Instant::now();
        let error = AgentDocMcpServer::new(root.path().into())
            .with_retrieval_policy(policy())
            .with_audit_config(&root.path().join("audit.json"))
            .unwrap_err();
        assert!(start.elapsed() < Duration::from_secs(12));
        assert_eq!(
            error.to_string(),
            "error[retrieval.audit_sink_unavailable] Sensitive retrieval requires an available trusted audit sink."
        );
        assert_eq!(sink.state.lock().unwrap().requests.len(), 1);
    }
}

#[test]
fn prose_with_a_real_sensitive_reference_keeps_its_envelope_and_gets_an_audit_label() {
    let sink = Sink::new(Mode::Ok);
    let root = tempfile::tempdir().unwrap();
    fixture(root.path(), &sink);
    fs::create_dir(root.path().join("docs")).unwrap();
    fs::write(root.path().join("docs/billing.adoc"), "# Billing @doc(billing.page)\n\nOrientation points to [[billing.internal]].\n\n::claim billing.internal\nstatus: draft\nvisibility: internal\n--\nPrivate credits behavior.\n::\n").unwrap();
    let build = AgentDocMcpServer::new(root.path().into())
        .run_build(adoc_mcp::BuildParams {
            project_root: None,
            path: Some("docs".into()),
            out: Some("dist".into()),
            no_embeddings: true,
        })
        .unwrap();
    assert_eq!(build["ok"], true, "{build}");
    let mut server = StdioGateway::start(root.path());
    server.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"adoc_search","arguments":{"query":"Orientation","prose_only":true,"lexical":true,"artifact":"dist/docs.graph.json"}}}));
    let response = server.receive();
    assert_eq!(
        response["result"]["content"][1]["text"], "Sensitive (internal).",
        "{response}"
    );
    let records = response["result"]["structuredContent"]["records"]
        .as_array()
        .unwrap();
    assert!(!records.is_empty());
    assert!(
        records
            .iter()
            .all(|record| record["record_type"] == "prose")
    );
    assert_eq!(sink.events()[0]["objects"].as_array().unwrap().len(), 1);
    assert_eq!(
        sink.events()[0]["objects"][0]["object_id"],
        "billing.internal"
    );
}

const REFERENCE_TARGET: &str = "guide.internal-long-reference";

fn reference_fixture(root: &Path, sink: &Sink, question_floor: &str, summary_body: &str) -> Value {
    fixture(root, sink);
    fs::write(root.join("gateway.yaml"), json!({"version":1,"mode":"strict","docs_path":"docs","retrieval_policy":{"audience":"internal","allowed_visibilities":["public","internal"],"excluded_object_ids":[]}}).to_string()).unwrap();
    fs::create_dir(root.join("docs")).unwrap();
    let source = format!(
        "# Guide @doc(guide.page)\n\n::claim {REFERENCE_TARGET}\nstatus: plain\nvisibility: internal\n--\nINTERNAL_TARGET_BODY.\n::\n\n::question guide.question\nstatus: answered\nvisibility: public\nresolved_by: {REFERENCE_TARGET}\n{question_floor}--\nzzzxqquestiontoken ordinary question.\n::\n\n::claim guide.a\nstatus: plain\n--\nFirst public claim.\n::\n\n::claim guide.b\nstatus: plain\n--\nSecond public claim.\n::\n\n::contradiction guide.conflict\nseverity: high\nstatus: unresolved\nclaims: [guide.a, guide.b]\n--\n{summary_body}\n::\n"
    );
    fs::write(root.join("docs/index.adoc"), source).unwrap();
    let build = AgentDocMcpServer::new(root.into())
        .run_build(adoc_mcp::BuildParams {
            project_root: None,
            path: Some("docs".into()),
            out: Some("dist".into()),
            no_embeddings: true,
        })
        .unwrap();
    assert_eq!(build["ok"], true, "{build}");
    fs::copy(root.join("dist/docs.graph.json"), root.join("graph.json")).unwrap();
    let graph: Value = serde_json::from_slice(&fs::read(root.join("graph.json")).unwrap()).unwrap();
    let target = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == REFERENCE_TARGET)
        .unwrap();
    json!({"object_id":REFERENCE_TARGET,"content_hash":target["content_hash"],"classification":"internal"})
}

fn assert_audit_refusal(response: &Value) {
    assert_eq!(
        response["error"]["data"]["code"], "retrieval.audit_sink_unavailable",
        "{response}"
    );
    assert!(!response.to_string().contains(REFERENCE_TARGET));
    assert!(!response.to_string().contains("INTERNAL_TARGET_BODY"));
}

fn answered_question_reference(command: &str) {
    for audited in [false, true] {
        let sink = Sink::new(Mode::Ok);
        let root = tempfile::tempdir().unwrap();
        let subject = reference_fixture(root.path(), &sink, "", "Ordinary contradiction.");
        let original = fs::read(root.path().join("graph.json")).unwrap();
        let mut gateway = StdioGateway::start_with_audit(root.path(), audited);
        let args = if command == "why" {
            json!({"object_id":"guide.question"})
        } else {
            json!({"query":"zzzxqquestiontoken","lexical":true,"objects_only":true})
        };
        gateway.call(2, command, args);
        let response = gateway.receive();
        if audited {
            assert_eq!(
                response["result"]["content"][1]["text"], "Sensitive (internal).",
                "{response}"
            );
            let records = response["result"]["structuredContent"]["records"]
                .as_array()
                .unwrap();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0]["id"], "guide.question");
            assert_eq!(records[0]["fields"]["resolved_by"], REFERENCE_TARGET);
            assert_eq!(sink.events().len(), 1);
            assert_eq!(sink.events()[0]["objects"], json!([subject]));
            assert_eq!(sink.events()[0]["command"], command);
            assert!(!response.to_string().contains("INTERNAL_TARGET_BODY"));
        } else {
            assert_audit_refusal(&response);
            assert!(sink.state.lock().unwrap().requests.is_empty());
        }
        assert_eq!(fs::read(root.path().join("graph.json")).unwrap(), original);
    }
}

#[test]
fn repair1_question_why_records_exposed_resolved_by() {
    answered_question_reference("why");
}

#[test]
fn repair1_question_search_records_exposed_resolved_by() {
    answered_question_reference("search");
}

#[test]
fn repair1_hidden_resolved_by_removes_witness_and_sensitive_scanned_nonhits_emit_none() {
    for audited in [false, true] {
        let sink = Sink::new(Mode::Ok);
        let root = tempfile::tempdir().unwrap();
        reference_fixture(
            root.path(),
            &sink,
            "field_visibility: resolved_by=restricted\n",
            "Ordinary contradiction.",
        );
        let mut gateway = StdioGateway::start_with_audit(root.path(), audited);
        for (id, command, args) in [
            (2, "why", json!({"object_id":"guide.question"})),
            (
                3,
                "search",
                json!({"query":"zzzxqquestiontoken","lexical":true,"objects_only":true}),
            ),
            (
                4,
                "search",
                json!({"query":"NEVER_MATCHES_ANY_SUBJECT","lexical":true,"objects_only":true}),
            ),
        ] {
            gateway.call(id, command, args);
            let response = gateway.receive();
            assert!(response.get("error").is_none(), "{response}");
            assert_eq!(response["result"]["content"].as_array().unwrap().len(), 1);
            let records = response["result"]["structuredContent"]["records"]
                .as_array()
                .unwrap();
            assert_eq!(records.len(), usize::from(id != 4));
            assert!(!response.to_string().contains(REFERENCE_TARGET));
            assert!(!response.to_string().contains("INTERNAL_TARGET_BODY"));
            if let Some(record) = records.first() {
                assert!(record["fields"].get("resolved_by").is_none());
            }
        }
        assert!(sink.events().is_empty());
    }
}

#[test]
fn repair1_contradiction_full_and_partial_typed_references_require_audit() {
    // Both reviewed cut positions expose only part of the same real typed reference.
    for prefix in [
        String::new(),
        "x".repeat(90),
        "x".repeat(110),
        "é".repeat(90),
    ] {
        for audited in [false, true] {
            let sink = Sink::new(Mode::Ok);
            let root = tempfile::tempdir().unwrap();
            let body = format!("{prefix} [[{REFERENCE_TARGET}]]");
            let subject = reference_fixture(root.path(), &sink, "", &body);
            let mut gateway = StdioGateway::start_with_audit(root.path(), audited);
            gateway.call(2, "contradictions", json!({}));
            let response = gateway.receive();
            if audited {
                assert_eq!(
                    response["result"]["content"][1]["text"], "Sensitive (internal).",
                    "prefix={prefix}: {response}"
                );
                let summary =
                    response["result"]["structuredContent"]["contradictions"][0]["summary"]
                        .as_str()
                        .unwrap();
                let expected = if prefix.is_empty() {
                    body.trim().to_string()
                } else {
                    format!("{}…", body.chars().take(119).collect::<String>())
                };
                assert_eq!(summary, expected);
                assert_eq!(sink.events().len(), 1);
                assert_eq!(sink.events()[0]["objects"], json!([subject]));
                assert!(!response.to_string().contains("INTERNAL_TARGET_BODY"));
            } else {
                assert_audit_refusal(&response);
                assert!(sink.state.lock().unwrap().requests.is_empty());
            }
        }
    }
}

#[test]
fn repair1_contradiction_references_beyond_summary_or_on_later_line_emit_no_event() {
    for (body, expected) in [
        (
            format!("{} [[{REFERENCE_TARGET}]]", "x".repeat(130)),
            format!("{}…", "x".repeat(119)),
        ),
        (
            format!("Ordinary first line.\n[[{REFERENCE_TARGET}]]"),
            "Ordinary first line.".into(),
        ),
        // The first-line spelling is code; the real typed reference is wholly unexposed.
        (
            format!("`[[{REFERENCE_TARGET}]]`\n[[{REFERENCE_TARGET}]]"),
            format!("`[[{REFERENCE_TARGET}]]`"),
        ),
    ] {
        for audited in [false, true] {
            let sink = Sink::new(Mode::Ok);
            let root = tempfile::tempdir().unwrap();
            reference_fixture(root.path(), &sink, "", &body);
            let mut gateway = StdioGateway::start_with_audit(root.path(), audited);
            gateway.call(2, "contradictions", json!({}));
            let response = gateway.receive();
            assert!(response.get("error").is_none(), "{response}");
            assert_eq!(response["result"]["content"].as_array().unwrap().len(), 1);
            let summary = response["result"]["structuredContent"]["contradictions"][0]["summary"]
                .as_str()
                .unwrap();
            assert_eq!(summary, expected);
            if !expected.contains(REFERENCE_TARGET) {
                assert!(!response.to_string().contains(REFERENCE_TARGET));
            }
            assert!(sink.events().is_empty());
        }
    }
}
