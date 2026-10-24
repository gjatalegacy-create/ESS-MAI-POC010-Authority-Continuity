use poc010_action_continuity::{emit, initialize, inspect, worker, Request, Result};
use serde_json::json;
use std::path::Path;

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let value=match args.get(1).map(String::as_str) {
        Some("init") if args.len()==4 => initialize(Path::new(&args[2]),&args[3])?,
        Some("inspect") if args.len()==4 => inspect(Path::new(&args[2]),&args[3])?,
        Some("worker") if args.len()==5 || args.len()==6 => {
            if std::fs::metadata(&args[3]).map_err(|e|e.to_string())?.len()>128*1024 { return Err("request file too large".into()); }
            let raw=std::fs::read(&args[3]).map_err(|e|e.to_string())?;
            let value=serde_json::from_slice(&raw).map_err(|e|e.to_string())?;
            worker(Path::new(&args[2]),Request::from_value(&value)?,&args[4],args.get(5).map(String::as_str).unwrap_or("normal"))?
        },
        _ => return Err("usage: poc010-action-continuity init WAL RESOURCE | inspect WAL RESOURCE | worker WAL REQUEST_JSON WORKER [MODE]".into())
    };
    emit(value)
}

fn main() {
    if let Err(reason) = run() {
        eprintln!(
            "{}",
            json!({"event":"error","reason":reason,"knowledge_verdict":null})
        );
        std::process::exit(2);
    }
}
