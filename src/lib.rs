//! Bounded local persistence experiment. No constitutional verdict is minted.
mod ledger;
mod source_gate;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub use ledger::{initialize, inspect};

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    principal: String,
    operation: String,
    resource: String,
    scope: String,
    context: String,
    request_id: String,
    payload: String,
}

impl Request {
    pub fn from_value(value: &Value) -> Result<Self> {
        let object = value.as_object().ok_or("request must be an object")?;
        if object.len() != 7 {
            return Err("request must contain exactly seven declared fields".into());
        }
        let field = |name: &str| -> Result<String> {
            let text = object
                .get(name)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("missing string {name}"))?;
            if text.is_empty() || text.len() > 16_384 {
                return Err(format!("empty or oversized {name}"));
            }
            Ok(text.to_owned())
        };
        let request = Self {
            principal: field("principal")?,
            operation: field("operation")?,
            resource: field("resource")?,
            scope: field("scope")?,
            context: field("context")?,
            request_id: field("request_id")?,
            payload: field("payload")?,
        };
        if request.operation != "persist" || request.scope != "poc010:persist" {
            return Err("operation or scope outside the local fixture contract".into());
        }
        Ok(request)
    }

    fn value(&self) -> Value {
        json!({"principal":self.principal,"operation":self.operation,"resource":self.resource,
            "scope":self.scope,"context":self.context,"request_id":self.request_id,"payload":self.payload})
    }

    fn identity_bytes(&self) -> Vec<u8> {
        let mut bytes = b"ESSMAI_POC010_ACTION_V1".to_vec();
        for field in [
            &self.principal,
            &self.operation,
            &self.resource,
            &self.scope,
            &self.context,
            &self.request_id,
        ] {
            put(&mut bytes, field.as_bytes());
        }
        bytes
    }

    fn binding_bytes(&self) -> Vec<u8> {
        let mut bytes = self.identity_bytes();
        put(&mut bytes, self.payload.as_bytes());
        bytes
    }

    pub fn action_sha256(&self) -> String {
        hex(&digest(&[&self.identity_bytes()]))
    }
}

fn put(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value);
}

fn digest(parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update(part);
    }
    hash.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn emit(value: Value) -> Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    serde_json::to_writer(&mut out, &value).map_err(|e| e.to_string())?;
    writeln!(out).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())
}

fn pause() -> Result<()> {
    let mut line = String::new();
    if io::stdin()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?
        == 0
    {
        return Err("control pipe closed before release".into());
    }
    Ok(())
}

/// A test principal is admitted by the declared, bounded fixture policy. This
/// does not authenticate a remote caller or substitute for native Shadow checks.
pub fn worker(path: &Path, request: Request, worker_name: &str, mode: &str) -> Result<Value> {
    if ![
        "normal",
        "barrier",
        "pause-before-commit",
        "pause-after-commit",
        "wrong-binding",
        "wrong-nonce",
        "unknown-handle",
        "consumed",
    ]
    .contains(&mode)
    {
        return Err("unknown worker mode".into());
    }
    if worker_name.is_empty() || worker_name.len() > 256 {
        return Err("invalid worker name".into());
    }
    let issuer = format!(
        "{}:{}:{}",
        worker_name,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let binding = request.binding_bytes();
    let mut gate = source_gate::BoundGate::new();
    let mut handle = gate.issue(&binding);
    let authorization =
        json!({"issuer_instance":issuer,"generation":handle.gen,"nonce":handle.nonce});
    emit(
        json!({"event":"authorization_issued","action_sha256":request.action_sha256(),
        "authorization_id":format!("{}:{}",issuer,handle.gen),"generation":handle.gen,"nonce":handle.nonce,
        "authority_scope":"trusted local test issuer; process-local source gate"}),
    )?;
    match mode {
        "wrong-nonce" => handle.nonce ^= 1,
        "unknown-handle" => handle.gen = u64::MAX,
        "consumed" => gate.consume(handle, &binding)?,
        _ => {}
    }
    let mut offered_binding = binding;
    if mode == "wrong-binding" {
        offered_binding.push(0);
    }
    gate.consume(handle, &offered_binding)?;
    emit(json!({"event":"authorization_verified","consumed":true,
        "authorization":authorization,"action_sha256":request.action_sha256()}))?;
    if mode == "barrier" {
        emit(
            json!({"event":"ready","worker":worker_name,"action_sha256":request.action_sha256()}),
        )?;
        pause()?;
    }
    // The non-copy source token ends at its boundary. Storage independently
    // checks its own immutable contract; cached results never bypass consumption.
    ledger::execute(path, &request, authorization, |event| {
        let stage = event["event"].as_str().unwrap_or("").to_owned();
        emit(event)?;
        if (stage == "before_commit" && mode == "pause-before-commit")
            || (stage == "effect_durable_before_ack" && mode == "pause-after-commit")
        {
            pause()?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample(id: &str, payload: &str) -> Request {
        Request::from_value(
            &json!({"principal":"test-principal","operation":"persist","resource":"test-vault",
            "scope":"poc010:persist","context":"same-topic","request_id":id,"payload":payload}),
        )
        .unwrap()
    }

    #[test]
    fn logical_identity_excludes_payload_but_binding_does_not() {
        let a = sample("1", "red");
        let b = sample("1", "blue");
        assert_eq!(a.action_sha256(), b.action_sha256());
        assert_ne!(a.binding_bytes(), b.binding_bytes());
    }

    #[test]
    fn each_identity_field_is_bound_and_delimiters_do_not_alias() {
        let a = sample("1", "x");
        for field in ["principal", "resource", "context", "request_id"] {
            let mut value = a.value();
            value[field] = json!(format!("{}:changed", value[field].as_str().unwrap()));
            assert_ne!(
                a.action_sha256(),
                Request::from_value(&value).unwrap().action_sha256()
            );
        }
        let mut b = a.clone();
        b.principal = "ab".into();
        b.context = "c".into();
        let mut c = b.clone();
        c.principal = "a".into();
        c.context = "bc".into();
        assert_ne!(b.identity_bytes(), c.identity_bytes());
    }

    #[test]
    fn unauthorized_operation_scope_empty_or_unknown_fields_refused() {
        for field in ["operation", "scope", "principal"] {
            let mut value = sample("1", "x").value();
            value[field] = json!("");
            assert!(Request::from_value(&value).is_err());
        }
        let mut value = sample("1", "x").value();
        value["operation"] = json!("delete");
        assert!(Request::from_value(&value).is_err());
        let mut value = sample("1", "x").value();
        value["extra"] = json!(1);
        assert!(Request::from_value(&value).is_err());
    }
}
