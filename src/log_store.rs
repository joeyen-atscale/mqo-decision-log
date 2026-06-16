use anyhow::{Context, Result};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// Append-only JSONL log store.
/// Invariants:
///   - append is the only mutation (never rewrite, never delete)
///   - each line is a valid JSON object
pub struct LogStore {
    path: PathBuf,
    /// Snapshot of raw bytes at open time (for append-only verification)
    snapshot_len: u64,
}

impl LogStore {
    /// Open (or create) the log at `path`.
    pub fn open(path: &str) -> Result<Self> {
        let path = PathBuf::from(path);
        // Create if absent, but do not truncate
        if !path.exists() {
            std::fs::File::create(&path)
                .with_context(|| format!("creating log file {:?}", path))?;
        }
        let meta = std::fs::metadata(&path)?;
        let snapshot_len = meta.len();
        Ok(Self { path, snapshot_len })
    }

    /// Append one JSON record as a single JSONL line.
    /// Panics (via anyhow) if the file was mutated since open (snapshot_len check).
    pub fn append(&mut self, record: &serde_json::Value) -> Result<()> {
        // Re-check file length to ensure no mutation happened since open
        let current_len = std::fs::metadata(&self.path)?.len();
        if current_len != self.snapshot_len {
            anyhow::bail!(
                "log file was modified between open and append (expected {} bytes, got {})",
                self.snapshot_len,
                current_len
            );
        }

        let line = serde_json::to_string(record)?;
        let mut f = OpenOptions::new().append(true).open(&self.path)
            .with_context(|| format!("opening {:?} for append", self.path))?;
        writeln!(f, "{}", line)?;
        self.snapshot_len = std::fs::metadata(&self.path)?.len();
        Ok(())
    }

    /// Read all records from the log. Returns an error if any line is invalid JSON.
    pub fn read_all(&self) -> Result<Vec<serde_json::Value>> {
        let f = std::fs::File::open(&self.path)
            .with_context(|| format!("opening {:?} for read", self.path))?;
        let reader = BufReader::new(f);
        let mut records = Vec::new();
        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let v: serde_json::Value = serde_json::from_str(&line)
                .with_context(|| format!("invalid JSON on line {}", i + 1))?;
            records.push(v);
        }
        Ok(records)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}
