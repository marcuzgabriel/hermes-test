mod hermes;
mod bundler;
mod coverage;

#[cfg(test)]
mod cli_tests;

use clap::{Parser, Subcommand};
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Suppress Hermes's internal "[hermes-compile]" debug output by redirecting
/// stderr to /dev/null during bytecode compilation. Console.log from tests
/// is collected via JSI host functions and stored in a buffer, not via stderr.
fn suppress_hermes_stderr<F, R>(f: F) -> R
where F: FnOnce() -> R {
    use std::os::unix::io::AsRawFd;
    let dev_null = match std::fs::File::open("/dev/null") {
        Ok(f) => f,
        Err(_) => return f(),
    };
    let saved_fd = unsafe { libc::dup(2) };
    unsafe { libc::dup2(dev_null.as_raw_fd(), 2); }
    let result = f();
    if saved_fd >= 0 {
        unsafe { libc::dup2(saved_fd, 2); libc::close(saved_fd); }
    }
    result
}

// Embed the harness bundle at compile time
const HARNESS_JS: &str = include_str!("../../../packages/hermes-test/dist/harness.bundle.js");

#[derive(Parser)]
#[command(name = "hermes-test", version, about = "Fast test runner for React Native")]
struct Cli {
    /// Test files or patterns (e.g. useActionMessages.test.ts)
    /// If a name doesn't contain a path separator, searches the project for it.
    files: Vec<String>,

    /// Watch for file changes and rerun tests
    #[arg(long, short)]
    watch: bool,

    /// Project root directory (auto-detected if omitted)
    #[arg(long)]
    root: Option<PathBuf>,

    /// Skip bundling, run raw JS files directly
    #[arg(long)]
    no_bundle: bool,

    /// JavaScript file to evaluate directly
    #[arg(long)]
    eval: Option<String>,

    /// Deprecated: split mode is incompatible with ht.shallow() component rendering
    #[arg(long, hide = true)]
    split: bool,

    /// Collect coverage and write lcov report to coverage/lcov.info
    #[arg(long)]
    coverage: bool,

    /// Update snapshot files instead of comparing
    #[arg(long, alias = "update-snapshots")]
    update_snapshots: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run tests (legacy subcommand, prefer flat: hermes-test [files] [--watch])
    Run {
        files: Vec<PathBuf>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        no_bundle: bool,
    },

    /// Watch for file changes (legacy subcommand, prefer: hermes-test [files] --watch)
    Watch {
        files: Vec<PathBuf>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    // Legacy subcommands still work
    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Run {
                files,
                root,
                no_bundle,
            } => {
                run_tests(&files, &root, no_bundle, false, false);
            }
            Commands::Watch {
                files,
                root,
            } => {
                watch_tests(&files, &root);
            }
        }
        return;
    }

    if let Some(ref path) = cli.eval {
        eval_file(path);
        return;
    }

    // Auto-detect project root: walk up from cwd to find package.json
    let root = cli.root.unwrap_or_else(|| find_project_root());

    // Resolve file arguments: if a name has no path separator, search the project
    let files = resolve_test_files(&cli.files, &root);

    if cli.split {
        eprintln!("\x1b[31mError:\x1b[0m --split is deprecated and incompatible with ht.shallow() component rendering.");
        std::process::exit(1);
    }

    if cli.watch {
        watch_tests(&files, &root);
    } else {
        run_tests(&files, &root, cli.no_bundle, cli.coverage, cli.update_snapshots);
    }
}

/// Walk up from cwd to find the nearest directory with package.json
fn find_project_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = cwd.as_path();
    loop {
        if dir.join("package.json").exists() {
            return dir.to_path_buf();
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => {
                // No package.json found, use cwd
                return cwd;
            }
        }
    }
}

/// Resolve file arguments: bare names like "useActionMessages.test.ts"
/// get searched for in the project. Paths like "src/foo.test.ts" are used as-is.
fn resolve_test_files(args: &[String], root: &PathBuf) -> Vec<PathBuf> {
    if args.is_empty() {
        return vec![];
    }

    let mut resolved = Vec::new();
    let all_tests = bundler::find_test_files(root);

    for arg in args {
        let p = PathBuf::from(arg);
        // If it contains a separator or exists as a path, use it directly
        if arg.contains('/') || arg.contains('\\') || p.exists() {
            resolved.push(p);
            continue;
        }

        // Search: match by filename (exact or substring)
        let matches: Vec<&PathBuf> = all_tests
            .iter()
            .filter(|f| {
                f.file_name()
                    .map(|n| {
                        let name = n.to_string_lossy();
                        name == arg.as_str() || name.contains(arg.as_str())
                    })
                    .unwrap_or(false)
            })
            .collect();

        if matches.is_empty() {
            eprintln!("No test file matching '{arg}' found in {}", root.display());
            std::process::exit(1);
        }

        for m in matches {
            if !resolved.contains(m) {
                resolved.push(m.clone());
            }
        }
    }

    resolved
}

fn eval_file(path: &str) {
    let rt = hermes::Runtime::new().unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1);
    });

    let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Failed to read {path}: {e}");
        std::process::exit(1);
    });

    match rt.eval(&source, path) {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("Evaluation failed: {e}");
            std::process::exit(1);
        }
    }
}

