mod hermes;
mod metro;

use clap::{Parser, Subcommand};
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

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

    /// Bundler to use: "esbuild" (default, fast) or "metro" (RN-compatible)
    #[arg(long, default_value = "esbuild")]
    bundler: String,

    /// JavaScript file to evaluate directly (bypass Metro)
    #[arg(long)]
    eval: Option<String>,

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
        #[arg(long, default_value = "esbuild")]
        bundler: String,
    },

    /// Watch for file changes (legacy subcommand, prefer: hermes-test [files] --watch)
    Watch {
        files: Vec<PathBuf>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value = "esbuild")]
        bundler: String,
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
                bundler,
            } => {
                let bundler = parse_bundler(&bundler);
                run_tests(&files, &root, no_bundle, bundler);
            }
            Commands::Watch {
                files,
                root,
                bundler,
            } => {
                let bundler = parse_bundler(&bundler);
                watch_tests(&files, &root, bundler);
            }
        }
        return;
    }

    if let Some(ref path) = cli.eval {
        eval_file(path);
        return;
    }

    let bundler = parse_bundler(&cli.bundler);

    // Auto-detect project root: walk up from cwd to find package.json
    let root = cli.root.unwrap_or_else(|| find_project_root());

    // Resolve file arguments: if a name has no path separator, search the project
    let files = resolve_test_files(&cli.files, &root);

    if cli.watch {
        watch_tests(&files, &root, bundler);
    } else {
        run_tests(&files, &root, cli.no_bundle, bundler);
    }
}

fn parse_bundler(s: &str) -> Option<metro::Bundler> {
    match s {
        "metro" => Some(metro::Bundler::Metro),
        "esbuild" => Some(metro::Bundler::Esbuild),
        "auto" => None,
        _ => {
            eprintln!("Unknown bundler '{s}'. Use 'esbuild', 'metro', or 'auto'.");
            std::process::exit(1);
        }
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
    let all_tests = metro::find_test_files(root);

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

fn run_tests(files: &[PathBuf], root: &PathBuf, no_bundle: bool, bundler: Option<metro::Bundler>) {
    let root = std::fs::canonicalize(root).unwrap_or_else(|e| {
        eprintln!("Invalid root directory: {e}");
        std::process::exit(1);
    });

    // Find test files
    let test_files = if files.is_empty() {
        metro::find_test_files(&root)
    } else {
        files.to_vec()
    };

    if test_files.is_empty() {
        eprintln!("No test files found");
        std::process::exit(1);
    }

    eprintln!(
        "Found {} test file{}",
        test_files.len(),
        if test_files.len() == 1 { "" } else { "s" }
    );

    let rt = hermes::Runtime::new().unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1);
    });

    // Inject the harness
    rt.eval(HARNESS_JS, "hermes-test/harness.js").unwrap_or_else(|e| {
        eprintln!("Failed to load harness: {e}");
        std::process::exit(1);
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
var __results = globalThis.__metroTest.runTests();
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
        // Scan test files for mockModule() calls to determine external modules
        let mock_modules = metro::find_mock_modules(&test_files);
        if !mock_modules.is_empty() {
            eprintln!(
                "Mocked modules: {}",
                mock_modules.join(", ")
            );
        }

        // Metro mode: generate entry, bundle via Metro, eval bundle
        let entry_content = metro::generate_entry(&test_files, None, &mock_modules);

        // Write entry to a temp file
        let entry_path = root.join(".hermes-test-entry.js");
        std::fs::write(&entry_path, &entry_content).unwrap_or_else(|e| {
            eprintln!("Failed to write entry file: {e}");
            std::process::exit(1);
        });

        let bundler_name = match bundler {
            Some(metro::Bundler::Metro) => "Metro",
            Some(metro::Bundler::Esbuild) => "esbuild",
            None => "auto",
        };
        eprintln!("Bundling with {bundler_name}...");
        let bundle = match if let Some(b) = bundler {
            metro::bundle_with(b, &entry_path, &root, &mock_modules)
        } else {
            metro::bundle_auto(&entry_path, &root, &mock_modules)
        } {
            Ok(b) => b,
            Err(e) => {
                let _ = std::fs::remove_file(&entry_path);
                eprintln!("Metro bundling failed: {e}");
                std::process::exit(1);
            }
        };

        let _ = std::fs::remove_file(&entry_path);

        // Eval the bundle (registers and runs tests, stashes results on global)
        if let Err(e) = rt.eval(&bundle, "bundle.js") {
            eprintln!("Test execution failed: {e}");
            std::process::exit(1);
        }

        // Read back the results from the global
        match rt.eval("globalThis.__metroTestResults", "results") {
            Ok(json) => {
                if !print_results(&json) {
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("Failed to read test results: {e}");
                std::process::exit(1);
            }
        }
    }
}

fn watch_tests(files: &[PathBuf], root: &PathBuf, bundler: Option<metro::Bundler>) {
    let root = std::fs::canonicalize(root).unwrap_or_else(|e| {
        eprintln!("Invalid root directory: {e}");
        std::process::exit(1);
    });

    let all_test_files = if files.is_empty() {
        metro::find_test_files(&root)
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

    // Initial run — build full dep graph
    let depgraph = run_cycle_with_depgraph(&all_test_files, &root, bundler);

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
                        metro::find_test_files(&root)
                    } else {
                        files.to_vec()
                    };
                    eprintln!(
                        "\x1b[2m[watch]\x1b[0m Could not determine affected tests, running all"
                    );
                    current_depgraph = run_cycle_with_depgraph(&all, &root, bundler);
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
                    let new_graph = run_cycle_with_depgraph(&affected, &root, bundler);
                    // Merge: union test file lists (don't lose entries from other test files)
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

/// Run a test cycle and return the updated dependency graph.
fn run_cycle_with_depgraph(
    test_files: &[PathBuf],
    root: &PathBuf,
    bundler: Option<metro::Bundler>,
) -> metro::DepGraph {
    let start = Instant::now();

    // Scan for mockModule() calls
    let mock_modules = metro::find_mock_modules(test_files);

    let entry_content = metro::generate_entry(test_files, None, &mock_modules);
    let entry_path = root.join(".hermes-test-entry.js");
    if std::fs::write(&entry_path, &entry_content).is_err() {
        eprintln!("Failed to write entry file");
        return std::collections::HashMap::new();
    }

    // Try to get dep graph via esbuild metafile
    let (bundle, depgraph) = if bundler == Some(metro::Bundler::Metro) {
        let b = match metro::bundle_with(metro::Bundler::Metro, &entry_path, root, &mock_modules) {
            Ok(b) => b,
            Err(e) => {
                let _ = std::fs::remove_file(&entry_path);
                eprintln!("\x1b[31mBundle failed: {e}\x1b[0m");
                return std::collections::HashMap::new();
            }
        };
        (b, std::collections::HashMap::new())
    } else {
        match metro::bundle_with_depgraph(&entry_path, root, test_files, &mock_modules) {
            Ok((b, d)) => (b, d),
            Err(e) => {
                let _ = std::fs::remove_file(&entry_path);
                eprintln!("\x1b[31mBundle failed: {e}\x1b[0m");
                return std::collections::HashMap::new();
            }
        }
    };

    let _ = std::fs::remove_file(&entry_path);

    let rt = match hermes::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Hermes error: {e}");
            return depgraph;
        }
    };

    if rt.eval(HARNESS_JS, "harness.js").is_err() {
        eprintln!("Failed to load harness");
        return depgraph;
    }

    if let Err(e) = rt.eval(&bundle, "bundle.js") {
        eprintln!("\x1b[31mTest execution failed: {e}\x1b[0m");
        return depgraph;
    }

    let elapsed = start.elapsed();

    match rt.eval("globalThis.__metroTestResults", "results") {
        Ok(json) => {
            print_results(&json);
            eprintln!("\x1b[2mRan in {}ms\x1b[0m", elapsed.as_millis());
        }
        Err(e) => {
            eprintln!("Failed to read results: {e}");
        }
    }

    depgraph
}

