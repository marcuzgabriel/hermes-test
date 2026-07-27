//! Unit tests for the pure helpers in main.rs.
//!
//! The most important block guards the never-run-zero-tests invariant
//! (hardening plan item 1): a directory arg must expand to its test files,
//! and a bad path or empty directory must be a hard error — never a green
//! run of nothing.

use std::fs;
use std::path::PathBuf;

use crate::{count_failed_suites, format_jest_summary, parse_result_counts, try_expand_file_args};

/// Fresh unique temp dir per test (no external tempfile dependency).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ht-cli-tests-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

// ---- try_expand_file_args: the silent-zero-run guard ----

#[test]
fn expand_directory_arg_finds_test_files() {
    let dir = temp_dir("expand-dir");
    fs::write(dir.join("a.test.ts"), "").unwrap();
    fs::write(dir.join("not-a-test.ts"), "").unwrap();
    fs::create_dir_all(dir.join("nested")).unwrap();
    fs::write(dir.join("nested/b.test.ts"), "").unwrap();

    let out = try_expand_file_args(&[dir.clone()], None).unwrap();
    let names: Vec<String> = out
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(out.len(), 2, "exactly the two test files, not the plain .ts file");
    assert!(names.contains(&"a.test.ts".to_string()));
    assert!(names.contains(&"b.test.ts".to_string()), "nested directories are searched");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn expand_respects_test_match_pattern() {
    let dir = temp_dir("expand-pattern");
    fs::write(dir.join("a.test.ts"), "").unwrap();
    fs::write(dir.join("b.hermes.test.ts"), "").unwrap();

    let out = try_expand_file_args(&[dir.clone()], Some(".hermes.test.ts")).unwrap();
    assert_eq!(out.len(), 1);
    assert!(out[0].to_string_lossy().ends_with("b.hermes.test.ts"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn expand_file_arg_passes_through() {
    let dir = temp_dir("expand-file");
    let file = dir.join("x.test.ts");
    fs::write(&file, "").unwrap();

    let out = try_expand_file_args(&[file.clone()], None).unwrap();
    assert_eq!(out, vec![file]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn expand_nonexistent_path_is_hard_error() {
    let bogus = PathBuf::from("/definitely/not/a/real/path.test.ts");
    let err = try_expand_file_args(&[bogus], None).unwrap_err();
    assert!(err.contains("No such file or directory"), "got: {err}");
}

#[test]
fn expand_directory_without_tests_is_hard_error() {
    // THE silent-green CI bug (reproduced on published v1.1.5): a directory
    // containing no test files must never expand to an empty run that exits 0.
    let dir = temp_dir("expand-empty");
    fs::write(dir.join("helper.ts"), "").unwrap();

    let err = try_expand_file_args(&[dir.clone()], None).unwrap_err();
    assert!(err.contains("No test files found"), "got: {err}");
    let _ = fs::remove_dir_all(&dir);
}

// ---- parse_result_counts / count_failed_suites ----

const RESULTS: &str = r#"{"passed":3,"failed":2,"total":5,"snapshots":1,"tests":[
    {"file":"a.test.ts","name":"t1","status":"pass"},
    {"file":"a.test.ts","name":"t2","status":"fail"},
    {"file":"a.test.ts","name":"t3","status":"fail"},
    {"file":"b.test.ts","name":"t4","status":"pass"},
    {"file":"c.test.ts","name":"t5","status":"pass"}
]}"#;

#[test]
fn parse_result_counts_reads_double_encoded_json() {
    // __HT_results is a JS string containing JSON, so the Hermes eval
    // round-trip delivers JSON-encoded JSON.
    let double = serde_json::to_string(RESULTS).unwrap();
    assert_eq!(parse_result_counts(&double), (3, 2, 5, 1));
}

#[test]
fn parse_result_counts_reads_plain_json() {
    assert_eq!(parse_result_counts(RESULTS), (3, 2, 5, 1));
}

#[test]
fn parse_result_counts_garbage_is_zeroes() {
    assert_eq!(parse_result_counts("not json at all"), (0, 0, 0, 0));
}

#[test]
fn count_failed_suites_counts_files_not_tests() {
    // Two failing tests, but both in a.test.ts — one failed suite.
    assert_eq!(count_failed_suites(RESULTS), 1);
    let double = serde_json::to_string(RESULTS).unwrap();
    assert_eq!(count_failed_suites(&double), 1);
    assert_eq!(count_failed_suites("garbage"), 0);
}

// ---- format_jest_summary ----

fn strip_ansi(s: &str) -> String {
    let re = regex::Regex::new("\x1b\\[[0-9;]*m").unwrap();
    re.replace_all(s, "").to_string()
}

#[test]
fn summary_all_green() {
    let out = strip_ansi(&format_jest_summary(24, 0, 1211, 0, 1211, 3, 2.02));
    assert!(out.contains("Test Suites:  24 passed, 24 total"), "got: {out}");
    assert!(out.contains("Tests:        1211 passed, 1211 total"), "got: {out}");
    assert!(out.contains("Snapshots:    3 passed, 3 total"), "got: {out}");
    assert!(out.contains("Time:         2.02s"), "got: {out}");
}

#[test]
fn summary_with_failures_reports_failed_suites() {
    // The pre-fix bug: file count was printed as BOTH passed and total even
    // with failures present ("24 passed, 24 total" above a failed test).
    let out = strip_ansi(&format_jest_summary(24, 1, 1210, 1, 1211, 3, 2.02));
    assert!(out.contains("Test Suites:  23 passed, 1 failed, 24 total"), "got: {out}");
    assert!(out.contains("Tests:        1210 passed, 1 failed, 1211 total"), "got: {out}");
}

#[test]
fn summary_failed_suites_never_underflows() {
    // Defensive: more failed suites than files must not panic in release
    // aggregation edge cases.
    let out = strip_ansi(&format_jest_summary(1, 2, 0, 5, 5, 0, 0.1));
    assert!(out.contains("0 passed, 2 failed, 1 total"), "got: {out}");
}