/// Expand positional args into test files. Directories expand to the test files
/// they contain (respecting the configured testMatch pattern). A path that does
/// not exist, or a directory containing no test files, is a HARD ERROR — the
/// runner must never exit 0 having run nothing the user explicitly pointed at.
fn expand_file_args(files: &[PathBuf], root: &std::path::Path) -> Vec<PathBuf> {
    let cfg = bundler::read_config(root);
    match try_expand_file_args(files, cfg.test_match.as_deref()) {
        Ok(out) => out,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(1);
        }
    }
}

/// Core of expand_file_args, separated so the never-run-zero-tests invariant
/// is unit-testable (the wrapper exits the process on Err).
fn try_expand_file_args(files: &[PathBuf], test_match: Option<&str>) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for f in files {
        if f.is_dir() {
            let found = bundler::find_test_files_with_pattern(f, test_match);
            if found.is_empty() {
                let suffix = test_match.unwrap_or(".test.ts");
                return Err(format!(
                    "\x1b[31mNo test files found in directory:\x1b[0m {}\n  Looking for files matching \x1b[1m*{suffix}\x1b[0m",
                    f.display()
                ));
            }
            out.extend(found);
        } else if f.is_file() {
            out.push(f.clone());
        } else {
            return Err(format!("\x1b[31mNo such file or directory:\x1b[0m {}", f.display()));
        }
    }
    Ok(out)
}

fn run_tests(files: &[PathBuf], root: &PathBuf, no_bundle: bool, coverage: bool, update_snapshots: bool) {
    let root = std::fs::canonicalize(root).unwrap_or_else(|e| {
        eprintln!("Invalid root directory: {e}");
        std::process::exit(1);
    });

    // Find test files
    let test_files = if files.is_empty() {
        bundler::find_test_files(&root)
    } else {
        expand_file_args(files, &root)
    };

    if test_files.is_empty() {
        let cfg = bundler::read_config(&root);
        let suffix = cfg.test_match.as_deref().unwrap_or(".test.ts");
        eprintln!("\x1b[31mNo test files found\x1b[0m");
        eprintln!();
        eprintln!("  Looking for files matching \x1b[1m*{suffix}\x1b[0m in {}", root.display());
        eprintln!();
        eprintln!("  Make sure your test files end with \x1b[1m{suffix}\x1b[0m");
        eprintln!("  Or set a custom pattern in hermes-test.config.json:");
        eprintln!("    {{ \"testMatch\": \".hermes.test.ts\" }}");
        std::process::exit(1);
    }

    eprintln!();
    eprintln!(" \x1b[1mhermes-test\x1b[0m \x1b[2mv{}\x1b[0m", env!("CARGO_PKG_VERSION"));
    eprintln!();

    let rt = hermes::Runtime::new().unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1);
    });

    // Inject the harness via source eval. The minified harness has no ES6 class
    // syntax, so bytecode compilation is unnecessary overhead (~90ms per invocation).
    suppress_hermes_stderr(|| {
        rt.eval(HARNESS_JS, "hermes-test/harness.js").unwrap_or_else(|e| {
            eprintln!("Failed to load harness: {e}");
            std::process::exit(1);
        });
    });

    if no_bundle {
        // Direct mode: read and concatenate test files, eval in Hermes
        let mut combined = String::new();
        for file in &test_files {
            let content = std::fs::read_to_string(file).unwrap_or_else(|e| {
                eprintln!("Failed to read {}: {e}", file.display());
                std::process::exit(1);
            });
            combined.push_str(&content);
            combined.push('\n');
        }

        // Append the runner
        combined.push_str(
            r#"
var __results = globalThis.__HT.runTests();
JSON.stringify({
  tests: __results,
  passed: __results.filter(function(t) { return t.status === 'pass'; }).length,
  failed: __results.filter(function(t) { return t.status === 'fail'; }).length,
  skipped: __results.filter(function(t) { return t.status === 'skip'; }).length,
  total: __results.length
});
"#,
        );

        match rt.eval(&combined, "tests") {
            Ok(json) => {
                if !print_results(&json) {
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("Test execution failed: {e}");
                std::process::exit(1);
            }
        }
    } else {
        let cfg = bundler::read_config(&root);
        let start = Instant::now();

        let alias_names: Vec<String> = cfg.aliases.iter().map(|(a, _)| a.clone()).collect();
        let alias_pairs: Vec<(String, String)> = cfg.aliases.clone();

        // Mock delivery: the plugin resolver (esbuild JS API onResolve hook) — one
        // bundle, one VM, all mock kinds. (The legacy pipeline — shadow trees,
        // package shims, isolated bundles, HT_RESOLVER flag — was deleted after
        // the plugin default soaked in prod; phase 4 of the resolver plan.)
        let batch_files: Vec<PathBuf> = test_files.clone();

        let all_mocks = bundler::find_mock_modules_with_alias_pairs(&batch_files, &alias_names, &alias_pairs);

        // Scan ht.shallow() for auto-mock generation
        let mut shallow_auto_mocks: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
        for file in &batch_files {
            for entry in bundler::scan_shallow_auto_mocks_with_pairs(file, &alias_names, &alias_pairs) {
                if let Some((_, existing_jsx, existing_other)) = shallow_auto_mocks.iter_mut().find(|(p, _, _)| p == &entry.0) {
                    for name in entry.1 { if !existing_jsx.contains(&name) { existing_jsx.push(name); } }
                    for name in entry.2 { if !existing_other.contains(&name) { existing_other.push(name); } }
                } else {
                    shallow_auto_mocks.push(entry);
                }
            }
        }

        // Debug: print shallow auto-mocks (HT_DEBUG=1)
        if !shallow_auto_mocks.is_empty() && std::env::var("HT_DEBUG").is_ok() {
            let mut dbg = format!("[DEBUG] Shallow auto-mocks ({}):\n", shallow_auto_mocks.len());
            for (path, jsx_names, other_names) in &shallow_auto_mocks {
                dbg.push_str(&format!("  {} -> jsx: {:?}, other: {:?}\n", path, jsx_names, other_names));
            }
            eprint!("{}", dbg);
        }

        run_tests_single(&rt, &batch_files, &root, &all_mocks, &cfg, start, &[], coverage, update_snapshots, &shallow_auto_mocks);
    }
}

