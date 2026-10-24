//! New POC010 integration tests. The observable effect is the local durable
//! transaction append. A killed worker can lose its caller acknowledgement;
//! the completion receipt is already in the same frame as that local effect.
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RESOURCE: &str = "poc010-test-vault";
const LIMIT: Duration = Duration::from_secs(20);
static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Temp {
    root: PathBuf,
    evidence: PathBuf,
    next: AtomicU64,
}

impl Temp {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let unique = format!(
            "{name}-{}-{stamp}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::SeqCst)
        );
        let root = std::env::temp_dir().join(format!("poc010-{unique}"));
        fs::create_dir(&root).unwrap();
        let evidence = std::env::var_os("POC010_TEST_EVIDENCE")
            .map(PathBuf::from)
            .map(|path| path.join(&unique))
            .unwrap_or_else(|| root.join("evidence"));
        fs::create_dir_all(&evidence).unwrap();
        Self {
            root,
            evidence,
            next: AtomicU64::new(0),
        }
    }

    fn wal(&self) -> PathBuf {
        self.root.join("actions.wal")
    }
    fn wal_arg(&self) -> String {
        self.wal().to_string_lossy().into_owned()
    }

    fn request(&self, id: &str, payload: &str) -> PathBuf {
        self.request_value(json!({
            "principal": "principal:alice",
            "operation": "persist",
            "resource": RESOURCE,
            "scope": "poc010:persist",
            "context": "five-cycle-context",
            "request_id": id,
            "payload": payload
        }))
    }

    fn request_value(&self, value: Value) -> PathBuf {
        let path = self.root.join(format!(
            "request-{}.json",
            self.next.fetch_add(1, Ordering::SeqCst)
        ));
        let bytes = serde_json::to_vec_pretty(&value).unwrap();
        fs::write(&path, &bytes).unwrap();
        fs::write(self.evidence.join(path.file_name().unwrap()), &bytes).unwrap();
        path
    }

    fn spawn(&self, label: &str, args: Vec<String>) -> Running {
        let serial = self.next.fetch_add(1, Ordering::SeqCst);
        let log_prefix = self.evidence.join(format!("{serial:03}-{label}"));
        fs::write(
            log_prefix.with_extension("command.json"),
            serde_json::to_vec_pretty(&args).unwrap(),
        )
        .unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_poc010-action-continuity"))
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start POC010 worker");
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (sender, lines) = mpsc::channel();
        let stdout_reader = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        if sender.send(Ok(line)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error.to_string()));
                        break;
                    }
                }
            }
        });
        let stderr_reader = thread::spawn(move || {
            let mut reader = stderr;
            let mut text = String::new();
            reader.read_to_string(&mut text).expect("read child stderr");
            text
        });
        Running {
            child,
            lines,
            stdout_reader: Some(stdout_reader),
            stderr_reader: Some(stderr_reader),
            events: Vec::new(),
            stdout: String::new(),
            log_prefix,
            saved: false,
        }
    }

    fn worker(&self, label: &str, request: &Path, mode: &str) -> Running {
        self.spawn(
            label,
            vec![
                "worker".into(),
                self.wal_arg(),
                request.to_string_lossy().into_owned(),
                label.into(),
                mode.into(),
            ],
        )
    }

    fn init(&self) {
        let output = self
            .spawn("init", vec!["init".into(), self.wal_arg(), RESOURCE.into()])
            .finish();
        output.assert_success();
    }

    fn inspect(&self) -> Value {
        let output = self
            .spawn(
                "inspect",
                vec!["inspect".into(), self.wal_arg(), RESOURCE.into()],
            )
            .finish();
        output.assert_success();
        let value = output.result().clone();
        assert_eq!(value["status"], "Inspected");
        value
    }

    fn normal(&self, label: &str, request: &Path) -> Output {
        let output = self.worker(label, request, "normal").finish();
        output.assert_success();
        assert!(
            output
                .events
                .iter()
                .any(|v| v["event"] == "authorization_verified"),
            "fresh permit must be verified before any successful cached result"
        );
        output
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Running {
    child: Child,
    lines: Receiver<Result<String, String>>,
    stdout_reader: Option<JoinHandle<()>>,
    stderr_reader: Option<JoinHandle<String>>,
    events: Vec<Value>,
    stdout: String,
    log_prefix: PathBuf,
    saved: bool,
}

impl Running {
    fn record(&mut self, line: Result<String, String>) -> Value {
        let line = line.expect("read child stdout");
        self.stdout.push_str(&line);
        let value: Value = serde_json::from_str(line.trim())
            .unwrap_or_else(|error| panic!("non-JSON child stdout {line:?}: {error}"));
        self.events.push(value.clone());
        value
    }

    fn until(&mut self, event: &str) -> Value {
        let deadline = Instant::now() + LIMIT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let line = self.lines.recv_timeout(remaining).unwrap_or_else(|error| {
                panic!("waiting for {event}: {error}; stdout={}", self.stdout)
            });
            let value = self.record(line);
            if value["event"] == event {
                return value;
            }
            assert_ne!(
                value["event"], "result",
                "worker completed before requested pause {event}"
            );
        }
    }

    fn release(&mut self) {
        let stdin = self
            .child
            .stdin
            .as_mut()
            .expect("worker stdin remains available");
        stdin.write_all(b"continue\n").unwrap();
        stdin.flush().unwrap();
    }

    fn kill(mut self) -> Output {
        assert!(
            self.child.try_wait().unwrap().is_none(),
            "worker must still be alive at the controlled crash point"
        );
        self.child.kill().expect("kill paused worker");
        let output = self.finish();
        assert!(
            !output.status.success(),
            "worker must actually terminate unsuccessfully"
        );
        assert!(
            !output.events.iter().any(|v| v["event"] == "result"),
            "killed worker must not acknowledge completion"
        );
        output
    }

    fn finish(mut self) -> Output {
        drop(self.child.stdin.take());
        let deadline = Instant::now() + LIMIT;
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "child did not exit within {LIMIT:?}"
            );
            thread::sleep(Duration::from_millis(5));
        };
        if let Some(reader) = self.stdout_reader.take() {
            reader.join().unwrap();
        }
        while let Ok(line) = self.lines.try_recv() {
            self.record(line);
        }
        let stderr = self.stderr_reader.take().unwrap().join().unwrap();
        self.save(&stderr, Some(&status));
        Output {
            status,
            events: self.events.clone(),
            stdout: self.stdout.clone(),
            stderr,
        }
    }

    fn save(&mut self, stderr: &str, status: Option<&ExitStatus>) {
        fs::write(
            self.log_prefix.with_extension("stdout.ndjson"),
            &self.stdout,
        )
        .unwrap();
        fs::write(self.log_prefix.with_extension("stderr.txt"), stderr).unwrap();
        fs::write(self.log_prefix.with_extension("exit.json"), serde_json::to_vec_pretty(&json!({
            "success": status.map(|value| value.success()), "code": status.and_then(|value| value.code())
        })).unwrap()).unwrap();
        self.saved = true;
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if !self.saved {
            let _ = self.child.kill();
            let status = self.child.wait().ok();
            if let Some(reader) = self.stdout_reader.take() {
                let _ = reader.join();
            }
            while let Ok(Ok(line)) = self.lines.try_recv() {
                self.stdout.push_str(&line);
            }
            let stderr = self
                .stderr_reader
                .take()
                .and_then(|reader| reader.join().ok())
                .unwrap_or_default();
            // Preserve the trace even if an assertion aborts the test path.
            let _ = fs::write(
                self.log_prefix.with_extension("stdout.ndjson"),
                &self.stdout,
            );
            let _ = fs::write(self.log_prefix.with_extension("stderr.txt"), stderr);
            let _ = fs::write(
                self.log_prefix.with_extension("exit.json"),
                format!(
                    "{{\"aborted_test\":true,\"success\":{}}}",
                    status.is_some_and(|value| value.success())
                ),
            );
        }
    }
}

