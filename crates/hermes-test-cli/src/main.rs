mod hermes;
mod bundler;
mod coverage;

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

    /// Use vendor/group bundle splitting (auto-enabled for large suites)
    #[arg(long)]
    split: bool,

    /// Collect coverage and write lcov report to coverage/lcov.info
    #[arg(long)]
    coverage: bool,

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

    if cli.watch {
        watch_tests(&files, &root);
    } else {
        run_tests(&files, &root, cli.no_bundle, cli.split, cli.coverage);
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

fn run_tests(files: &[PathBuf], root: &PathBuf, no_bundle: bool, force_split: bool, coverage: bool) {
    let root = std::fs::canonicalize(root).unwrap_or_else(|e| {
        eprintln!("Invalid root directory: {e}");
        std::process::exit(1);
    });

    // Find test files
    let test_files = if files.is_empty() {
        bundler::find_test_files(&root)
    } else {
        files.to_vec()
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

        let all_mocks = bundler::find_mock_modules(&test_files);

        // All files in one batch — shadow wrappers handle aliased mock isolation.
        // HT_PER_FILE forces per-file isolation as fallback.
        let force_per_file = std::env::var("HT_PER_FILE").is_ok();
        if force_per_file {
            run_tests_per_file(&rt, &test_files, &root, &cfg, start);
        } else {
            let use_split = force_split || cfg.split;
            if use_split {
                run_tests_split(&rt, &test_files, &root, &all_mocks, &cfg, start);
            } else {
                run_tests_single(&rt, &test_files, &root, &all_mocks, &cfg, start, &[], coverage);
            }
        }
    }
}

/// Single-bundle path: generates one entry, bundles everything together.
fn run_tests_single(
    rt: &hermes::Runtime,
    test_files: &[PathBuf],
    root: &PathBuf,
    mock_modules: &[String],
    cfg: &bundler::BundleConfig,
    start: Instant,
    transforms: &[(PathBuf, PathBuf)],
    coverage: bool,
) {
    // Check single-bundle cache FIRST — skip shadow wrapper/shim setup if cached.
    let cache_key = bundler::compute_single_bundle_cache_key(test_files, root, mock_modules, cfg);
    let cache_dir = root.join(".hermes-test-cache");
    let cache_path = cache_dir.join(format!("single-{cache_key}.js"));
    let bytecode_path = cache_dir.join(format!("single-{cache_key}.hbc"));

    // Try bytecode cache first (fastest), then JS cache, then fresh bundle.
    let bundle = if bytecode_path.exists() {
        // Bytecode cache hit — skip everything, load directly
        None
    } else if let Ok(cached) = std::fs::read_to_string(&cache_path) {
        // JS cache hit — patched bundle, skip shadow setup + esbuild
        Some(cached)
    } else {
        // Cache miss — full pipeline: shadow wrappers → esbuild → patch → cache
        let (wrapper_cfg, wrapper_shim_dir) = bundler::create_wrapper_shims(root, cfg);
        let (shadow_cfg, shadow_dirs) = bundler::create_shadow_wrappers(root, mock_modules, &wrapper_cfg);
        let non_aliased_mocks: Vec<String> = mock_modules.iter().filter(|m| {
            !cfg.aliases.iter().any(|(alias, _)| *m == alias || m.starts_with(&format!("{alias}/")))
        }).cloned().collect();
        let (shim_cfg, shim_dir, remaining_externals) =
            bundler::create_package_shims(root, &non_aliased_mocks, &shadow_cfg);
        let entry_content = bundler::generate_entry(test_files, None, mock_modules, &shim_cfg, transforms, Some(root));
        let entry_path = root.join(".hermes-test-entry.js");
        std::fs::write(&entry_path, &entry_content).unwrap_or_else(|e| {
            eprintln!("Failed to write entry file: {e}");
            std::process::exit(1);
        });

        let b = match bundler::bundle_auto_with_config(&entry_path, root, &remaining_externals, &shim_cfg) {
            Ok(b) => b,
            Err(e) => {
                let _ = std::fs::remove_file(&entry_path);
                for dir in &shadow_dirs { let _ = std::fs::remove_dir_all(dir); }
                if let Some(ref d) = shim_dir { let _ = std::fs::remove_dir_all(d); }
                if let Some(ref d) = wrapper_shim_dir { let _ = std::fs::remove_dir_all(d); }
                for (_, temp) in transforms { let _ = std::fs::remove_file(temp); }
                eprintln!("Bundling failed: {e}");
                std::process::exit(1);
            }
        };

        // Cleanup temp dirs
        let _ = std::fs::remove_file(&entry_path);
        for dir in &shadow_dirs { let _ = std::fs::remove_dir_all(dir); }
        if let Some(ref d) = shim_dir { let _ = std::fs::remove_dir_all(d); }
        if let Some(ref d) = wrapper_shim_dir { let _ = std::fs::remove_dir_all(d); }
        for (_, temp) in transforms { let _ = std::fs::remove_file(temp); }

        // Cache the PATCHED bundle (patches are already applied by bundle_auto_with_config)
        let _ = std::fs::create_dir_all(&cache_dir);
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                let n = entry.file_name(); let n = n.to_string_lossy();
                if n.starts_with("single-") && !n.contains(&cache_key) { let _ = std::fs::remove_file(entry.path()); }
            }
        }
        let _ = std::fs::write(&cache_path, &b);
        Some(b)
    };

    // Coverage: instrument the bundle post-esbuild
    let bundle = if coverage {
        let js = bundle.or_else(|| std::fs::read_to_string(&cache_path).ok())
            .unwrap_or_else(|| { eprintln!("No bundle for coverage"); std::process::exit(1); });
        match coverage::instrument_bundle(&js, "bundle.js") {
            Some(instrumented) => {
                eprintln!(" \x1b[2mCoverage:\x1b[0m instrumented ({} → {} bytes)", js.len(), instrumented.len());
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
            // Coverage: eval JS directly (instrumented code, no bytecode cache)
            rt.eval(js, "bundle.js")
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
    if coverage {
        if let Some(lcov) = coverage::collect_coverage(rt) {
            let cov_dir = root.join("coverage");
            let _ = std::fs::create_dir_all(&cov_dir);
            let lcov_path = cov_dir.join("lcov.info");
            match std::fs::write(&lcov_path, &lcov) {
                Ok(_) => eprintln!(" \x1b[32mCoverage:\x1b[0m {}", lcov_path.display()),
                Err(e) => eprintln!(" \x1b[33mCoverage write failed: {e}\x1b[0m"),
            }
        } else {
            eprintln!(" \x1b[33mNo coverage data collected\x1b[0m");
        }
    }

    let elapsed = start.elapsed();
    match rt.eval("globalThis.__HT_results", "results") {
        Ok(json) => {
            if !print_summary_with_time(&json, elapsed.as_millis(), test_files.len()) {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to read test results: {e}");
            std::process::exit(1);
        }
    }
}

/// Per-file bundling: each test file gets its own esbuild invocation with
/// only its own mocks externalized. Prevents mock interference across files.
/// The Hermes runtime is reused across files for speed.
fn run_tests_per_file(
    _rt: &hermes::Runtime,
    test_files: &[PathBuf],
    root: &PathBuf,
    cfg: &bundler::BundleConfig,
    start: Instant,
) {
    // Phase 1: Parallel esbuild — spawn all bundlers concurrently.
    // Each file gets a unique entry path to avoid write conflicts.
    let mut handles: Vec<std::thread::JoinHandle<(PathBuf, Result<String, String>)>> = Vec::new();

    for (i, file) in test_files.iter().enumerate() {
        let file_slice = &[file.clone()];
        let file_mocks = bundler::find_mock_modules(file_slice);
        let entry_content = bundler::generate_entry(file_slice, None, &file_mocks, cfg, &[], Some(root));
        let entry_path = root.join(format!(".hermes-test-entry-{i}.js"));
        if let Err(e) = std::fs::write(&entry_path, &entry_content) {
            eprintln!("Failed to write entry: {e}");
            continue;
        }

        let file_clone = file.clone();
        let entry_clone = entry_path.clone();
        let root_clone = root.clone();
        let mocks_clone = file_mocks.clone();

        handles.push(std::thread::spawn(move || {
            let result = bundler::bundle_auto(&entry_clone, &root_clone, &mocks_clone);
            let _ = std::fs::remove_file(&entry_clone);
            (file_clone, result)
        }));
    }

    // Collect all bundle results
    let bundles: Vec<(PathBuf, Result<String, String>)> = handles
        .into_iter()
        .map(|h| h.join().expect("esbuild thread panicked"))
        .collect();

    // Phase 2: Sequential Hermes eval — run each bundle in its own runtime.
    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    let mut _total_skipped = 0usize;
    let mut total_count = 0usize;
    let mut any_failed = false;

    for (file, bundle_result) in &bundles {
        let name = file.file_name().map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let bundle = match bundle_result {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Bundling failed for {name}: {e}");
                any_failed = true;
                continue;
            }
        };

        let rt = match hermes::Runtime::new() {
            Ok(r) => r,
            Err(e) => { eprintln!("Runtime error: {e}"); any_failed = true; continue; }
        };
        suppress_hermes_stderr(|| {
            if let Err(e) = rt.eval(HARNESS_JS, "hermes-test/harness.js") {
                eprintln!("Failed to load harness: {e}");
            }
        });

        let dummy_path = file.with_extension("js");
        if let Err(e) = suppress_hermes_stderr(|| {
            if let Some(bytecode) = bundler::compile_to_bytecode(bundle, &dummy_path) {
                rt.eval_bytes(&bytecode, "bundle.hbc")
            } else {
                rt.eval(bundle, "bundle.js")
            }
        }) {
            eprintln!("Test execution failed for {name}: {e}");
            any_failed = true;
            continue;
        }

        print_console_logs(&rt);

        match rt.eval("globalThis.__HT_results", "results") {
            Ok(raw) => {
                let inner: String = serde_json::from_str(&raw).unwrap_or(raw.clone());
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&inner) {
                    let p = v["passed"].as_u64().unwrap_or(0) as usize;
                    let f = v["failed"].as_u64().unwrap_or(0) as usize;
                    let s = v["skipped"].as_u64().unwrap_or(0) as usize;
                    let t = v["total"].as_u64().unwrap_or(0) as usize;
                    total_passed += p;
                    total_failed += f;
                    _total_skipped += s;
                    total_count += t;

                    if f > 0 {
                        any_failed = true;
                        print_results(&raw);
                    } else {
                        println!("\x1b[32mPASS\x1b[0m  {name} \x1b[2m({t} tests)\x1b[0m");
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to read results: {e}");
                any_failed = true;
            }
        }
    }

    // Print summary
    let elapsed = start.elapsed();
    println!();
    if total_failed > 0 {
        print!(" \x1b[31mTests:\x1b[0m  \x1b[32m{total_passed} passed\x1b[0m, \x1b[31m{total_failed} failed\x1b[0m");
    } else {
        print!(" \x1b[32mTests:\x1b[0m  {total_passed} passed");
    }
    println!(", {total_count} total");
    println!(" \x1b[2mFiles:\x1b[0m  {}", test_files.len());
    println!(" \x1b[2mTime:\x1b[0m   {:.2}s", elapsed.as_secs_f64());

    if any_failed {
        std::process::exit(1);
    }
}

/// Split-bundle path: vendor (node_modules) + group bundles (local code).
/// Avoids Hermes super-linear scaling with large bundles.
fn run_tests_split(
    rt: &hermes::Runtime,
    test_files: &[PathBuf],
    root: &PathBuf,
    mock_modules: &[String],
    cfg: &bundler::BundleConfig,
    start: Instant,
) {
    let bundle_start = Instant::now();
    let split = match bundler::bundle_split(test_files, root, mock_modules, cfg) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Bundle split failed: {e}");
            std::process::exit(1);
        }
    };
    let bundle_ms = bundle_start.elapsed().as_millis();

    // Eval vendor (setup + all node_modules) — use bytecode cache
    let exec_start = Instant::now();
    let (eval_vendor, vendor_cached) = if let Some((bytecode, cached)) = bundler::compile_to_bytecode_cached(&split.vendor, root, "vendor") {
        (rt.eval_bytes(&bytecode, "vendor.hbc"), cached)
    } else {
        (rt.eval(&split.vendor, "vendor.js"), false)
    };
    if let Err(e) = eval_vendor {
        eprintln!("Vendor eval failed: {e}");
        std::process::exit(1);
    }

    // Eval each group bundle
    for (i, group) in split.groups.iter().enumerate() {
        let name = format!("group-{i}.js");
        let eval_result = if let Some(bytecode) = bundler::compile_to_bytecode(group, &root.join(&name)) {
            rt.eval_bytes(&bytecode, &format!("group-{i}.hbc"))
        } else {
            rt.eval(group, &name)
        };
        if let Err(e) = eval_result {
            eprintln!("Group {i} eval failed: {e}");
            // Continue to other groups — don't exit on a single group failure
        }
    }

    // Run all registered tests
    let runner = r#"
var __results = globalThis.__HT.runTests();
globalThis.__HT_results = JSON.stringify({
  tests: __results,
  passed: __results.filter(function(t) { return t.status === 'pass'; }).length,
  failed: __results.filter(function(t) { return t.status === 'fail'; }).length,
  skipped: __results.filter(function(t) { return t.status === 'skip'; }).length,
  total: __results.length
});
"#;
    if let Err(e) = rt.eval(runner, "runner.js") {
        eprintln!("Test runner failed: {e}");
        std::process::exit(1);
    }

    print_console_logs(rt);

    let exec_ms = exec_start.elapsed().as_millis();
    let elapsed = start.elapsed();
    let cache_str = if vendor_cached { " (cached)" } else { "" };
    eprintln!(" \x1b[2mSplit:\x1b[0m  bundle {bundle_ms}ms, exec {exec_ms}ms (vendor {}KB{cache_str} + {} groups)",
        split.vendor.len() / 1024, split.groups.len());
    match rt.eval("globalThis.__HT_results", "results") {
        Ok(json) => {
            if !print_results_with_time(&json, elapsed.as_millis(), test_files.len()) {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to read test results: {e}");
            std::process::exit(1);
        }
    }
}

fn watch_tests(files: &[PathBuf], root: &PathBuf) {
    let root = std::fs::canonicalize(root).unwrap_or_else(|e| {
        eprintln!("Invalid root directory: {e}");
        std::process::exit(1);
    });

    let all_test_files = if files.is_empty() {
        bundler::find_test_files(&root)
    } else {
        files.to_vec()
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

    // Persistent runtime — created once, reused across watch cycles
    let rt = hermes::Runtime::new().unwrap_or_else(|e| {
        eprintln!("Hermes error: {e}");
        std::process::exit(1);
    });

    // Load harness once
    if rt.eval(HARNESS_JS, "harness.js").is_err() {
        eprintln!("Failed to load harness");
        std::process::exit(1);
    }

    let cfg = bundler::read_config(&root);

    // Initial run — full bundle to build dep graph + load vendor
    let depgraph = run_persistent_cycle(&rt, &all_test_files, &root, &cfg, true);

    // Watch loop
    let mut current_depgraph = depgraph;

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
                    if (name.ends_with(".test.ts")
                        || name.ends_with(".test.tsx")
                        || name.ends_with(".test.js"))
                        && !affected.contains(&canonical)
                    {
                        affected.push(canonical);
                    }
                }

                // If we couldn't determine affected tests, run all
                if affected.is_empty() {
                    let all = if files.is_empty() {
                        bundler::find_test_files(&root)
                    } else {
                        files.to_vec()
                    };
                    eprintln!(
                        "\x1b[2m[watch]\x1b[0m Could not determine affected tests, running all"
                    );
                    let new_graph = run_persistent_cycle(&rt, &all, &root, &cfg, false);
                    current_depgraph = new_graph;
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
                    let new_graph = run_persistent_cycle(&rt, &affected, &root, &cfg, false);
                    // Merge dep graphs
                    for (k, v) in new_graph {
                        let entry = current_depgraph.entry(k).or_default();
                        for test in v {
                            if !entry.contains(&test) {
                                entry.push(test);
                            }
                        }
                    }
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

/// Run a test cycle on a persistent runtime. On first_run, loads vendor/setup.
/// On reruns, resets registry and only re-evals the test group bundle.
fn run_persistent_cycle(
    rt: &hermes::Runtime,
    test_files: &[PathBuf],
    root: &PathBuf,
    cfg: &bundler::BundleConfig,
    first_run: bool,
) -> bundler::DepGraph {
    let start = Instant::now();

    let mock_modules = bundler::find_mock_modules(test_files);

    if first_run {
        // First run: use split mode to load vendor into __HT_mocks (persists across reruns)
        let split = match bundler::bundle_split(test_files, root, &mock_modules, cfg) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("\x1b[31mBundle split failed: {e}\x1b[0m");
                return std::collections::HashMap::new();
            }
        };

        // Eval vendor (setup + all node_modules → __HT_mocks)
        let eval_vendor = if let Some(bytecode) = bundler::compile_to_bytecode(&split.vendor, &root.join("vendor.js")) {
            rt.eval_bytes(&bytecode, "vendor.hbc")
        } else {
            rt.eval(&split.vendor, "vendor.js")
        };
        if let Err(e) = eval_vendor {
            eprintln!("\x1b[31mVendor eval failed: {e}\x1b[0m");
            return std::collections::HashMap::new();
        }

        // Eval all group bundles
        for (i, group) in split.groups.iter().enumerate() {
            let name = format!("group-{i}.js");
            let eval_result = if let Some(bytecode) = bundler::compile_to_bytecode(group, &root.join(&name)) {
                rt.eval_bytes(&bytecode, &format!("group-{i}.hbc"))
            } else {
                rt.eval(group, &name)
            };
            if let Err(e) = eval_result {
                eprintln!("\x1b[31mGroup {i} eval failed: {e}\x1b[0m");
                return std::collections::HashMap::new();
            }
        }

        // Run tests
        let runner = r#"
var __results = globalThis.__HT.runTests();
globalThis.__HT_results = JSON.stringify({
  tests: __results,
  passed: __results.filter(function(t) { return t.status === 'pass'; }).length,
  failed: __results.filter(function(t) { return t.status === 'fail'; }).length,
  skipped: __results.filter(function(t) { return t.status === 'skip'; }).length,
  total: __results.length
});
"#;
        if let Err(e) = rt.eval(runner, "runner.js") {
            eprintln!("Test runner failed: {e}");
            return std::collections::HashMap::new();
        }

        let elapsed = start.elapsed();
        match rt.eval("globalThis.__HT_results", "results") {
            Ok(json) => {
                print_summary(&json);
                eprintln!("\x1b[2mRan in {}ms\x1b[0m", elapsed.as_millis());
            }
            Err(e) => eprintln!("Failed to read results: {e}"),
        }

        // Build dep graph for change tracking (separate esbuild pass with metafile)
        let entry_content = bundler::generate_entry(test_files, None, &mock_modules, cfg, &[], Some(root));
        let entry_path = root.join(".hermes-test-entry.js");
        let depgraph = if std::fs::write(&entry_path, &entry_content).is_ok() {
            let dg = match bundler::bundle_with_depgraph(&entry_path, root, test_files, &mock_modules) {
                Ok((_, d)) => d,
                Err(_) => std::collections::HashMap::new(),
            };
            let _ = std::fs::remove_file(&entry_path);
            dg
        } else {
            std::collections::HashMap::new()
        };

        depgraph
    } else {
        // Rerun: reset harness state, bundle only affected files, re-eval
        let reset_js = "globalThis.__HT.resetRegistry(); globalThis.__HT_logs = [];";
        if let Err(e) = rt.eval(reset_js, "reset.js") {
            eprintln!("Failed to reset harness: {e}");
            return std::collections::HashMap::new();
        }

        // Bundle affected test files as a group (--packages=external if vendor loaded)
        let entry = bundler::generate_group_entry_pub(test_files, &mock_modules, Some(root));
        let entry_path = root.join(".hermes-test-rerun.js");
        if std::fs::write(&entry_path, &entry).is_err() {
            eprintln!("Failed to write rerun entry");
            return std::collections::HashMap::new();
        }

        let esbuild_path = match bundler::find_esbuild_pub(root) {
            Ok(p) => p,
            Err(_) => {
                eprintln!("esbuild not found");
                return std::collections::HashMap::new();
            }
        };

        let bundle = match bundler::bundle_esbuild_with_config_pub(
            &entry_path, &esbuild_path, &mock_modules, cfg, true,
        ) {
            Ok(b) => b,
            Err(e) => {
                let _ = std::fs::remove_file(&entry_path);
                eprintln!("\x1b[31mBundle failed: {e}\x1b[0m");
                return std::collections::HashMap::new();
            }
        };
        let _ = std::fs::remove_file(&entry_path);

        // Eval the group bundle in the persistent runtime
        let eval_result = if let Some(bytecode) = bundler::compile_to_bytecode(&bundle, &root.join("rerun.js")) {
            rt.eval_bytes(&bytecode, "rerun.hbc")
        } else {
            rt.eval(&bundle, "rerun.js")
        };
        if let Err(e) = eval_result {
            eprintln!("\x1b[31mTest execution failed: {e}\x1b[0m");
            return std::collections::HashMap::new();
        }

        // Run tests
        let runner = r#"
var __results = globalThis.__HT.runTests();
globalThis.__HT_results = JSON.stringify({
  tests: __results,
  passed: __results.filter(function(t) { return t.status === 'pass'; }).length,
  failed: __results.filter(function(t) { return t.status === 'fail'; }).length,
  skipped: __results.filter(function(t) { return t.status === 'skip'; }).length,
  total: __results.length
});
"#;
        if let Err(e) = rt.eval(runner, "runner.js") {
            eprintln!("Test runner failed: {e}");
            return std::collections::HashMap::new();
        }

        let elapsed = start.elapsed();
        match rt.eval("globalThis.__HT_results", "results") {
            Ok(json) => {
                print_summary(&json);
                eprintln!("\x1b[2mRan in {}ms (persistent)\x1b[0m", elapsed.as_millis());
            }
            Err(e) => eprintln!("Failed to read results: {e}"),
        }

        // No dep graph update on reruns (would need metafile which adds overhead)
        std::collections::HashMap::new()
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
                match level {
                    "warn" => eprintln!("\x1b[33m⚠ {msg}\x1b[0m"),
                    "error" => eprintln!("\x1b[31m✗ {msg}\x1b[0m"),
                    _ => eprintln!("  {msg}"),
                }
            }
        }
    }
}