/// Single-bundle path: generates one entry, bundles everything together via the
/// onResolve plugin resolver.
fn run_tests_single(
    rt: &hermes::Runtime,
    test_files: &[PathBuf],
    root: &PathBuf,
    mock_modules: &[String],
    cfg: &bundler::BundleConfig,
    start: Instant,
    transforms: &[(PathBuf, PathBuf)],
    coverage: bool,
    update_snapshots: bool,
    shallow_auto_mocks: &[(String, Vec<String>, Vec<String>)],
) {
    let cache_prefix = "plugin";
    let cache_key = bundler::compute_single_bundle_cache_key(test_files, root, mock_modules, cfg);
    let cache_dir = root.join(".hermes-test-cache");
    let cache_path = cache_dir.join(format!("{cache_prefix}-{cache_key}.js"));
    let bytecode_path = cache_dir.join(format!("{cache_prefix}-{cache_key}.hbc"));

    // Try bytecode cache first (fastest), then JS cache, then fresh bundle.
    // When coverage is enabled, we need source maps so always do a fresh bundle.
    let mut sm_info: Option<coverage::SourceMapInfo> = None;

    let bundle = if !coverage && bytecode_path.exists() {
        // Bytecode cache hit — skip everything, load directly
        None
    } else if !coverage && cache_path.exists() {
        // JS cache hit — patched bundle, skip shadow setup + esbuild
        Some(std::fs::read_to_string(&cache_path).unwrap_or_default())
    } else {
        // Cache miss (or coverage mode) — full pipeline. The onResolve plugin
        // delivers ALL mocks (relative, alias, package).
        let (shim_cfg, wrapper_shim_dir) = bundler::create_wrapper_shims(root, cfg);
        let entry_content = bundler::generate_entry_with_shallow(test_files, None, mock_modules, &shim_cfg, transforms, Some(root), shallow_auto_mocks);
        let entry_path = root.join(".hermes-test-entry.js");
        std::fs::write(&entry_path, &entry_content).unwrap_or_else(|e| {
            eprintln!("Failed to write entry file: {e}");
            std::process::exit(1);
        });

        let b = if coverage {
            // Coverage: use sourcemap-aware bundling
            let esbuild_path = match bundler::find_esbuild_pub(root) {
                Ok(p) => p,
                Err(_) => {
                    let _ = std::fs::remove_file(&entry_path);
                    if let Some(ref d) = wrapper_shim_dir { let _ = std::fs::remove_dir_all(d); }
                    eprintln!("esbuild not found. Install it: bun add -d esbuild");
                    std::process::exit(1);
                }
            };
            let _ = &esbuild_path;
            let sm_result = {
                let pm = bundler::create_plugin_mock_wrappers(test_files, root, &shim_cfg, mock_modules);
                let r = bundler::bundle_via_plugin_with_sourcemap(
                    &entry_path, root, &pm.external_mocks, &shim_cfg,
                    &pm.file_wrappers, &pm.text_wrappers,
                );
                let _ = std::fs::remove_dir_all(&pm.dir);
                r
            };
            match sm_result {
                Ok(result) => {
                    let patched_lines = result.code.lines().count() as u32;
                    let line_delta = patched_lines.saturating_sub(result.pre_patch_line_count);
                    if let Some(source_map) = result.source_map {
                        sm_info = Some(coverage::SourceMapInfo { source_map, line_delta });
                    }
                    result.code
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&entry_path);
                    if let Some(ref d) = wrapper_shim_dir { let _ = std::fs::remove_dir_all(d); }
                    for (_, temp) in transforms { let _ = std::fs::remove_file(temp); }
                    eprintln!("Bundling failed: {e}");
                    std::process::exit(1);
                }
            }
        } else {
            // Classify every mock and generate onResolve wrappers; only mocks
            // with no bundleable real module (natives, shims, unresolvable
            // specifiers) remain text-externalized.
            let bundle_result = {
                let pm = bundler::create_plugin_mock_wrappers(test_files, root, &shim_cfg, mock_modules);
                let r = bundler::bundle_via_plugin_with_config(
                    &entry_path, root, &pm.external_mocks, &shim_cfg,
                    &pm.file_wrappers, &pm.text_wrappers,
                );
                let _ = std::fs::remove_dir_all(&pm.dir);
                r
            };
            match bundle_result {
                Ok(b) => b,
                Err(e) => {
                    let _ = std::fs::remove_file(&entry_path);
                    if let Some(ref d) = wrapper_shim_dir { let _ = std::fs::remove_dir_all(d); }
                    for (_, temp) in transforms { let _ = std::fs::remove_file(temp); }
                    eprintln!("Bundling failed: {e}");
                    std::process::exit(1);
                }
            }
        };

        // Cleanup temp dirs
        let _ = std::fs::remove_file(&entry_path);
        if let Some(ref d) = wrapper_shim_dir { let _ = std::fs::remove_dir_all(d); }
        for (_, temp) in transforms { let _ = std::fs::remove_file(temp); }

        // Cache the PATCHED bundle
        if !coverage {
            let _ = std::fs::create_dir_all(&cache_dir);
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                for entry in entries.flatten() {
                    let n = entry.file_name(); let n = n.to_string_lossy();
                    if n.starts_with(&format!("{cache_prefix}-")) && !n.contains(&cache_key) { let _ = std::fs::remove_file(entry.path()); }
                }
            }
            let _ = std::fs::write(&cache_path, &b);
        }
        Some(b)
    };

    // Coverage: instrument the bundle post-esbuild
    let coverage_map_path = cache_dir.join("coverage-map.json");
    let bundle = if coverage {
        let js = bundle.or_else(|| std::fs::read_to_string(&cache_path).ok())
            .unwrap_or_else(|| { eprintln!("No bundle for coverage"); std::process::exit(1); });
        let result = if let Some(ref info) = sm_info {
            coverage::instrument_bundle_with_sourcemap(&js, "bundle.js", info)
        } else {
            coverage::instrument_bundle(&js, "bundle.js")
        };
        match result {
            Some((instrumented, coverage_map)) => {
                eprintln!(" \x1b[2mCoverage:\x1b[0m instrumented ({} → {} bytes)", js.len(), instrumented.len());
                let _ = std::fs::create_dir_all(&cache_dir);
                let _ = std::fs::write(&coverage_map_path, &coverage_map);
                Some(instrumented)
            }
            None => {
                eprintln!(" \x1b[33mCoverage: instrumentation failed, running without\x1b[0m");
                Some(js)
            }
        }
    } else {
        bundle
    };

    // Set coverage flag so harness suppresses PASS lines
    if coverage {
        let _ = rt.eval("globalThis.__HT_coverage = true", "coverage-flag");
    }

    // Set update-snapshots flag
    if update_snapshots {
        let _ = rt.eval("globalThis.__HT_updateSnapshots = true", "snapshot-flag");
    }

    // Eval: prefer bytecode → compile+cache bytecode → fallback to JS text
    let eval_result = if !coverage && bytecode_path.exists() {
        // Bytecode cache hit
        match std::fs::read(&bytecode_path) {
            Ok(bytes) => rt.eval_bytes(&bytes, "bundle.hbc"),
            Err(_) => {
                let js = std::fs::read_to_string(&cache_path).unwrap_or_default();
                rt.eval(&js, "bundle.js")
            }
        }
    } else if let Some(ref js) = bundle {
        if coverage {
            // Coverage: compile to bytecode first (Hermes handles instrumented code
            // differently in raw JS eval vs bytecode — bytecode is needed for correctness)
            match crate::hermes::compile_bytecode(js, "bundle.js") {
                Ok(bytecode) => rt.eval_bytes(&bytecode, "bundle.hbc"),
                Err(_) => rt.eval(js, "bundle.js"),
            }
        } else {
            // Try to compile to bytecode and cache it
            match crate::hermes::compile_bytecode(js, "bundle.js") {
                Ok(bytecode) => {
                    let _ = std::fs::write(&bytecode_path, &bytecode);
                    rt.eval_bytes(&bytecode, "bundle.hbc")
                }
                Err(_) => rt.eval(js, "bundle.js"),
            }
        }
    } else {
        eprintln!("No bundle available");
        std::process::exit(1);
    };
    if let Err(e) = eval_result {
        eprintln!("Test execution failed: {e}");
        std::process::exit(1);
    }

    print_console_logs(rt);

    // Collect coverage before printing summary
    let mut coverage_failed = false;
    if coverage {
        let map_path = if coverage_map_path.exists() { Some(coverage_map_path.as_path()) } else { None };
        if let Some(lcov) = coverage::collect_coverage(rt, map_path) {
            let cov_dir = root.join("coverage");
            let _ = std::fs::create_dir_all(&cov_dir);
            let lcov_path = cov_dir.join("lcov.info");
            match std::fs::write(&lcov_path, &lcov) {
                Ok(_) => eprintln!(" \x1b[32mCoverage:\x1b[0m {}", lcov_path.display()),
                Err(e) => eprintln!(" \x1b[33mCoverage write failed: {e}\x1b[0m"),
            }
            // Terminal summary
            let total_pct = coverage::print_summary(&lcov);
            // HTML report
            let html_path = cov_dir.join("index.html");
            match coverage::generate_html_report(&lcov, &html_path, &root) {
                Ok(_) => eprintln!(" \x1b[32mHTML report:\x1b[0m {}", html_path.display()),
                Err(e) => eprintln!(" \x1b[33mHTML report failed: {e}\x1b[0m"),
            }
            // Check coverage threshold
            if let Some(threshold) = cfg.coverage_threshold {
                if total_pct < threshold {
                    eprintln!();
                    eprintln!(" \x1b[31mCoverage {total_pct:.1}% is below threshold {threshold:.0}%\x1b[0m");
                    coverage_failed = true;
                }
            }
        } else {
            eprintln!(" \x1b[33mNo coverage data collected\x1b[0m");
        }
    }

    let elapsed = start.elapsed();
    match rt.eval("globalThis.__HT_results", "results") {
        Ok(json) => {
            let (main_passed, main_failed, main_total, main_snapshots) = parse_result_counts(&json);
            print_jest_summary(
                test_files.len(),
                count_failed_suites(&json),
                main_passed,
                main_failed,
                main_total,
                main_snapshots,
                elapsed.as_secs_f64(),
            );
            if main_failed > 0 {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to read test results: {e}");
            std::process::exit(1);
        }
    }

    if coverage_failed {
        std::process::exit(1);
    }
}

