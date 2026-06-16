# mqo-decision-log

Durable, auditable append-only log of every `mqo-agent` decision.

## Overview

`mqo-agent` makes a chain of decisions per question — which pillars it chose, the access-policy verdict, the budget consumed, the final signed credential. Those decisions vanish when the process exits. `mqo-decision-log` is the append-only sink: every agent run writes one structured, queryable decision record, so the agent's behavior over time is auditable and downstream consumers (`mqo-trace-harvest`, `mqo-scorecard`) have a real source.

## Usage

```bash
# Append a decision record
mqo-decision-log append --session <id> --record record.json

# Query records with filters
mqo-decision-log query [--session <id>] [--since <ts>] [--outcome answered|clarify|blocked]

# Aggregate summary (for mqo-scorecard trend deltas)
mqo-decision-log summary [--since <ts>]

# Verify log integrity (append-only, monotonic timestamps, provfs xattr)
mqo-decision-log verify

# Run as MCP stdio server
mqo-decision-log serve
```

## Record schema

```json
{
  "ts": "2026-06-16T10:00:00Z",
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

The schema is shared with `mqo-agent`'s `answer.json` — no translation layer required.

## Acceptance criteria

1. `append` adds exactly one record; prior records are byte-identical (append-only proven).
2. An `mqo-agent` `answer.json` fixture round-trips: appends without translation, `query` returns it intact.
3. `query --session <id> --outcome clarify` returns only matching records; `--since <ts>` filters by timestamp.
4. `summary` emits aggregate counts (clarify rate, block rate, pillar-fire frequency, total budget) matching hand-computed values.
5. `verify` passes on an untampered log and fails (non-zero, naming the offending record) on a tampered fixture.
6. `verify` reports the `provfs` session xattr when present; absent = reported without failing.
7. `query`/`summary` output is deterministic (timestamps come from records, not wall clock).
8. `serve` answers MCP tool calls; `--help` documents every flag; all tests run cluster-free against bundled fixtures.

## Installation

```bash
cargo install --path .
```

## License

Licensed under either of MIT or Apache-2.0 at your option.
