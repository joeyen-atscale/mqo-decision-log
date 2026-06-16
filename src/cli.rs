use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::log_store::LogStore;
use crate::query::QueryFilter;
use crate::summary::compute_summary;
use crate::verify::verify_log;

#[derive(Parser)]
#[command(
    name = "mqo-decision-log",
    version,
    about = "Durable, auditable append-only log of every mqo-agent decision",
    long_about = None,
)]
pub struct Cli {
    /// Path to the JSONL log file (default: mqo-decisions.jsonl)
    #[arg(long, short = 'l', global = true)]
    pub log: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Append one decision record to the log (record is a JSON file path or '-' for stdin)
    Append {
        /// Session identifier
        #[arg(long)]
        session: String,
        /// Path to the JSON record file (or '-' for stdin)
        #[arg(long)]
        record: String,
    },
    /// Query log records with optional filters; emits matching records as JSONL
    Query {
        /// Filter by session id
        #[arg(long)]
        session: Option<String>,
        /// Filter by minimum timestamp (RFC3339)
        #[arg(long)]
        since: Option<String>,
        /// Filter by outcome (answered|clarify|blocked)
        #[arg(long)]
        outcome: Option<String>,
    },
    /// Emit aggregate summary counts as JSON (for mqo-scorecard)
    Summary {
        /// Only include records at or after this timestamp (RFC3339)
        #[arg(long)]
        since: Option<String>,
    },
    /// Verify log integrity: append-only, monotonic timestamps, provfs xattr
    Verify,
    /// Run as MCP server subprocess exposing append/query/summary tools
    Serve,
}

pub fn run_append(log_path: &str, session: &str, record_arg: &str) -> Result<()> {
    let json_str = if record_arg == "-" {
        use std::io::Read;
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        s
    } else {
        std::fs::read_to_string(record_arg)?
    };

    let mut record: serde_json::Value = serde_json::from_str(&json_str)?;

    // Ensure session field is set
    if record.get("session").is_none() {
        record["session"] = serde_json::Value::String(session.to_string());
    }

    // Validate schema minimally
    crate::record::validate_record(&record)?;

    let mut store = LogStore::open(log_path)?;
    store.append(&record)?;
    Ok(())
}

pub fn run_query(
    log_path: &str,
    session: Option<&str>,
    since: Option<&str>,
    outcome: Option<&str>,
) -> Result<()> {
    let filter = QueryFilter {
        session: session.map(|s| s.to_string()),
        since: since.map(|s| s.to_string()),
        outcome: outcome.map(|s| s.to_string()),
    };
    let store = LogStore::open(log_path)?;
    let records = crate::query::run_query(&store, &filter)?;
    for rec in &records {
        println!("{}", serde_json::to_string(rec)?);
    }
    Ok(())
}

pub fn run_summary(log_path: &str, since: Option<&str>) -> Result<()> {
    let store = LogStore::open(log_path)?;
    let summary = compute_summary(&store, since)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

pub fn run_verify(log_path: &str) -> Result<()> {
    let result = verify_log(log_path)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    if result.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