/// Parse (passed, failed, total, snapshots) out of a __HT_results JSON string.
fn parse_result_counts(json: &str) -> (u64, u64, u64, u64) {
    let inner: String = serde_json::from_str(json).unwrap_or_else(|_| json.to_string());
    let v: serde_json::Value = serde_json::from_str(&inner).unwrap_or_default();
    (
        v["passed"].as_u64().unwrap_or(0),
        v["failed"].as_u64().unwrap_or(0),
        v["total"].as_u64().unwrap_or(0),
        v["snapshots"].as_u64().unwrap_or(0),
    )
}

fn watch_tests(files: &[PathBuf], root: &PathBuf) {
    let root = std::fs::canonicalize(root).unwrap_or_else(|e| {
        eprintln!("Invalid root directory: {e}");
        std::process::exit(1);
    });

    let all_test_files = if files.is_empty() {
        bundler::find_test_files(&root)
    } else {
        expand_file_args(files, &root)
    };

    if all_test_files.is_empty() {
        eprintln!("No test files found");
        std::process::exit(1);
    }

    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(Duration::from_millis(50), tx)
        .unwrap_or_else(|e| {
            eprintln!("Failed to create file watcher: {e}");
            std::process::exit(1);
        });

    use notify_debouncer_mini::notify::RecursiveMode;
    debouncer
        .watcher()
        .watch(&root, RecursiveMode::Recursive)
        .unwrap_or_else(|e| {
            eprintln!("Failed to watch directory: {e}");
            std::process::exit(1);
        });

    eprintln!("\x1b[2m[watch]\x1b[0m Watching for changes in {}", root.display());
    eprintln!("\x1b[2m[watch]\x1b[0m Press Ctrl+C to stop\n");
    let watch_all = std::env::var("HT_WATCH_ALL").is_ok();

    let cfg = bundler::read_config(&root);

    // Initial run: use the same fresh-runtime + single-bundle path as watch reruns.
    // This keeps watch startup behavior aligned with normal test runs.
    let initial_rt = hermes::Runtime::new().unwrap_or_else(|e| {
        eprintln!("Hermes error: {e}");
        std::process::exit(1);
    });
    suppress_hermes_stderr(|| {
        let _ = initial_rt.eval(HARNESS_JS, "hermes-test/harness.js");
    });
    let alias_names: Vec<String> = cfg.aliases.iter().map(|(a, _)| a.clone()).collect();
    let alias_pairs: Vec<(String, String)> = cfg.aliases.clone();
    let initial_batch: Vec<PathBuf> = all_test_files.clone();
    let mock_modules = bundler::find_mock_modules_with_alias_pairs(&initial_batch, &alias_names, &alias_pairs);
    let initial_start = Instant::now();

    let mut initial_shallow: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    for f in &initial_batch {
        for s in bundler::scan_shallow_auto_mocks_with_pairs(f, &alias_names, &alias_pairs) {
            if let Some((_, ej, eo)) = initial_shallow.iter_mut().find(|(p, _, _)| p == &s.0) {
                for name in s.1 { if !ej.contains(&name) { ej.push(name); } }
                for name in s.2 { if !eo.contains(&name) { eo.push(name); } }
            } else { initial_shallow.push(s); }
        }
    }

    let (shim_cfg, wrapper_shim_dir) = bundler::create_wrapper_shims(&root, &cfg);
    let entry = bundler::generate_entry_with_shallow(&initial_batch, None, &mock_modules, &shim_cfg, &[], Some(&root), &initial_shallow);
    let entry_path = root.join(".hermes-test-watch-initial-entry.js");
    let _ = std::fs::write(&entry_path, &entry);

    let bundle_result = if initial_batch.is_empty() {
        Ok(String::new())
    } else {
        let pm = bundler::create_plugin_mock_wrappers(&initial_batch, &root, &shim_cfg, &mock_modules);
        let r = bundler::bundle_via_plugin_with_config(
            &entry_path, &root, &pm.external_mocks, &shim_cfg,
            &pm.file_wrappers, &pm.text_wrappers,
        );
        let _ = std::fs::remove_dir_all(&pm.dir);
        r
    };
    let _ = std::fs::remove_file(&entry_path);
    if let Some(ref d) = wrapper_shim_dir { let _ = std::fs::remove_dir_all(d); }

    match bundle_result {
        Ok(bundle) => {
            let eval_result = if initial_batch.is_empty() {
                Ok(String::new())
            } else {
                match crate::hermes::compile_bytecode(&bundle, "bundle.js") {
                    Ok(bc) => initial_rt.eval_bytes(&bc, "bundle.hbc"),
                    Err(_) => initial_rt.eval(&bundle, "bundle.js"),
                }
            };
            if let Err(e) = eval_result {
                eprintln!("\x1b[31mInitial watch execution failed: {e}\x1b[0m");
            } else {
                print_console_logs(&initial_rt);
                let elapsed = initial_start.elapsed();
                let (mp, mf, mt, ms, mfs) = if initial_batch.is_empty() {
                    (0, 0, 0, 0, 0)
                } else if let Ok(json) = initial_rt.eval("globalThis.__HT_results", "results") {
                    let (p, f, t, sn) = parse_result_counts(&json);
                    (p, f, t, sn, count_failed_suites(&json))
                } else {
                    (0, 0, 0, 0, 0)
                };
                print_jest_summary(all_test_files.len(), mfs, mp, mf, mt, ms, elapsed.as_secs_f64());
            }
        }
        Err(e) => eprintln!("\x1b[31mInitial watch bundle failed: {e}\x1b[0m"),
    }

    // Build dep graph for affected-file watch reruns.
    let depgraph_entry = bundler::generate_entry(&all_test_files, None, &mock_modules, &cfg, &[], Some(&root));
    let depgraph_entry_path = root.join(".hermes-test-entry.js");
    let depgraph = if std::fs::write(&depgraph_entry_path, &depgraph_entry).is_ok() {
        let dg = match bundler::bundle_with_depgraph(&depgraph_entry_path, &root, &all_test_files, &mock_modules) {
            Ok((_, d)) => d,
            Err(_) => std::collections::HashMap::new(),
        };
        let _ = std::fs::remove_file(&depgraph_entry_path);
        dg
    } else {
        std::collections::HashMap::new()
    };

    // Watch loop
    let current_depgraph = depgraph;

    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
                // Collect changed source files
                let changed_paths: Vec<PathBuf> = events
                    .iter()
                    .filter(|e| {
                        let p = e.path.to_string_lossy();
                        !p.contains("node_modules")
                            && !p.contains(".hermes-test-")
                            && !p.contains("/target/")
                            && (p.ends_with(".ts")
                                || p.ends_with(".tsx")
                                || p.ends_with(".js")
                                || p.ends_with(".jsx"))
                            && e.kind == DebouncedEventKind::Any
                    })
                    .map(|e| e.path.clone())
                    .collect();

                if changed_paths.is_empty() {
                    continue;
                }

                // Show changed files
                let changed_names: Vec<String> = changed_paths
                    .iter()
                    .filter_map(|p| {
                        p.strip_prefix(&root)
                            .ok()
                            .map(|rel| rel.to_string_lossy().to_string())
                    })
                    .collect();
                eprintln!(
                    "\n\x1b[2m[watch]\x1b[0m Changed: {}",
                    changed_names.join(", ")
                );

                // Find affected test files from the dep graph
                let mut affected: Vec<PathBuf> = Vec::new();
                for changed in &changed_paths {
                    let canonical = std::fs::canonicalize(changed)
                        .unwrap_or_else(|_| changed.clone());

                    if let Some(tests) = current_depgraph.get(&canonical) {
                        for t in tests {
                            if !affected.contains(t) {
                                affected.push(t.clone());
                            }
                        }
                    }

                    // If the changed file IS a test file, include it
                    let name = changed.to_string_lossy();
                    let suffix = cfg.test_match.as_deref().unwrap_or(".test.ts");
                    let is_test = name.ends_with(suffix)
                        || (suffix.ends_with(".ts") && name.ends_with(&format!("{}x", suffix)))
                        || name.ends_with(".test.ts")
                        || name.ends_with(".test.tsx")
                        || name.ends_with(".test.js");
                    if is_test && !affected.contains(&canonical) {
                        affected.push(canonical);
                    }
                }

                // Determine which tests to run
                let rerun_files = if watch_all {
                    let all = if files.is_empty() {
                        bundler::find_test_files(&root)
                    } else {
                        files.to_vec()
                    };
                    eprintln!(
                        "\x1b[2m[watch]\x1b[0m HT_WATCH_ALL=1 set, running full suite"
                    );
                    all
                } else if affected.is_empty() {
                    let all = if files.is_empty() {
                        bundler::find_test_files(&root)
                    } else {
                        files.to_vec()
                    };
                    eprintln!(
                        "\x1b[2m[watch]\x1b[0m Could not determine affected tests, running all"
                    );
                    all
                } else {
                    let affected_names: Vec<String> = affected
                        .iter()
                        .filter_map(|p| {
                            p.strip_prefix(&root)
                                .ok()
                                .map(|r| r.to_string_lossy().to_string())
                        })
                        .collect();
                    eprintln!(
                        "\x1b[2m[watch]\x1b[0m Running {} affected test{}: {}",
                        affected.len(),
                        if affected.len() == 1 { "" } else { "s" },
                        affected_names.join(", ")
                    );
                    affected
                };

                // Fresh runtime + full single-bundle for correct results
                let watch_rt = match hermes::Runtime::new() {
                    Ok(r) => r,
                    Err(e) => { eprintln!("Hermes error: {e}"); continue; }
                };
                suppress_hermes_stderr(|| {
                    let _ = watch_rt.eval(HARNESS_JS, "hermes-test/harness.js");
                });
                let alias_names: Vec<String> = cfg.aliases.iter().map(|(a, _)| a.clone()).collect();
                let alias_pairs: Vec<(String, String)> = cfg.aliases.clone();
                let rerun_batch: Vec<PathBuf> = rerun_files.clone();
                let mock_modules = bundler::find_mock_modules_with_alias_pairs(&rerun_batch, &alias_names, &alias_pairs);
                let rerun_start = Instant::now();

                // Scan shallow auto-mocks
                let mut watch_shallow: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
                for f in &rerun_batch {
                    for s in bundler::scan_shallow_auto_mocks_with_pairs(f, &alias_names, &alias_pairs) {
                        if let Some((_, ej, eo)) = watch_shallow.iter_mut().find(|(p, _, _)| p == &s.0) {
                            for name in s.1 { if !ej.contains(&name) { ej.push(name); } }
                            for name in s.2 { if !eo.contains(&name) { eo.push(name); } }
                        } else { watch_shallow.push(s); }
                    }
                }

                // Build single bundle
                let (shim_cfg, wrapper_shim_dir) = bundler::create_wrapper_shims(&root, &cfg);
                let entry = bundler::generate_entry_with_shallow(&rerun_batch, None, &mock_modules, &shim_cfg, &[], Some(&root), &watch_shallow);
                let entry_path = root.join(".hermes-test-watch-entry.js");
                let _ = std::fs::write(&entry_path, &entry);

                let bundle_result = if rerun_batch.is_empty() {
                    Ok(String::new())
                } else {
                    let pm = bundler::create_plugin_mock_wrappers(&rerun_batch, &root, &shim_cfg, &mock_modules);
                    let r = bundler::bundle_via_plugin_with_config(
                        &entry_path, &root, &pm.external_mocks, &shim_cfg,
                        &pm.file_wrappers, &pm.text_wrappers,
                    );
                    let _ = std::fs::remove_dir_all(&pm.dir);
                    r
                };
                let _ = std::fs::remove_file(&entry_path);
                if let Some(ref d) = wrapper_shim_dir { let _ = std::fs::remove_dir_all(d); }

                match bundle_result {
                    Ok(bundle) => {
                        let eval_result = if rerun_batch.is_empty() {
                            Ok(String::new())
                        } else {
                            match crate::hermes::compile_bytecode(&bundle, "bundle.js") {
                                Ok(bc) => watch_rt.eval_bytes(&bc, "bundle.hbc"),
                                Err(_) => watch_rt.eval(&bundle, "bundle.js"),
                            }
                        };
                        if let Err(e) = eval_result {
                            eprintln!("\x1b[31mTest execution failed: {e}\x1b[0m");
                        } else {
                            print_console_logs(&watch_rt);
                            let elapsed = rerun_start.elapsed();
                            let (mp, mf, mt, ms, mfs) = if rerun_batch.is_empty() {
                                (0, 0, 0, 0, 0)
                            } else if let Ok(json) = watch_rt.eval("globalThis.__HT_results", "results") {
                                let (p, f, t, sn) = parse_result_counts(&json);
                                (p, f, t, sn, count_failed_suites(&json))
                            } else {
                                (0, 0, 0, 0, 0)
                            };
                            print_jest_summary(rerun_files.len(), mfs, mp, mf, mt, ms, elapsed.as_secs_f64());
                        }
                    }
                    Err(e) => eprintln!("\x1b[31mBundle failed: {e}\x1b[0m"),
                }
            }
            Ok(Err(e)) => {
                eprintln!("Watch error: {e:?}");
            }
            Err(e) => {
                eprintln!("Channel error: {e}");
                break;
            }
        }
    }
}


