/// Acceptance tests for mqo-decision-log, paired to each numbered AC in the PRD.
///
/// AC1: append adds exactly one record; prior records byte-identical (append-only)
/// AC2: answer.json round-trip — appends without translation, query returns intact
/// AC3: query --session / --outcome / --since filters work correctly
/// AC4: summary computes correct aggregate counts matching hand-computed values
/// AC5: verify passes clean log, fails (non-zero) on tampered log (names offending record)
/// AC6: verify reports provfs xattr when present; absent = reported, not fatal
/// AC7: query/summary output stable across runs (deterministic)
/// AC8: serve answers tool calls; --help documents every flag; tests run cluster-free

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn copy_fixture(dir: &TempDir, name: &str) -> PathBuf {
    let src = fixtures_dir().join(name);
    let dst = dir.path().join(name);
    fs::copy(&src, &dst).expect("copy fixture");
    dst
}

// ─── AC1 ──────────────────────────────────────────────────────────────────────
#[test]
fn ac1_append_adds_exactly_one_record_and_prior_bytes_unchanged() {
    use mqo_decision_log::log_store::LogStore;

    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("test.jsonl");
    let log_str = log_path.to_str().unwrap();

    // Start with 2 records
    let rec1 = serde_json::json!({
        "ts": "2026-06-16T08:00:00Z", "session": "s1",
        "question": "Q1", "access_verdict": "allow",
        "budget_consumed": 100.0, "outcome": "answered"
    });
    let rec2 = serde_json::json!({
        "ts": "2026-06-16T08:01:00Z", "session": "s1",
        "question": "Q2", "access_verdict": "allow",
        "budget_consumed": 200.0, "outcome": "clarify"
    });

    {
        let mut store = LogStore::open(log_str).unwrap();
        store.append(&rec1).unwrap();
        store.append(&rec2).unwrap();
    }

    // Snapshot bytes after 2 records
    let before = fs::read_to_string(log_str).unwrap();
    let before_lines: Vec<&str> = before.lines().collect();
    assert_eq!(before_lines.len(), 2, "expected 2 lines before append");

    // Append third record
    let rec3 = serde_json::json!({
        "ts": "2026-06-16T08:02:00Z", "session": "s1",
        "question": "Q3", "access_verdict": "deny",
        "budget_consumed": 50.0, "outcome": "blocked"
    });
    {
        let mut store = LogStore::open(log_str).unwrap();
        store.append(&rec3).unwrap();
    }

    let after = fs::read_to_string(log_str).unwrap();
    let after_lines: Vec<&str> = after.lines().collect();
    assert_eq!(after_lines.len(), 3, "expected exactly 3 lines after append");

    // Prior lines must be byte-identical
    for (i, (b, a)) in before_lines.iter().zip(after_lines.iter()).enumerate() {
        assert_eq!(b, a, "line {} was mutated — append-only violated", i + 1);
    }

    // New line parses to the appended record
    let parsed: serde_json::Value = serde_json::from_str(after_lines[2]).unwrap();
    assert_eq!(parsed["question"].as_str().unwrap(), "Q3");
}

// ─── AC2 ──────────────────────────────────────────────────────────────────────
#[test]
fn ac2_answer_json_round_trips_without_translation() {
    use mqo_decision_log::log_store::LogStore;
    use mqo_decision_log::query::{run_query, QueryFilter};
    use mqo_decision_log::record::validate_record;

    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("test.jsonl");
    let log_str = log_path.to_str().unwrap();

    // Read answer.json fixture
    let answer_str = fs::read_to_string(fixtures_dir().join("answer.json")).unwrap();
    let answer: serde_json::Value = serde_json::from_str(&answer_str).unwrap();

    // Must validate without translation
    validate_record(&answer).expect("answer.json must pass schema validation");

    // Append it
    {
        let mut store = LogStore::open(log_str).unwrap();
        store.append(&answer).unwrap();
    }

    // Query it back
    let store = LogStore::open(log_str).unwrap();
    let filter = QueryFilter {
        session: Some("sess-abc123".to_string()),
        since: None,
        outcome: None,
    };
    let records = run_query(&store, &filter).unwrap();
    assert_eq!(records.len(), 1, "query should return the appended record");

    let returned = &records[0];
    assert_eq!(returned["question"], answer["question"]);
    assert_eq!(returned["outcome"], answer["outcome"]);
    assert_eq!(returned["credential_id"], answer["credential_id"]);
    assert_eq!(returned["budget_consumed"], answer["budget_consumed"]);
}

