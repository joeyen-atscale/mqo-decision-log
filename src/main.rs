use anyhow::Result;
use clap::Parser;
use mqo_decision_log::cli::{Cli, Command};
use mqo_decision_log::{cli, serve};

fn main() -> Result<()> {
    let c = Cli::parse();
    let log_path = c.log.as_deref().unwrap_or("mqo-decisions.jsonl");

    match c.command {
        Command::Append { session, record } => {
            cli::run_append(log_path, &session, &record)
        }
        Command::Query { session, since, outcome } => {
            cli::run_query(log_path, session.as_deref(), since.as_deref(), outcome.as_deref())
        }
        Command::Summary { since } => {
            cli::run_summary(log_path, since.as_deref())
        }
        Command::Verify => {
            cli::run_verify(log_path)
        }
        Command::Serve => {
            serve::run_serve(log_path)
        }
    }
}