/// Print console.log/warn/error output that was collected during test execution.
fn print_console_logs(rt: &hermes::Runtime) {
    let logs_js = r#"(function() {
        var logs = globalThis.__HT_logs;
        if (!logs || !logs.length) return '[]';
        var out = [];
        for (var i = 0; i < logs.length; i++) {
            out.push({ level: logs[i].level, message: logs[i].message });
        }
        return JSON.stringify(out);
    })()"#;

    if let Ok(json) = rt.eval(logs_js, "console-logs") {
        let inner: String = serde_json::from_str(&json).unwrap_or(json.clone());
        if let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(&inner) {
            for entry in &entries {
                let level = entry["level"].as_str().unwrap_or("log");
                let msg = entry["message"].as_str().unwrap_or("");
                // Skip known noise from our custom react-reconciler host config
                if msg.contains("Expected host context to exist") { continue; }
                match level {
                    "warn" => eprintln!("\x1b[33m⚠ {msg}\x1b[0m"),
                    "error" => eprintln!("\x1b[31m✗ {msg}\x1b[0m"),
                    _ => eprintln!("  {msg}"),
                }
            }
        }
    }
}

/// Count suites (distinct source files) containing at least one failing test
/// in a __HT_results JSON string.
fn count_failed_suites(json: &str) -> usize {
    let inner: String = serde_json::from_str(json).unwrap_or_else(|_| json.to_string());
    let v: serde_json::Value = match serde_json::from_str(&inner) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let mut failed: Vec<&str> = Vec::new();
    if let Some(tests) = v["tests"].as_array() {
        for t in tests {
            if t["status"].as_str() == Some("fail") {
                let file = t["file"].as_str().unwrap_or("unknown");
                if !failed.contains(&file) {
                    failed.push(file);
                }
            }
        }
    }
    failed.len()
}