// ─── AC3 ──────────────────────────────────────────────────────────────────────
#[test]
fn ac3_query_filters_by_session_outcome_since() {
    use mqo_decision_log::log_store::LogStore;
    use mqo_decision_log::query::{run_query, QueryFilter};

    let dir = TempDir::new().unwrap();
    let log_path = copy_fixture(&dir, "sample.jsonl");
    let log_str = log_path.to_str().unwrap();

    let store = LogStore::open(log_str).unwrap();

    // Filter by session
    let f = QueryFilter {
        session: Some("sess-aaa".to_string()),
        since: None,
        outcome: None,
    };
    let recs = run_query(&store, &f).unwrap();
    assert_eq!(recs.len(), 2, "sess-aaa has 2 records");
    for r in &recs {
        assert_eq!(r["session"].as_str().unwrap(), "sess-aaa");
    }

    // Filter by outcome = clarify
    let f = QueryFilter {
        session: None,
        since: None,
        outcome: Some("clarify".to_string()),
    };
    let recs = run_query(&store, &f).unwrap();
    assert_eq!(recs.len(), 1, "only 1 clarify record in sample");
    assert_eq!(recs[0]["outcome"].as_str().unwrap(), "clarify");

    // Filter by session + outcome
    let f = QueryFilter {
        session: Some("sess-bbb".to_string()),
        since: None,
        outcome: Some("blocked".to_string()),
    };
    let recs = run_query(&store, &f).unwrap();
    assert_eq!(recs.len(), 1, "sess-bbb has 1 blocked record");

    // Filter by --since (only records at/after 09:10)
    let f = QueryFilter {
        session: None,
        since: Some("2026-06-16T09:10:00Z".to_string()),
        outcome: None,
    };
    let recs = run_query(&store, &f).unwrap();
    assert_eq!(recs.len(), 3, "3 records at or after 09:10");
    for r in &recs {
        assert!(r["ts"].as_str().unwrap() >= "2026-06-16T09:10:00Z");
    }
}

// ─── AC4 ──────────────────────────────────────────────────────────────────────
#[test]
fn ac4_summary_matches_hand_computed_values() {
    use mqo_decision_log::log_store::LogStore;
    use mqo_decision_log::summary::compute_summary;

    let dir = TempDir::new().unwrap();
    let log_path = copy_fixture(&dir, "sample.jsonl");
    let log_str = log_path.to_str().unwrap();

    let store = LogStore::open(log_str).unwrap();
    let s = compute_summary(&store, None).unwrap();

    // Hand-computed from sample.jsonl:
    // 5 total: 3 answered, 1 clarify, 1 blocked
    // clarify_rate = 1/5 = 0.2, block_rate = 1/5 = 0.2
    // total_budget = 800 + 200 + 50 + 600 + 1100 = 2750
    assert_eq!(s.total_questions, 5);
    assert_eq!(s.answered_count, 3);
    assert_eq!(s.clarify_count, 1);
    assert_eq!(s.blocked_count, 1);
    assert!((s.clarify_rate - 0.2).abs() < 1e-9, "clarify_rate mismatch");
    assert!((s.block_rate - 0.2).abs() < 1e-9, "block_rate mismatch");
    assert!((s.total_budget_consumed - 2750.0).abs() < 1e-6, "budget mismatch");

    // Pillar frequencies: sales=1, product=1, store=1, pii=1, hr=1, headcount=1, revenue=2, time=2
    assert_eq!(s.pillar_fire_frequency.get("revenue").copied().unwrap_or(0), 2);
    assert_eq!(s.pillar_fire_frequency.get("time").copied().unwrap_or(0), 2);
    assert_eq!(s.pillar_fire_frequency.get("sales").copied().unwrap_or(0), 1);
    assert_eq!(s.pillar_fire_frequency.get("pii").copied().unwrap_or(0), 1);

    // Test --since filter: only records at/after 09:10 (3 records)
    let s2 = compute_summary(&store, Some("2026-06-16T09:10:00Z")).unwrap();
    assert_eq!(s2.total_questions, 3);
    assert_eq!(s2.answered_count, 2);
    assert_eq!(s2.blocked_count, 1);
    assert!((s2.total_budget_consumed - (50.0 + 600.0 + 1100.0)).abs() < 1e-6);
}