fn print_results_with_time(json: &str, elapsed_ms: u128, file_count: usize) -> bool {
    let ok = print_results(json);
    eprintln!(" \x1b[2mFiles:\x1b[0m  {file_count}");
    eprintln!(" \x1b[2mTime:\x1b[0m   {:.2}s", elapsed_ms as f64 / 1000.0);
    ok
}

/// Summary-only: per-file results already printed live by the harness via __HT_print.
fn print_summary_with_time(json: &str, elapsed_ms: u128, file_count: usize) -> bool {
    let inner: String = serde_json::from_str(json).unwrap_or(json.to_string());
    let results: serde_json::Value = match serde_json::from_str(&inner) {
        Ok(v) => v,
        Err(_) => { eprintln!("{inner}"); return false; }
    };
    let passed = results["passed"].as_u64().unwrap_or(0);
    let failed = results["failed"].as_u64().unwrap_or(0);
    let total = results["total"].as_u64().unwrap_or(0);
    let secs = elapsed_ms as f64 / 1000.0;
    eprintln!();
    if failed == 0 {
        eprintln!(" \x1b[32mTests:\x1b[0m  {passed} passed, {total} total");
    } else {
        eprintln!(" \x1b[31mTests:\x1b[0m  \x1b[32m{passed} passed\x1b[0m, \x1b[31m{failed} failed\x1b[0m, {total} total");
    }
    eprintln!(" \x1b[2mFiles:\x1b[0m  {file_count}");
    eprintln!(" \x1b[2mTime:\x1b[0m   {secs:.2}s");
    failed == 0
}

/// Summary only — per-file results already printed live by harness via __HT_print.
fn print_summary(json: &str) -> bool {
    let inner: String = serde_json::from_str(json).unwrap_or(json.to_string());
    let results: serde_json::Value = match serde_json::from_str(&inner) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let passed = results["passed"].as_u64().unwrap_or(0);
    let failed = results["failed"].as_u64().unwrap_or(0);
    let total = results["total"].as_u64().unwrap_or(0);
    eprintln!();
    if failed == 0 {
        eprintln!(" \x1b[32mTests:\x1b[0m  {passed} passed, {total} total");
    } else {
        eprintln!(" \x1b[31mTests:\x1b[0m  \x1b[32m{passed} passed\x1b[0m, \x1b[31m{failed} failed\x1b[0m, {total} total");
    }
    failed == 0
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