fn format_jest_summary(file_count: usize, failed_suites: usize, passed: u64, failed: u64, total: u64, snapshots: u64, secs: f64) -> String {
    let mut out = String::new();
    if failed_suites > 0 {
        let passed_suites = file_count.saturating_sub(failed_suites);
        out.push_str(&format!(" \x1b[1mTest Suites:\x1b[0m  \x1b[32m{passed_suites} passed\x1b[0m, \x1b[31m{failed_suites} failed\x1b[0m, {file_count} total\n"));
    } else {
        out.push_str(&format!(" \x1b[1mTest Suites:\x1b[0m  \x1b[32m{file_count} passed\x1b[0m, {file_count} total\n"));
    }
    if failed > 0 {
        out.push_str(&format!(" \x1b[1mTests:\x1b[0m        \x1b[32m{passed} passed\x1b[0m, \x1b[31m{failed} failed\x1b[0m, {total} total\n"));
    } else {
        out.push_str(&format!(" \x1b[1mTests:\x1b[0m        \x1b[32m{passed} passed\x1b[0m, {total} total\n"));
    }
    if snapshots > 0 {
        out.push_str(&format!(" \x1b[1mSnapshots:\x1b[0m    \x1b[32m{snapshots} passed\x1b[0m, {snapshots} total\n"));
    }
    out.push_str(&format!(" \x1b[1mTime:\x1b[0m         {secs:.2}s"));
    out
}