// ─── AC5 ──────────────────────────────────────────────────────────────────────
#[test]
fn ac5_verify_passes_clean_log_fails_tampered() {
    use mqo_decision_log::verify::verify_log;

    let dir = TempDir::new().unwrap();

    // Clean log
    let clean_path = copy_fixture(&dir, "sample.jsonl");
    let result = verify_log(clean_path.to_str().unwrap()).unwrap();
    assert!(result.ok, "verify should pass on clean log");
    assert!(result.violations.is_empty(), "no violations on clean log");

    // Tampered log
    let tampered_path = copy_fixture(&dir, "tampered.jsonl");
    let result = verify_log(tampered_path.to_str().unwrap()).unwrap();
    assert!(!result.ok, "verify should fail on tampered log");
    assert!(!result.violations.is_empty(), "should report violations");
    // The violation message should name the offending record (line 2)
    let viol = &result.violations[0];
    assert!(viol.contains("line 2") || viol.contains("tampered"),
        "violation must name offending record: got {:?}", viol);
}

// ─── AC6 ──────────────────────────────────────────────────────────────────────
#[test]
fn ac6_verify_reports_provfs_xattr_absent_without_failing() {
    use mqo_decision_log::verify::verify_log;

    let dir = TempDir::new().unwrap();
    let log_path = copy_fixture(&dir, "sample.jsonl");

    // On a normal box without provfs, xattr is absent — this must not cause failure
    let result = verify_log(log_path.to_str().unwrap()).unwrap();
    // provfs xattr absent → provfs_active = false, but ok can still be true
    assert!(!result.provfs_active, "provfs should not be active on test box");
    assert!(result.provfs_session_xattr.is_none(), "no xattr present");
    // The clean log should still pass even without provfs
    assert!(result.ok, "verify should pass on clean log regardless of provfs");
}

// ─── AC7 ──────────────────────────────────────────────────────────────────────
#[test]
fn ac7_query_and_summary_are_deterministic() {
    use mqo_decision_log::log_store::LogStore;
    use mqo_decision_log::query::{run_query, QueryFilter};
    use mqo_decision_log::summary::compute_summary;

    let dir = TempDir::new().unwrap();
    let log_path = copy_fixture(&dir, "sample.jsonl");
    let log_str = log_path.to_str().unwrap();

    let filter = QueryFilter {
        session: None,
        since: Some("2026-06-16T09:00:00Z".to_string()),
        outcome: None,
    };

    // Run query twice
    let r1 = {
        let store = LogStore::open(log_str).unwrap();
        run_query(&store, &filter).unwrap()
    };
    let r2 = {
        let store = LogStore::open(log_str).unwrap();
        run_query(&store, &filter).unwrap()
    };
    assert_eq!(
        serde_json::to_string(&r1).unwrap(),
        serde_json::to_string(&r2).unwrap(),
        "query output must be identical across two runs"
    );

    // Run summary twice
    let s1 = {
        let store = LogStore::open(log_str).unwrap();
        compute_summary(&store, None).unwrap()
    };
    let s2 = {
        let store = LogStore::open(log_str).unwrap();
        compute_summary(&store, None).unwrap()
    };
    assert_eq!(s1.total_questions, s2.total_questions);
    assert_eq!(s1.total_budget_consumed, s2.total_budget_consumed);
    assert_eq!(s1.clarify_count, s2.clarify_count);
}

// ─── AC8 ──────────────────────────────────────────────────────────────────────
#[test]
fn ac8_serve_handles_tool_calls() {
    use mqo_decision_log::serve::handle_request_for_test;

    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("serve-test.jsonl");
    let log_str = log_path.to_str().unwrap();

    // append via serve
    let append_req = serde_json::json!({
        "id": 1,
        "method": "append",
        "params": {
            "session": "srv-session",
            "record": {
                "ts": "2026-06-16T10:30:00Z",
                "session": "srv-session",
                "question": "Test via serve",
                "access_verdict": "allow",
                "budget_consumed": 42.0,
                "outcome": "answered"
            }
        }
    });
    let resp = handle_request_for_test(log_str, &serde_json::to_string(&append_req).unwrap());
    assert_eq!(resp["result"]["appended"], serde_json::json!(true));

    // query via serve
    let query_req = serde_json::json!({
        "id": 2,
        "method": "query",
        "params": {"session": "srv-session"}
    });
    let resp = handle_request_for_test(log_str, &serde_json::to_string(&query_req).unwrap());
    let records = resp["result"]["records"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["question"].as_str().unwrap(), "Test via serve");

    // summary via serve
    let summary_req = serde_json::json!({
        "id": 3,
        "method": "summary",
        "params": {}
    });
    let resp = handle_request_for_test(log_str, &serde_json::to_string(&summary_req).unwrap());
    assert_eq!(resp["result"]["total_questions"], serde_json::json!(1));

    // unknown method
    let bad_req = serde_json::json!({"id": 4, "method": "nonexistent", "params": {}});
    let resp = handle_request_for_test(log_str, &serde_json::to_string(&bad_req).unwrap());
    assert!(resp.get("error").is_some());
}