fn print_results(json: &str) -> bool {
    // Returns true if all tests passed, false if any failed
    // The result is double-quoted JSON (JSON.stringify returns a string, which hermes_eval
    // wraps in another JSON.stringify). Parse accordingly.
    let inner: String = match serde_json::from_str(json) {
        Ok(s) => s,
        Err(_) => {
            // Already raw JSON
            println!("{json}");
            return false;
        }
    };

    // Parse the test results
    let results: serde_json::Value = match serde_json::from_str(&inner) {
        Ok(v) => v,
        Err(_) => {
            println!("{inner}");
            return false;
        }
    };

    let passed = results["passed"].as_u64().unwrap_or(0);
    let failed = results["failed"].as_u64().unwrap_or(0);
    let skipped = results["skipped"].as_u64().unwrap_or(0);
    let total = results["total"].as_u64().unwrap_or(0);

    // Print each test result
    if let Some(tests) = results["tests"].as_array() {
        for test in tests {
            let name = test["name"].as_str().unwrap_or("?");
            let status = test["status"].as_str().unwrap_or("?");
            let icon = match status {
                "pass" => "\x1b[32m✓\x1b[0m",
                "fail" => "\x1b[31m✗\x1b[0m",
                "skip" => "\x1b[33m○\x1b[0m",
                _ => "?",
            };
            eprintln!("  {icon} {name}");
            if status == "fail" {
                if let Some(error) = test["error"].as_str() {
                    eprintln!("    \x1b[31m{error}\x1b[0m");
                }
            }
        }
    }

    eprintln!();
    let status_line = format!(
        "{passed} passed{}{}, {total} total",
        if failed > 0 {
            format!(", \x1b[31m{failed} failed\x1b[0m")
        } else {
            String::new()
        },
        if skipped > 0 {
            format!(", {skipped} skipped")
        } else {
            String::new()
        }
    );
    eprintln!("{status_line}");

    failed == 0
}