struct Output {
    status: ExitStatus,
    events: Vec<Value>,
    stdout: String,
    stderr: String,
}

impl Output {
    fn assert_success(&self) {
        assert!(
            self.status.success(),
            "status={:?}\nstdout={}\nstderr={}",
            self.status,
            self.stdout,
            self.stderr
        );
    }

    fn result(&self) -> &Value {
        self.events
            .iter()
            .rev()
            .find(|value| value["event"] == "result")
            .expect("final structured result")
    }

    fn assert_refused(&self) {
        assert_eq!(
            self.status.code(),
            Some(2),
            "stdout={} stderr={}",
            self.stdout,
            self.stderr
        );
        assert!(!self.events.iter().any(|value| value["event"] == "result"));
        let error = self
            .stderr
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|value| value["event"] == "error")
            .expect("structured refusal on stderr");
        assert!(error["reason"]
            .as_str()
            .is_some_and(|reason| !reason.is_empty()));
    }
}

fn assert_digest(value: &Value) {
    let digest = value.as_str().expect("digest string");
    assert_eq!(digest.len(), 64);
    assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
}

fn race(temp: &Temp, request: &Path, label: &str) -> (Output, Output) {
    let mut first = temp.worker(&format!("{label}-worker-a"), request, "barrier");
    let mut second = temp.worker(&format!("{label}-worker-b"), request, "barrier");
    first.until("ready");
    second.until("ready");
    for worker in [&first, &second] {
        let verified = worker
            .events
            .iter()
            .position(|value| value["event"] == "authorization_verified")
            .expect("each permit must already be validated and consumed before the claim barrier");
        let ready = worker
            .events
            .iter()
            .position(|value| value["event"] == "ready")
            .unwrap();
        assert!(verified < ready);
        assert_eq!(worker.events[verified]["consumed"], true);
        assert!(!worker.events.iter().any(|value| matches!(value["event"].as_str(),
            Some("storage_verified" | "before_commit" | "atomic_claim" | "effect_durable_before_ack" | "result"))),
            "neither worker may reach storage or claim an effect before both validated permits are ready");
    }
    let a = first
        .events
        .iter()
        .find(|value| value["event"] == "authorization_issued")
        .unwrap();
    let b = second
        .events
        .iter()
        .find(|value| value["event"] == "authorization_issued")
        .unwrap();
    assert_eq!(a["action_sha256"], b["action_sha256"]);
    assert_digest(&a["action_sha256"]);
    assert_ne!(
        a["authorization_id"], b["authorization_id"],
        "independently issued permit identities must be distinguishable across processes"
    );
    assert!(a.get("generation").is_some() && a.get("nonce").is_some());
    assert!(b.get("generation").is_some() && b.get("nonce").is_some());
    // Both independent workers have validated and consumed their distinct
    // permits; neither has accessed the shared claim store before release.
    first.release();
    second.release();
    let first = first.finish();
    let second = second.finish();
    first.assert_success();
    second.assert_success();
    for output in [&first, &second] {
        assert!(output
            .events
            .iter()
            .any(|value| value["event"] == "authorization_verified"));
        assert_eq!(output.result()["effect_count"], 1);
        assert_digest(&output.result()["receipt_sha256"]);
    }
    let mut statuses = [
        first.result()["status"].as_str().unwrap(),
        second.result()["status"].as_str().unwrap(),
    ];
    statuses.sort();
    assert_eq!(statuses, ["AlreadyCommitted", "Committed"]);
    assert_eq!(
        first.result()["receipt_sha256"],
        second.result()["receipt_sha256"]
    );
    assert_eq!(
        first.result()["action_sha256"],
        second.result()["action_sha256"]
    );
    assert_eq!(temp.inspect()["effect_count"], 1);
    (first, second)
}

