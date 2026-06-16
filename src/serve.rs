use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

/// Minimal MCP-style stdio server exposing append/query/summary tools.
/// Protocol: newline-delimited JSON requests → newline-delimited JSON responses.
///
/// Request format:
///   {"id": <any>, "method": "append"|"query"|"summary", "params": {...}}
///
/// Response format:
///   {"id": <any>, "result": {...}} or {"id": <any>, "error": {"message": "..."}}
pub fn run_serve(log_path: &str) -> Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = BufReader::new(stdin.lock());

    eprintln!("mqo-decision-log serve: listening on stdin (MCP stdio mode)");

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = handle_request(log_path, &line);
        let out = serde_json::to_string(&response)?;
        writeln!(stdout, "{}", out)?;
        stdout.flush()?;
    }

    Ok(())
}

/// Public test hook so acceptance tests can call request handling directly.
pub fn handle_request_for_test(log_path: &str, raw: &str) -> Value {
    handle_request(log_path, raw)
}

fn handle_request(log_path: &str, raw: &str) -> Value {
    let req: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            return json!({"id": null, "error": {"message": format!("invalid JSON: {}", e)}});
        }
    };

    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(json!({}));

    match method {
        "append" => {
            let session = params.get("session").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let record = params.get("record").cloned().unwrap_or(json!({}));
            match do_append(log_path, &session, record) {
                Ok(_) => json!({"id": id, "result": {"appended": true}}),
                Err(e) => json!({"id": id, "error": {"message": e.to_string()}}),
            }
        }
        "query" => {
            let session = params.get("session").and_then(|v| v.as_str()).map(|s| s.to_string());
            let since = params.get("since").and_then(|v| v.as_str()).map(|s| s.to_string());
            let outcome = params.get("outcome").and_then(|v| v.as_str()).map(|s| s.to_string());

            use crate::log_store::LogStore;
            use crate::query::{run_query, QueryFilter};

            match LogStore::open(log_path) {
                Ok(store) => {
                    let filter = QueryFilter { session, since, outcome };
                    match run_query(&store, &filter) {
                        Ok(records) => json!({"id": id, "result": {"records": records}}),
                        Err(e) => json!({"id": id, "error": {"message": e.to_string()}}),
                    }
                }
                Err(e) => json!({"id": id, "error": {"message": e.to_string()}}),
            }
        }
        "summary" => {
            let since = params.get("since").and_then(|v| v.as_str()).map(|s| s.to_string());

            use crate::log_store::LogStore;
            use crate::summary::compute_summary;

            match LogStore::open(log_path) {
                Ok(store) => {
                    match compute_summary(&store, since.as_deref()) {
                        Ok(summary) => {
                            let v = serde_json::to_value(summary).unwrap_or(json!({}));
                            json!({"id": id, "result": v})
                        }
                        Err(e) => json!({"id": id, "error": {"message": e.to_string()}}),
                    }
                }
                Err(e) => json!({"id": id, "error": {"message": e.to_string()}}),
            }
        }
        other => {
            json!({"id": id, "error": {"message": format!("unknown method: {}", other)}})
        }
    }
}

fn do_append(log_path: &str, session: &str, mut record: Value) -> Result<()> {
    if record.get("session").is_none() {
        record["session"] = Value::String(session.to_string());
    }
    crate::record::validate_record(&record)?;
    let mut store = crate::log_store::LogStore::open(log_path)?;
    store.append(&record)
}
