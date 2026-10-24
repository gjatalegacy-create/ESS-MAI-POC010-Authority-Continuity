//! New bounded adapter following v189's validate -> identity/conflict check ->
//! durable transaction -> materialized receipt order. The local append IS the
//! observable effect. No remote effect or native knowledge admission is claimed.
use crate::{digest, hex, Request, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

const MAGIC: &[u8; 8] = b"POC010\0\x01";
const HEADER: usize = 40;
const MAX_FRAME: usize = 256 * 1024;
const MAX_WAL: u64 = 64 * 1024 * 1024;

struct Record {
    request: Request,
    receipt: [u8; 32],
}
struct Replay {
    entries: BTreeMap<[u8; 32], Record>,
    head: [u8; 32],
    physical_effects: u64,
}

fn io_error(e: std::io::Error) -> String {
    format!("storage: {e}")
}

fn validate_authorization_trace(auth: &Value) -> Result<()> {
    if auth.as_object().map(|o| o.len()) != Some(3)
        || auth["issuer_instance"].as_str().is_none_or(str::is_empty)
        || auth["generation"].as_u64().is_none_or(|n| n == 0)
        || auth["nonce"].as_u64().is_none()
    {
        return Err("authorization trace missing".into());
    }
    Ok(())
}

pub fn initialize(path: &Path, resource: &str) -> Result<Value> {
    if resource.is_empty() || resource.len() > 16_384 {
        return Err("invalid resource".into());
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(path)
        .map_err(io_error)?;
    file.lock().map_err(io_error)?;
    file.write_all(MAGIC).map_err(io_error)?;
    file.write_all(&digest(&[
        b"ESSMAI_POC010_RESOURCE_V1",
        resource.as_bytes(),
    ]))
    .map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    Ok(json!({"event":"result","status":"Initialized","resource":resource,"effect_count":0}))
}

fn open(path: &Path, resource: &str, write: bool) -> Result<(File, Replay)> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(write)
        .open(path)
        .map_err(io_error)?;
    if write {
        file.lock().map_err(io_error)?;
    } else {
        file.lock_shared().map_err(io_error)?;
    }
    if file.metadata().map_err(io_error)?.len() > MAX_WAL {
        return Err("bounded WAL size exceeded".into());
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(io_error)?;
    let replay = decode(&bytes, resource)?;
    Ok((file, replay))
}

fn decode(bytes: &[u8], resource: &str) -> Result<Replay> {
    if bytes.len() < HEADER
        || &bytes[..8] != MAGIC
        || bytes[8..HEADER] != digest(&[b"ESSMAI_POC010_RESOURCE_V1", resource.as_bytes()])
    {
        return Err("invalid header or resource binding".into());
    }
    let mut replay = Replay {
        entries: BTreeMap::new(),
        head: digest(&[&bytes[..HEADER]]),
        physical_effects: 0,
    };
    let mut at = HEADER;
    while at < bytes.len() {
        if bytes.len() - at < 36 {
            return Err("incomplete frame: evidence preserved; no automatic repair".into());
        }
        let length = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        if length == 0 || length > MAX_FRAME || bytes.len() - at - 36 < length {
            return Err("invalid or incomplete frame length".into());
        }
        let stored: [u8; 32] = bytes[at + 4..at + 36].try_into().unwrap();
        let body = &bytes[at + 36..at + 36 + length];
        let computed = digest(&[
            b"ESSMAI_POC010_TX_V1",
            &replay.head,
            &(length as u32).to_le_bytes(),
            body,
        ]);
        if stored != computed {
            return Err("frame SHA-256 or chain mismatch".into());
        }
        let value: Value =
            serde_json::from_slice(body).map_err(|e| format!("invalid record: {e}"))?;
        if value.as_object().map(|o| o.len()) != Some(5)
            || value["version"] != 1
            || value["kind"] != "claim_effect_commit"
            || value["sequence"] != replay.physical_effects + 1
        {
            return Err("record schema or sequence mismatch".into());
        }
        let request = Request::from_value(&value["request"])?;
        if request.resource != resource {
            return Err("record resource mismatch".into());
        }
        validate_authorization_trace(&value["authorization"])?;
        if serde_json::to_vec(&value).map_err(|e| e.to_string())? != body {
            return Err("noncanonical record encoding".into());
        }
        let key = digest(&[&request.identity_bytes()]);
        if replay.entries.contains_key(&key) {
            return Err("duplicate physical execution claim or identity hash collision".into());
        }
        replay.entries.insert(
            key,
            Record {
                request,
                receipt: computed,
            },
        );
        replay.physical_effects += 1;
        replay.head = computed;
        at += 36 + length;
    }
    Ok(replay)
}

pub fn inspect(path: &Path, resource: &str) -> Result<Value> {
    let (_file, replay) = open(path, resource, false)?;
    Ok(
        json!({"event":"result","status":"Inspected","effect_count":replay.physical_effects,
        "head_sha256":hex(&replay.head),"entries":replay.entries.values().map(|entry|json!({
            "action_sha256":entry.request.action_sha256(),"request_id":entry.request.request_id,
            "payload_sha256":hex(&digest(&[entry.request.payload.as_bytes()])),"receipt_sha256":hex(&entry.receipt)
        })).collect::<Vec<_>>()}),
    )
}

pub(crate) fn execute(
    path: &Path,
    request: &Request,
    authorization: Value,
    mut trace: impl FnMut(Value) -> Result<()>,
) -> Result<Value> {
    validate_authorization_trace(&authorization)?;
    let (mut file, replay) = open(path, &request.resource, true)?;
    let identity = request.identity_bytes();
    let key = digest(&[&identity]);
    trace(
        json!({"event":"storage_verified","action_sha256":hex(&key),"effect_count":replay.physical_effects,"head_sha256":hex(&replay.head)}),
    )?;
    if let Some(prior) = replay.entries.get(&key) {
        if prior.request.identity_bytes() != identity
            || prior.request.payload.as_bytes() != request.payload.as_bytes()
        {
            return Err(
                "ConflictingReplay: same logical identity with different immutable material".into(),
            );
        }
        // A complete frame may survive a writer that died before its sync.
        // Reconciliation must establish durability before acknowledging it.
        file.sync_all().map_err(io_error)?;
        trace(
            json!({"event":"atomic_claim","outcome":"existing","action_sha256":hex(&key),"new_effect":false}),
        )?;
        return Ok(
            json!({"event":"result","status":"AlreadyCommitted","action_sha256":hex(&key),
            "receipt_sha256":hex(&prior.receipt),"effect_count":replay.physical_effects}),
        );
    }
    trace(
        json!({"event":"before_commit","action_sha256":hex(&key),"effect_count":replay.physical_effects}),
    )?;
    let body = serde_json::to_vec(
        &json!({"version":1,"kind":"claim_effect_commit","sequence":replay.physical_effects+1,
        "request":request.value(),"authorization":authorization}),
    )
    .map_err(|e| e.to_string())?;
    if body.len() > MAX_FRAME {
        return Err("frame size exceeded".into());
    }
    let length = (body.len() as u32).to_le_bytes();
    let receipt = digest(&[b"ESSMAI_POC010_TX_V1", &replay.head, &length, &body]);
    let mut frame = Vec::with_capacity(36 + body.len());
    frame.extend_from_slice(&length);
    frame.extend_from_slice(&receipt);
    frame.extend_from_slice(&body);
    if file.metadata().map_err(io_error)?.len() + frame.len() as u64 > MAX_WAL {
        return Err("bounded WAL size exceeded".into());
    }
    file.seek(SeekFrom::End(0)).map_err(io_error)?;
    file.write_all(&frame).map_err(io_error)?;
    file.flush().map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    // This is the local transaction's durable linearization point. The claim,
    // payload and receipt coexist in the same frame, before acknowledgement.
    trace(
        json!({"event":"atomic_claim","outcome":"committed","action_sha256":hex(&key),"new_effect":true,"receipt_sha256":hex(&receipt)}),
    )?;
    trace(
        json!({"event":"effect_durable_before_ack","action_sha256":hex(&key),"effect_count":replay.physical_effects+1,"receipt_sha256":hex(&receipt)}),
    )?;
    Ok(
        json!({"event":"result","status":"Committed","action_sha256":hex(&key),
        "receipt_sha256":hex(&receipt),"effect_count":replay.physical_effects+1}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::sample;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn path() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "poc010-ledger-{}-{}-{}.wal",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        initialize(&path, "test-vault").unwrap();
        path
    }
    fn commit(path: &Path, request: &Request) -> Value {
        execute(
            path,
            request,
            json!({"issuer_instance":"unit-fixture","generation":1,"nonce":2}),
            |_| Ok(()),
        )
        .unwrap()
    }
    #[test]
    fn retry_is_byte_stable_and_returns_original_receipt() {
        let p = path();
        let req = sample("one", "bytes, punctuation preserved");
        let first = commit(&p, &req);
        let before = std::fs::read(&p).unwrap();
        let retry = commit(&p, &req);
        assert_eq!(retry["status"], "AlreadyCommitted");
        assert_eq!(first["receipt_sha256"], retry["receipt_sha256"]);
        assert_eq!(before, std::fs::read(&p).unwrap());
        assert_eq!(inspect(&p, "test-vault").unwrap()["effect_count"], 1);
    }
    #[test]
    fn same_key_different_material_fails_and_preserves_file() {
        let p = path();
        commit(&p, &sample("one", "a"));
        let before = std::fs::read(&p).unwrap();
        assert!(execute(
            &p,
            &sample("one", "b"),
            json!({"issuer_instance":"unit","generation":1,"nonce":2}),
            |_| Ok(())
        )
        .unwrap_err()
        .contains("ConflictingReplay"));
        assert_eq!(before, std::fs::read(&p).unwrap());
    }
    #[test]
    fn full_corruption_and_torn_tail_fail_closed_without_repair() {
        for append in [false, true] {
            let p = path();
            commit(&p, &sample("one", "a"));
            let mut bytes = std::fs::read(&p).unwrap();
            if append {
                bytes.extend_from_slice(&[1, 2, 3]);
            } else {
                let n = bytes.len();
                bytes[n - 1] ^= 1;
            }
            std::fs::write(&p, &bytes).unwrap();
            assert!(inspect(&p, "test-vault").is_err());
            assert!(execute(
                &p,
                &sample("two", "a"),
                json!({"issuer_instance":"unit","generation":1,"nonce":2}),
                |_| Ok(())
            )
            .is_err());
            assert_eq!(bytes, std::fs::read(&p).unwrap());
        }
    }
    #[test]
    fn wrong_resource_and_accidental_reinitialization_fail() {
        let p = path();
        let bytes = std::fs::read(&p).unwrap();
        assert!(inspect(&p, "other-vault").is_err());
        assert!(initialize(&p, "test-vault").is_err());
        assert_eq!(bytes, std::fs::read(&p).unwrap());
    }
    #[test]
    fn incomplete_authorization_trace_cannot_create_an_unreadable_record() {
        let p = path();
        let before = std::fs::read(&p).unwrap();
        assert!(execute(&p, &sample("one", "a"), json!({}), |_| Ok(())).is_err());
        assert_eq!(before, std::fs::read(&p).unwrap());
        assert_eq!(inspect(&p, "test-vault").unwrap()["effect_count"], 0);
    }
    #[test]
    fn trace_error_before_commit_cannot_write_and_after_commit_reconciles() {
        let p = path();
        let req = sample("one", "x");
        let auth = json!({"issuer_instance":"unit","generation":1,"nonce":2});
        assert!(execute(&p, &req, auth.clone(), |e| {
            if e["event"] == "before_commit" {
                Err("interrupted".into())
            } else {
                Ok(())
            }
        })
        .is_err());
        assert_eq!(inspect(&p, "test-vault").unwrap()["effect_count"], 0);
        assert!(execute(&p, &req, auth, |e| {
            if e["event"] == "effect_durable_before_ack" {
                Err("interrupted".into())
            } else {
                Ok(())
            }
        })
        .is_err());
        assert_eq!(commit(&p, &req)["status"], "AlreadyCommitted");
        assert_eq!(inspect(&p, "test-vault").unwrap()["effect_count"], 1);
    }
    #[test]
    fn rehashed_duplicate_physical_claim_is_still_refused() {
        let p = path();
        commit(&p, &sample("one", "a"));
        let mut bytes = std::fs::read(&p).unwrap();
        let previous: [u8; 32] = bytes[HEADER + 4..HEADER + 36].try_into().unwrap();
        let mut value: Value = serde_json::from_slice(&bytes[HEADER + 36..]).unwrap();
        value["sequence"] = json!(2);
        let body = serde_json::to_vec(&value).unwrap();
        let length = (body.len() as u32).to_le_bytes();
        let sha = digest(&[b"ESSMAI_POC010_TX_V1", &previous, &length, &body]);
        bytes.extend_from_slice(&length);
        bytes.extend_from_slice(&sha);
        bytes.extend_from_slice(&body);
        std::fs::write(&p, &bytes).unwrap();
        assert!(inspect(&p, "test-vault")
            .unwrap_err()
            .contains("duplicate physical execution claim"));
        assert_eq!(bytes, std::fs::read(&p).unwrap());
    }
    #[test]
    fn valid_older_prefix_is_accepted_declared_freshness_limit() {
        let p = path();
        commit(&p, &sample("one", "a"));
        let older = std::fs::read(&p).unwrap();
        commit(&p, &sample("two", "b"));
        assert_eq!(inspect(&p, "test-vault").unwrap()["effect_count"], 2);
        std::fs::write(&p, &older).unwrap();
        assert_eq!(inspect(&p, "test-vault").unwrap()["effect_count"], 1);
    }
    #[test]
    fn repeated_process_local_admission_does_not_define_semantic_idempotency() {
        let request = sample("one", "a");
        let binding = request.binding_bytes();
        let mut gate = crate::source_gate::BoundGate::new();
        let first = gate.issue(&binding);
        let second = gate.issue(&binding);
        gate.consume(first, &binding).unwrap();
        gate.consume(second, &binding).unwrap();
        let p = path();
        let first = commit(&p, &request);
        let second = commit(&p, &request);
        assert_eq!(first["status"], "Committed");
        assert_eq!(second["status"], "AlreadyCommitted");
        assert_eq!(inspect(&p, "test-vault").unwrap()["effect_count"], 1);
    }
}