#[test]
fn independent_processes_claim_one_logical_action_once() {
    let temp = Temp::new("independent-process-race");
    temp.init();
    let request = temp.request("request-1", "observable local append");
    race(&temp, &request, "race");
}

#[test]
fn identical_payload_with_distinct_logical_requests_produces_two_effects() {
    let temp = Temp::new("distinct-request-ids");
    temp.init();
    let a = temp.normal("request-a", &temp.request("request-a", "identical payload"));
    let b = temp.normal("request-b", &temp.request("request-b", "identical payload"));
    assert_eq!(a.result()["status"], "Committed");
    assert_eq!(b.result()["status"], "Committed");
    assert_ne!(a.result()["action_sha256"], b.result()["action_sha256"]);
    assert_eq!(temp.inspect()["effect_count"], 2);
}

#[test]
fn invalid_permits_are_refused_before_new_or_cached_effects() {
    let temp = Temp::new("invalid-permits");
    temp.init();
    let request = temp.request("request-1", "authorized local append");
    for cached in [false, true] {
        if cached {
            assert_eq!(
                temp.normal("valid", &request).result()["status"],
                "Committed"
            );
        }
        let before = fs::read(temp.wal()).unwrap();
        for mode in ["wrong-binding", "wrong-nonce", "unknown-handle", "consumed"] {
            temp.worker(&format!("{mode}-cached-{cached}"), &request, mode)
                .finish()
                .assert_refused();
            assert_eq!(
                fs::read(temp.wal()).unwrap(),
                before,
                "invalid permit changed persistent state: {mode}"
            );
            assert_eq!(temp.inspect()["effect_count"], usize::from(cached));
        }
    }
}

