use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// The canonical decision record schema.
/// Shared with mqo-agent's answer.json so the agent can emit directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    /// ISO-8601 / RFC-3339 timestamp
    pub ts: String,
    /// Session identifier
    pub session: String,
    /// The question that was posed
    pub question: String,
    /// Ordered list of plan steps
    #[serde(default)]
    pub plan: Vec<String>,
    /// Access policy verdict
    pub access_verdict: String,
    /// Budget consumed (arbitrary unit, e.g. tokens or cost)
    pub budget_consumed: f64,
    /// Pillars that fired during this decision
    #[serde(default)]
    pub pillars_fired: Vec<String>,
    /// Outcome: "answered" | "clarify" | "blocked"
    pub outcome: String,
    /// Optional credential id from rosetta-credential
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_id: Option<String>,
}

/// Validate that a JSON value conforms to the minimum required schema.
pub fn validate_record(v: &serde_json::Value) -> Result<()> {
    for required in &["ts", "session", "question", "access_verdict", "budget_consumed", "outcome"] {
        if v.get(required).is_none() {
            bail!("record missing required field: {}", required);
        }
    }
    let outcome = v["outcome"].as_str().unwrap_or("");
    if !matches!(outcome, "answered" | "clarify" | "blocked") {
        bail!("record.outcome must be one of: answered, clarify, blocked (got {:?})", outcome);
    }
    Ok(())
}
