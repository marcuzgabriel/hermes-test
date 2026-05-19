mod hermes;
mod metro;

use clap::{Parser, Subcommand};
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

// Embed the harness bundle at compile time
const HARNESS_JS: &str = include_str!("../../../packages/metro-test/dist/harness.bundle.js");

#[derive(Parser)]
#[command(name = "metro-test", version, about = "Fast test runner for React Native")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// JavaScript file to evaluate directly (bypass Metro)
    #[arg(long)]
    eval: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run tests
    Run {
        /// Specific test files to run (if omitted, finds all *.test.ts files)
        files: Vec<PathBuf>,

        /// Project root directory
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Skip bundling, run raw JS files directly
        #[arg(long)]
        no_bundle: bool,

        /// Bundler to use: "esbuild" (default, fast) or "metro" (RN-compatible)
        #[arg(long, default_value = "esbuild")]
        bundler: String,
    },

    /// Watch for file changes and rerun tests
    Watch {
        /// Specific test files to watch (if omitted, finds all *.test.ts files)
        files: Vec<PathBuf>,

        /// Project root directory
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Bundler to use: "esbuild" (default, fast) or "metro" (RN-compatible)
        #[arg(long, default_value = "esbuild")]
        bundler: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Run {
            files,
            root,
            no_bundle,
            bundler,
        }) => {
            let bundler = match bundler.as_str() {
                "metro" => Some(metro::Bundler::Metro),
                "esbuild" => Some(metro::Bundler::Esbuild),
                "auto" => None,
                _ => {
                    eprintln!("Unknown bundler '{bundler}'. Use 'esbuild', 'metro', or 'auto'.");
                    std::process::exit(1);
                }
            };
            run_tests(&files, &root, no_bundle, bundler);
        }
        Some(Commands::Watch {
            files,
            root,
            bundler,
        }) => {
            let bundler = match bundler.as_str() {
                "metro" => Some(metro::Bundler::Metro),
                "esbuild" => Some(metro::Bundler::Esbuild),
                "auto" => None,
                _ => {
                    eprintln!("Unknown bundler '{bundler}'. Use 'esbuild', 'metro', or 'auto'.");
                    std::process::exit(1);
                }
            };
            watch_tests(&files, &root, bundler);
        }
        None => {
            if let Some(ref path) = cli.eval {
                eval_file(path);
            } else {
                // Default: run tests in current directory
                run_tests(&[], &PathBuf::from("."), false, None);
            }
        }
    }
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
    rt.eval(HARNESS_JS, "metro-test/harness.js").unwrap_or_else(|e| {
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
            Ok(json) => print_results(&json),
            Err(e) => {
                eprintln!("Test execution failed: {e}");
                std::process::exit(1);
            }
        }
    } else {
        // Metro mode: generate entry, bundle via Metro, eval bundle
        let entry_content = metro::generate_entry(&test_files, None);

        // Write entry to a temp file
        let entry_path = root.join(".metro-test-entry.js");
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
            metro::bundle_with(b, &entry_path, &root)
        } else {
            metro::bundle_auto(&entry_path, &root)
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
            Ok(json) => print_results(&json),
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

    // Initial test discovery
    let test_files = if files.is_empty() {
        metro::find_test_files(&root)
    } else {
        files.to_vec()
    };

    if test_files.is_empty() {
        eprintln!("No test files found");
        std::process::exit(1);
    }

    // Determine watch directories: project src dirs (not node_modules)
    let watch_dir = root.clone();

    // Set up file watcher with 50ms debounce
    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(Duration::from_millis(50), tx)
        .unwrap_or_else(|e| {
            eprintln!("Failed to create file watcher: {e}");
            std::process::exit(1);
        });

    use notify_debouncer_mini::notify::RecursiveMode;
    debouncer
        .watcher()
        .watch(&watch_dir, RecursiveMode::Recursive)
        .unwrap_or_else(|e| {
            eprintln!("Failed to watch directory: {e}");
            std::process::exit(1);
        });

    eprintln!("\x1b[2m[watch]\x1b[0m Watching for changes in {}", root.display());
    eprintln!("\x1b[2m[watch]\x1b[0m Press Ctrl+C to stop\n");

    // Initial run
    run_cycle(&test_files, &root, bundler);

    // Watch loop
    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
                // Filter: only care about .ts/.tsx/.js/.jsx changes, skip node_modules
                let relevant = events.iter().any(|e| {
                    let p = e.path.to_string_lossy();
                    !p.contains("node_modules")
                        && !p.contains(".metro-test-")
                        && !p.contains("/target/")
                        && (p.ends_with(".ts")
                            || p.ends_with(".tsx")
                            || p.ends_with(".js")
                            || p.ends_with(".jsx"))
                        && e.kind == DebouncedEventKind::Any
                });

                if !relevant {
                    continue;
                }

                // Show which files changed
                let changed: Vec<String> = events
                    .iter()
                    .filter_map(|e| {
                        let p = e.path.to_string_lossy();
                        if p.contains("node_modules") || p.contains(".metro-test-") {
                            return None;
                        }
                        e.path
                            .strip_prefix(&root)
                            .ok()
                            .map(|rel| rel.to_string_lossy().to_string())
                    })
                    .collect();

                if !changed.is_empty() {
                    eprintln!(
                        "\n\x1b[2m[watch]\x1b[0m Changed: {}",
                        changed.join(", ")
                    );
                }

                // Re-discover test files (new files may have been added)
                let test_files = if files.is_empty() {
                    metro::find_test_files(&root)
                } else {
                    files.to_vec()
                };

                if !test_files.is_empty() {
                    run_cycle(&test_files, &root, bundler);
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

/// Run a single test cycle: bundle + eval + print results. Returns true if all passed.
fn run_cycle(test_files: &[PathBuf], root: &PathBuf, bundler: Option<metro::Bundler>) -> bool {
    let start = Instant::now();

    let entry_content = metro::generate_entry(test_files, None);
    let entry_path = root.join(".metro-test-entry.js");
    if std::fs::write(&entry_path, &entry_content).is_err() {
        eprintln!("Failed to write entry file");
        return false;
    }

    let bundle = match if let Some(b) = bundler {
        metro::bundle_with(b, &entry_path, root)
    } else {
        metro::bundle_auto(&entry_path, root)
    } {
        Ok(b) => b,
        Err(e) => {
            let _ = std::fs::remove_file(&entry_path);
            eprintln!("\x1b[31mBundle failed: {e}\x1b[0m");
            return false;
        }
    };

    let _ = std::fs::remove_file(&entry_path);

    // Fresh Hermes runtime for each run (clean state)
    let rt = match hermes::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Hermes error: {e}");
            return false;
        }
    };

    if rt.eval(HARNESS_JS, "harness.js").is_err() {
        eprintln!("Failed to load harness");
        return false;
    }

    if let Err(e) = rt.eval(&bundle, "bundle.js") {
        eprintln!("\x1b[31mTest execution failed: {e}\x1b[0m");
        return false;
    }

    let elapsed = start.elapsed();

    match rt.eval("globalThis.__metroTestResults", "results") {
        Ok(json) => {
            print_results(&json);
            eprintln!("\x1b[2mRan in {}ms\x1b[0m", elapsed.as_millis());
            // Check if all passed
            if let Ok(inner) = serde_json::from_str::<String>(&json) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&inner) {
                    return v["failed"].as_u64().unwrap_or(1) == 0;
                }
            }
            false
        }
        Err(e) => {
            eprintln!("Failed to read results: {e}");
            false
        }
    }
}

fn print_results(json: &str) {
    // The result is double-quoted JSON (JSON.stringify returns a string, which hermes_eval
    // wraps in another JSON.stringify). Parse accordingly.
    let inner: String = match serde_json::from_str(json) {
        Ok(s) => s,
        Err(_) => {
            // Already raw JSON
            println!("{json}");
            return;
        }
    };

    // Parse the test results
    let results: serde_json::Value = match serde_json::from_str(&inner) {
        Ok(v) => v,
        Err(_) => {
            println!("{inner}");
            return;
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

    if failed > 0 {
        std::process::exit(1);
    }
}