#[test]
fn changed_payload_under_existing_logical_request_is_a_conflict() {
    let temp = Temp::new("changed-payload");
    temp.init();
    let original = temp.request("request-1", "original payload");
    let first = temp.normal("original", &original);
    let before = fs::read(temp.wal()).unwrap();
    temp.worker(
        "changed",
        &temp.request("request-1", "changed payload"),
        "normal",
    )
    .finish()
    .assert_refused();
    assert_eq!(fs::read(temp.wal()).unwrap(), before);
    let retry = temp.normal("retry", &original);
    assert_eq!(retry.result()["status"], "AlreadyCommitted");
    assert_eq!(
        retry.result()["receipt_sha256"],
        first.result()["receipt_sha256"]
    );
    assert_eq!(temp.inspect()["effect_count"], 1);
}

#[test]
fn missing_wal_is_refused_without_silent_initialization() {
    let temp = Temp::new("missing-wal");
    let request = temp.request("request-1", "must not invent missing history");
    // First exercise a never-initialized path, then disappearance of an already
    // committed history. Both must fail without creating an empty replacement.
    for history_existed in [false, true] {
        if history_existed {
            temp.init();
            temp.normal("commit-before-removal", &request);
            fs::copy(temp.wal(), temp.evidence.join("removed-original.wal")).unwrap();
            fs::remove_file(temp.wal()).unwrap();
        }
        assert!(!temp.wal().exists());
        temp.spawn(
            "inspect-missing",
            vec!["inspect".into(), temp.wal_arg(), RESOURCE.into()],
        )
        .finish()
        .assert_refused();
        assert!(!temp.wal().exists());
        temp.worker("worker-missing", &request, "normal")
            .finish()
            .assert_refused();
        assert!(
            !temp.wal().exists(),
            "missing history must not be replaced by an empty store"
        );
    }
}

#[test]
fn undelegated_scope_operation_and_wrong_resource_cannot_write() {
    let temp = Temp::new("request-policy-refusals");
    temp.init();
    let request = temp.request("request-1", "only the declared scope may persist");
    let original: Value = serde_json::from_slice(&fs::read(&request).unwrap()).unwrap();
    let before = fs::read(temp.wal()).unwrap();
    for (field, offered) in [
        ("scope", "poc010:delete"),
        ("operation", "delete"),
        ("resource", "another-vault"),
    ] {
        let mut changed = original.clone();
        changed[field] = json!(offered);
        let path = temp.request_value(changed);
        let output = temp
            .worker(&format!("invalid-{field}"), &path, "normal")
            .finish();
        output.assert_refused();
        if field != "resource" {
            assert!(
                !output
                    .events
                    .iter()
                    .any(|value| value["event"] == "authorization_issued"),
                "invalid policy request must fail before permit issuance"
            );
        }
        assert_eq!(fs::read(temp.wal()).unwrap(), before);
    }
    assert_eq!(temp.inspect()["effect_count"], 0);
}

