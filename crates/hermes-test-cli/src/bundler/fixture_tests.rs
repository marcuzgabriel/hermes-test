//! Golden fixture tests for the esbuild-output patches (hardening plan item 2).
//!
//! Each directory under tests/fixtures/<transform>/ is one fixture:
//!   input.js    — esbuild-shaped source fed to the transform
//!   expected.js — golden file: the exact output the transform must produce
//!   behavior.js — assertions executed in REAL Hermes against the transformed
//!                 code (text can look right and still behave wrong — Day 23)
//!
//! Workflow:
//!   - fix a bug:        add a fixture dir reproducing it, watch it fail,
//!                       patch the Rust until green
//!   - change a patch:   HT_UPDATE_FIXTURES=1 cargo test, then review the
//!                       expected.js diffs like any code change
//!   - bump esbuild:     regenerate input.js from the new output shape; the
//!                       expected.js diffs show exactly what changed

use std::fs;
use std::path::PathBuf;

use super::patches::{hoist_mock_modules, inject_mock_require_shim, patch_esbuild_for_hermes};
use crate::hermes::Runtime;

fn fixtures_root(transform: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(transform)
}

fn run_fixture_dir(transform: &str, apply: impl Fn(&str) -> String) {
    let root = fixtures_root(transform);
    let mut dirs: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("fixture dir missing: {} ({e})", root.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "no fixtures found in {}", root.display());

    let update = std::env::var("HT_UPDATE_FIXTURES").is_ok();
    let mut failures: Vec<String> = Vec::new();

    for dir in &dirs {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let input = fs::read_to_string(dir.join("input.js"))
            .unwrap_or_else(|e| panic!("{name}: input.js unreadable: {e}"));
        let actual = apply(&input);

        let expected_path = dir.join("expected.js");
        if update {
            fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("{name}: cannot write expected.js: {e}"));
            eprintln!("[fixtures] {name}: expected.js updated — review the diff");
        } else {
            match fs::read_to_string(&expected_path) {
                Err(_) => {
                    failures.push(format!(
                        "{name}: expected.js missing — run `HT_UPDATE_FIXTURES=1 cargo test` once, review the generated file, commit it"
                    ));
                    continue;
                }
                Ok(expected) if actual != expected => {
                    failures.push(format!(
                        "{name}: transform output no longer matches expected.js — if the change is intentional, rerun with HT_UPDATE_FIXTURES=1 and review the diff"
                    ));
                    continue;
                }
                Ok(_) => {}
            }
        }

        // Text matching is not enough: run the transformed code in real Hermes
        // and let behavior.js assert the semantics (throws on failure).
        let behavior = fs::read_to_string(dir.join("behavior.js"))
            .unwrap_or_else(|e| panic!("{name}: behavior.js unreadable: {e}"));
        let program = format!("{actual}\n{behavior}");
        let rt = Runtime::new().expect("failed to create Hermes runtime");
        if let Err(e) = rt.eval(&program, &format!("fixture-{name}.js")) {
            failures.push(format!("{name}: behavior check failed in Hermes: {e}"));
        }
    }

    assert!(
        failures.is_empty(),
        "\n{} fixture(s) failed in {transform}:\n  {}\n",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn class_extends_fixtures() {
    run_fixture_dir("class-extends", patch_esbuild_for_hermes);
}

// Patches 1–3 of patch_esbuild_for_hermes rewrite esbuild's runtime helpers
// (__copyProps, __export, __toESM). The inputs reproduce the helper text
// byte-for-byte from a real bundle (examples/expo-app cache) — the patches
// are exact-text matches, so whitespace drift IS the failure being guarded.
#[test]
fn esbuild_helper_fixtures() {
    run_fixture_dir("esbuild-helpers", patch_esbuild_for_hermes);
}

// The shim that replaces esbuild's "Dynamic require of X is not supported"
// throw with the __HT_mocks registry Proxy — how externalized/native modules
// become mockable (silent-green risk class: if it stops matching, tests run
// against nothing).
#[test]
fn mock_require_shim_fixtures() {
    run_fixture_dir("mock-require-shim", inject_mock_require_shim);
}

// Reordering inside bundled test-file bodies: init_*() module initializers
// are pushed below the last ht.mock() so mocks register before modules
// capture values at init time.
#[test]
fn hoist_mocks_fixtures() {
    run_fixture_dir("hoist-mocks", hoist_mock_modules);
}
