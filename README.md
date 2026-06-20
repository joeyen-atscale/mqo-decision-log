# mqo-decision-log

An append-only JSONL log for `mqo-agent` decisions, with a CLI to append, query, summarize, and verify the trail.

## Why it exists

An `mqo-agent` run makes a chain of decisions per question — which pillars it fired, the access-policy verdict, the budget it consumed, the outcome it reached. Those decisions live in the process and vanish when it exits, which leaves nothing to audit and nothing for downstream tools to read. This is the sink: one structured record per run, written to a file that only ever grows. The agent's behavior becomes a history you can query instead of a thing you have to reconstruct from logs.

The record schema is the same shape `mqo-agent` already emits as its `answer.json`, so the agent appends its own output with no translation step.

## Install

```bash
cargo install --path .
```

Requires Rust 1.85 or newer. This produces the `mqo-decision-log` binary.

## Quickstart

Every command takes `-l/--log <path>` (default `mqo-decisions.jsonl`). Append a record, then read it back:

```bash
cat > rec.json <<'EOF'
{
  "ts": "2026-06-19T10:00:00Z",
  "session": "sess-1",
  "question": "revenue by region?",
  "plan": ["resolve_pillars", "check_access", "run_query"],
  "access_verdict": "allow",
  "budget_consumed": 1250.5,
  "pillars_fired": ["revenue", "region"],
  "outcome": "answered",
  "credential_id": "vc-001"
}
EOF

mqo-decision-log -l demo.jsonl append --session sess-1 --record rec.json
mqo-decision-log -l demo.jsonl query --outcome answered
```

`query` emits matching records as JSONL — one record per line, fields sorted, ready to pipe into `jq`. `summary` rolls the whole log (or a `--since` window) into one JSON object:

```bash
mqo-decision-log -l demo.jsonl summary
```

```json
{
  "total_questions": 1,
  "answered_count": 1,
  "clarify_count": 0,
  "blocked_count": 0,
  "clarify_rate": 0.0,
  "block_rate": 0.0,
  "pillar_fire_frequency": { "revenue": 1, "region": 1 },
  "total_budget_consumed": 1250.5,
  "since_filter": null
}
```

`append` reads the record from a file or from stdin (`--record -`). If the record omits `session`, the `--session` flag fills it in.

## Commands

| Command | What it does |
| --- | --- |
| `append --session <id> --record <file\|->` | Validate one record and append it as a single JSONL line. |
| `query [--session <id>] [--since <rfc3339>] [--outcome answered\|clarify\|blocked]` | Emit matching records as JSONL; filters combine with AND. |
| `summary [--since <rfc3339>]` | Aggregate counts, clarify/block rates, per-pillar fire frequency, total budget. |
| `verify` | Check log integrity; print a JSON report; exit non-zero on a violation. |
| `serve` | Run a stdin/stdout request loop exposing `append`, `query`, and `summary`. |

## Record schema

The required fields are `ts`, `session`, `question`, `access_verdict`, `budget_consumed`, and `outcome`; `plan`, `pillars_fired`, and `credential_id` are optional. `outcome` must be one of `answered`, `clarify`, or `blocked` — anything else is rejected on append.

```json
{
  "ts": "2026-06-19T10:00:00Z",
  "session": "sess-abc123",
  "question": "What is the revenue by region?",
  "plan": ["resolve_pillars", "check_access", "run_query"],
  "access_verdict": "allow",
  "budget_consumed": 1250.5,
  "pillars_fired": ["revenue", "region", "time"],
  "outcome": "answered",
  "credential_id": "vc-001"
}
```

## How it works

The store is a plain JSONL file — one JSON object per line, append the only mutation. Append never rewrites or deletes, and it refuses to write if the file changed length between open and append, so a concurrent writer can't clobber the tail.

Timestamps in records drive every read path. `query` filters on the record's own `ts` (lexicographic RFC-3339 comparison, which is why the field must be zero-padded UTC), and `summary` derives all of its aggregates from the records, not the wall clock — so the same log produces the same output on any machine, any time.

`verify` is a structural check, not a cryptographic one. It confirms every line is valid JSON, that timestamps are monotonic within each session, and that no record carries a `_tampered` marker; on a clean log it reports `ok: true`, and on a violation it names the offending line and exits non-zero. On a filesystem with the `provfs` LSM active, it also reads the `user.prov.session` xattr off the log file and reports it — and reports its absence without failing, so the check is portable to plain filesystems.

`serve` is a minimal newline-delimited request/response loop over stdin and stdout, not a full JSON-RPC/MCP implementation. Each line is `{"id": <any>, "method": "append"|"query"|"summary", "params": {...}}`; each reply is `{"id": <same>, "result": {...}}` or `{"id": <same>, "error": {"message": "..."}}`.

## Where it fits

Part of the mqo fleet. `mqo-agent` writes the records; downstream readers (`mqo-trace-harvest`, `mqo-scorecard`) consume this log — `summary` exists to feed `mqo-scorecard` trend deltas directly.

## Status

Early — `v0.1.0`. The core append/query/summary/verify path is implemented and covered by acceptance tests that run cluster-free against bundled fixtures (`tests/fixtures/`). Two limits are deliberate and worth knowing: `verify` proves structure and in-process append-only behavior, not byte-level immutability of past records, and detects tampering only via the `_tampered` marker; `serve` speaks the minimal protocol above rather than full MCP.

## License

Dual-licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