fn print_jest_summary(file_count: usize, failed_suites: usize, passed: u64, failed: u64, total: u64, snapshots: u64, secs: f64) {
    eprintln!();
    eprintln!("{}", format_jest_summary(file_count, failed_suites, passed, failed, total, snapshots, secs));
}

fn print_results(json: &str) -> bool {
    let inner: String = match serde_json::from_str(json) {
        Ok(s) => s,
        Err(_) => { println!("{json}"); return false; }
    };
    let results: serde_json::Value = match serde_json::from_str(&inner) {
        Ok(v) => v,
        Err(_) => { println!("{inner}"); return false; }
    };

    let passed = results["passed"].as_u64().unwrap_or(0);
    let failed = results["failed"].as_u64().unwrap_or(0);
    let skipped = results["skipped"].as_u64().unwrap_or(0);
    let total = results["total"].as_u64().unwrap_or(0);

    if let Some(tests) = results["tests"].as_array() {
        // Group tests by source file
        let mut files: Vec<(String, Vec<&serde_json::Value>)> = Vec::new();
        for test in tests {
            let file = test["file"].as_str().unwrap_or("unknown");
            if let Some(f) = files.iter_mut().find(|(n, _)| n == file) {
                f.1.push(test);
            } else {
                files.push((file.to_string(), vec![test]));
            }
        }

        // Print per-file results
        eprintln!();
        for (file_name, file_tests) in &files {
            let file_failed = file_tests.iter().filter(|t| t["status"].as_str() == Some("fail")).count();
            let file_total = file_tests.len();
            let file_duration: u64 = file_tests.iter().map(|t| t["duration"].as_u64().unwrap_or(0)).sum();
            let time_str = if file_duration > 0 { format!(" \x1b[2m({file_duration}ms)\x1b[0m") } else { String::new() };

            if file_failed == 0 {
                eprintln!(" \x1b[32mPASS\x1b[0m  {file_name} \x1b[2m({file_total} tests)\x1b[0m{time_str}");
            } else {
                let file_passed = file_total - file_failed;
                eprintln!(" \x1b[31mFAIL\x1b[0m  {file_name} \x1b[2m({file_passed} passed, {file_failed} failed)\x1b[0m{time_str}");
                // Show only failing tests
                for test in file_tests {
                    if test["status"].as_str() != Some("fail") { continue; }
                    let name = test["name"].as_str().unwrap_or("?");
                    eprintln!("       \x1b[31m✗ {name}\x1b[0m");
                    if let Some(error) = test["error"].as_str() {
                        if !error.is_empty() {
                            eprintln!("         \x1b[2m{error}\x1b[0m");
                            // Hint for common native module errors
                            if error.contains("Property 'window' doesn't exist")
                                || error.contains("Property 'document' doesn't exist")
                                || error.contains("requireNativeComponent")
                                || error.contains("TurboModuleRegistry")
                                || error.contains("NativeModules")
                                || error.contains("UIManager")
                            {
                                eprintln!("         \x1b[33mHint: this looks like a native module error. Add the package to \"externals\" in hermes-test.config.json\x1b[0m");
                            }
                        }
                    }
                }
            }
        }
    }

    // Summary
    eprintln!();
    if failed == 0 {
        eprintln!(" \x1b[32mTests:\x1b[0m  {passed} passed, {total} total");
    } else {
        eprintln!(" \x1b[31mTests:\x1b[0m  \x1b[32m{passed} passed\x1b[0m, \x1b[31m{failed} failed\x1b[0m, {total} total");
    }
    if skipped > 0 {
        eprintln!(" \x1b[33mSkip:\x1b[0m   {skipped}");
    }

    failed == 0
}
