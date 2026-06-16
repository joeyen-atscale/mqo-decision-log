use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::log_store::LogStore;
use crate::query::{run_query, QueryFilter};

#[derive(Debug, Serialize, Deserialize)]
pub struct Summary {
    pub total_questions: usize,
    pub answered_count: usize,
    pub clarify_count: usize,
    pub blocked_count: usize,
    pub clarify_rate: f64,
    pub block_rate: f64,
    pub pillar_fire_frequency: HashMap<String, usize>,
    pub total_budget_consumed: f64,
    /// The --since filter that was applied (if any)
    pub since_filter: Option<String>,
}

pub fn compute_summary(store: &LogStore, since: Option<&str>) -> Result<Summary> {
    let filter = QueryFilter {
        session: None,
        since: since.map(|s| s.to_string()),
        outcome: None,
    };
    let records = run_query(store, &filter)?;

    let total = records.len();
    let mut answered = 0usize;
    let mut clarify = 0usize;
    let mut blocked = 0usize;
    let mut pillar_freq: HashMap<String, usize> = HashMap::new();
    let mut total_budget = 0.0f64;

    for rec in &records {
        match rec.get("outcome").and_then(|v| v.as_str()).unwrap_or("") {
            "answered" => answered += 1,
            "clarify" => clarify += 1,
            "blocked" => blocked += 1,
            _ => {}
        }
        if let Some(pillars) = rec.get("pillars_fired").and_then(|v| v.as_array()) {
            for p in pillars {
                if let Some(name) = p.as_str() {
                    *pillar_freq.entry(name.to_string()).or_insert(0) += 1;
                }
            }
        }
        if let Some(budget) = rec.get("budget_consumed").and_then(|v| v.as_f64()) {
            total_budget += budget;
        }
    }

    let clarify_rate = if total > 0 { clarify as f64 / total as f64 } else { 0.0 };
    let block_rate = if total > 0 { blocked as f64 / total as f64 } else { 0.0 };

    Ok(Summary {
        total_questions: total,
        answered_count: answered,
        clarify_count: clarify,
        blocked_count: blocked,
        clarify_rate,
        block_rate,
        pillar_fire_frequency: pillar_freq,
        total_budget_consumed: total_budget,
        since_filter: since.map(|s| s.to_string()),
    })
}