#[test]
fn five_cycle_continuity_across_concurrency_crashes_and_historical_retry() {
    let temp = Temp::new("five-cycle-continuity");
    temp.init();
    let first_request = temp.request("r1", "first local persistent effect");
    let mut cycles = Vec::new();

    // Cycle 1: independently issued permits race for one canonical action.
    let (first, second) = race(&temp, &first_request, "cycle-1");
    let first_receipt = first.result()["receipt_sha256"].clone();
    cycles.push(json!({"cycle":1,"scenario":"two independently issued permits, synchronized workers",
        "worker_results":[first.result(),second.result()],"observed_effect_count":temp.inspect()["effect_count"]}));

    // Cycle 2: another process reissues authorization and retries the same action.
    let before = fs::read(temp.wal()).unwrap();
    let retry = temp.normal("cycle-2-retry-r1", &first_request);
    assert_eq!(retry.result()["status"], "AlreadyCommitted");
    assert_eq!(retry.result()["receipt_sha256"], first_receipt);
    assert_eq!(fs::read(temp.wal()).unwrap(), before);
    cycles.push(
        json!({"cycle":2,"scenario":"fresh process exact retry","result":retry.result(),
        "wal_unchanged":true,"observed_effect_count":temp.inspect()["effect_count"]}),
    );

    // Cycle 3: death before commit creates no effect and must not lose the action.
    let second_request = temp.request("r2", "second local persistent effect");
    let before = fs::read(temp.wal()).unwrap();
    let mut child = temp.worker(
        "cycle-3-before-commit",
        &second_request,
        "pause-before-commit",
    );
    child.until("before_commit");
    assert!(child
        .events
        .iter()
        .any(|value| value["event"] == "authorization_verified"));
    let killed = child.kill();
    assert_eq!(fs::read(temp.wal()).unwrap(), before);
    let after_before_commit_kill = temp.inspect()["effect_count"].clone();
    assert_eq!(after_before_commit_kill, 1);
    let recovered = temp.normal("cycle-3-retry-r2", &second_request);
    assert_eq!(recovered.result()["status"], "Committed");
    assert_eq!(recovered.result()["effect_count"], 2);
    cycles.push(json!({"cycle":3,"scenario":"kill before commit then fresh retry",
        "crash_events":killed.events,"after_kill_effect_count":after_before_commit_kill,"result":recovered.result(),
        "observed_effect_count":temp.inspect()["effect_count"]}));

    // Cycle 4: the effect AND completion receipt are durable; only the caller
    // acknowledgement is missing. This is the local transaction crash window.
    let third_request = temp.request("r3", "third local persistent effect");
    let mut child = temp.worker("cycle-4-after-commit", &third_request, "pause-after-commit");
    let durable_event = child.until("effect_durable_before_ack");
    let killed = child.kill();
    let after_durable_kill = temp.inspect()["effect_count"].clone();
    assert_eq!(after_durable_kill, 3);
    let after_effect = fs::read(temp.wal()).unwrap();
    let reconciled = temp.normal("cycle-4-retry-r3", &third_request);
    assert_eq!(reconciled.result()["status"], "AlreadyCommitted");
    assert_eq!(reconciled.result()["effect_count"], 3);
    assert_eq!(
        reconciled.result()["receipt_sha256"],
        durable_event["receipt_sha256"]
    );
    assert_eq!(
        reconciled.result()["action_sha256"],
        durable_event["action_sha256"]
    );
    assert_eq!(fs::read(temp.wal()).unwrap(), after_effect);
    cycles.push(json!({"cycle":4,"scenario":"kill after durable local effect and receipt, before caller acknowledgement",
        "durable_event":durable_event,"crash_events":killed.events,"after_kill_effect_count":after_durable_kill,
        "result":reconciled.result(),"wal_unchanged_on_retry":true,"observed_effect_count":temp.inspect()["effect_count"]}));

    // Cycle 5: the first receipt remains authoritative after later commits;
    // changed input under that same logical identity remains a conflict.
    let before = fs::read(temp.wal()).unwrap();
    let historical = temp.normal("cycle-5-historical-r1", &first_request);
    assert_eq!(historical.result()["status"], "AlreadyCommitted");
    assert_eq!(historical.result()["receipt_sha256"], first_receipt);
    assert_eq!(historical.result()["effect_count"], 3);
    temp.worker(
        "cycle-5-changed-r1",
        &temp.request("r1", "changed first payload"),
        "normal",
    )
    .finish()
    .assert_refused();
    assert_eq!(fs::read(temp.wal()).unwrap(), before);
    assert_eq!(temp.inspect()["effect_count"], 3);
    cycles.push(
        json!({"cycle":5,"scenario":"historical exact retry and changed payload refusal",
        "result":historical.result(),"changed_payload_refused":true,"wal_unchanged":true,
        "observed_effect_count":temp.inspect()["effect_count"]}),
    );

    fs::copy(temp.wal(), temp.evidence.join("five-cycle-final.wal")).unwrap();
    fs::write(temp.evidence.join("five-cycle-results.json"), serde_json::to_vec_pretty(&json!({
        "boundary":"local transactional append; effect and completion receipt share one durable record",
        "same_store_and_principal_resource_scope_context":true,"cycles":cycles,
        "final_physical_effect_count":temp.inspect()["effect_count"],
        "external_nontransactional_exactly_once_claimed":false
    })).unwrap()).unwrap();
}
