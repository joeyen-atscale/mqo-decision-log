use anyhow::Result;
use crate::log_store::LogStore;

pub struct QueryFilter {
    pub session: Option<String>,
    pub since: Option<String>,
    pub outcome: Option<String>,
}

pub fn run_query(store: &LogStore, filter: &QueryFilter) -> Result<Vec<serde_json::Value>> {
    let records = store.read_all()?;
    let mut result = Vec::new();

    for rec in records {
        if let Some(ref sess) = filter.session {
            let rec_sess = rec.get("session").and_then(|v| v.as_str()).unwrap_or("");
            if rec_sess != sess.as_str() {
                continue;
            }
        }
        if let Some(ref since) = filter.since {
            let rec_ts = rec.get("ts").and_then(|v| v.as_str()).unwrap_or("");
            // Lexicographic comparison works for ISO-8601
            if rec_ts < since.as_str() {
                continue;
            }
        }
        if let Some(ref outcome) = filter.outcome {
            let rec_outcome = rec.get("outcome").and_then(|v| v.as_str()).unwrap_or("");
            if rec_outcome != outcome.as_str() {
                continue;
            }
        }
        result.push(rec);
    }

    Ok(result)
}
