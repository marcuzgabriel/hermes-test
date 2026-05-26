use std::path::{Path, PathBuf};

/// Read path aliases from tsconfig.json "compilerOptions.paths" and hermes-test.config.json.
/// Returns Vec<(alias, target_path)> for esbuild --alias flags.
#[derive(Clone)]
pub struct BundleConfig {
    pub aliases: Vec<(String, String)>,
    pub externals: Vec<String>,
    pub shims: Vec<(String, String)>, // (module_name, file_path) — replacement shims
    pub wrapper_shims: Vec<(String, String)>, // (module_name, builtin_name) — hermes-test/shims/*
    pub root: Option<PathBuf>,
    pub split: bool,
    pub test_match: Option<String>, // e.g. ".hermes.test.ts" — only discover matching files
    pub coverage_threshold: Option<f64>, // e.g. 65.0 — fail if total coverage below this %
}

/// Strip JS-style comments (// and /* */) from JSON-like content.
/// tsconfig.json and hermes-test.config.json allow comments.
fn strip_json_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        if in_string {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                out.push(bytes[i] as char);
                out.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if bytes[i] == b'"' { in_string = false; }
            out.push(bytes[i] as char);
            i += 1;
        } else if bytes[i] == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
        } else if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            // Line comment — skip to end of line
            while i < bytes.len() && bytes[i] != b'\n' { i += 1; }
        } else if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            // Block comment — skip to */
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') { i += 1; }
            i += 2;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// Parse a JSON file that may contain comments and trailing commas.
fn parse_json_file(content: &str) -> Option<serde_json::Value> {
    let clean = strip_json_comments(content);
    // Remove trailing commas before } and ] (common in tsconfig/config files)
    let clean = regex::Regex::new(r",\s*([}\]])")
        .unwrap()
        .replace_all(&clean, "$1")
        .to_string();
    serde_json::from_str(&clean).ok()
}

/// Read "paths" from a tsconfig.json file and add as esbuild aliases.
/// Follows "extends" to read paths from parent tsconfigs (standard TS behavior).
pub fn read_tsconfig_paths(base_dir: &Path, tsconfig_path: &Path, aliases: &mut Vec<(String, String)>) {
    let Ok(content) = std::fs::read_to_string(tsconfig_path) else { return };
    let Some(json) = parse_json_file(&content) else { return };

    // Follow "extends" first — parent paths are overridden by child paths
    if let Some(extends_val) = json["extends"].as_str() {
        let parent_path = tsconfig_path.parent().unwrap_or(base_dir).join(extends_val);
        if parent_path.exists() {
            let parent_dir = parent_path.parent().unwrap_or(base_dir);
            read_tsconfig_paths(parent_dir, &parent_path, aliases);
        }
    }

    // Parse "paths" from compilerOptions
    if let Some(paths) = json["compilerOptions"]["paths"].as_object() {
        for (key, val) in paths {
            let Some(arr) = val.as_array() else { continue };
            let Some(first) = arr.first().and_then(|v| v.as_str()) else { continue };

            let alias = key.trim_end_matches("/*").trim_end_matches('*');
            let target = first.trim_end_matches("/*").trim_end_matches('*');
            if alias.is_empty() || target.is_empty() { continue; }

            let target_path = if target.starts_with("./") || target.starts_with("../") {
                let joined = base_dir.join(target);
                joined.canonicalize().unwrap_or(joined).to_string_lossy().to_string()
            } else {
                target.to_string()
            };
            aliases.push((alias.to_string(), target_path));
        }
    }
}

/// Read hermes-test.config.json and resolve tsconfig paths.
pub fn read_config(project_root: &Path) -> BundleConfig {
    let mut config = BundleConfig {
        aliases: Vec::new(), externals: Vec::new(), shims: Vec::new(),
        wrapper_shims: Vec::new(), root: None, split: false, test_match: None,
        coverage_threshold: None,
    };

    // Find hermes-test.config.json — check project root, then walk up
    let mut search_dir = project_root.canonicalize().unwrap_or_else(|_| project_root.to_path_buf());
    let config_content = loop {
        let path = search_dir.join("hermes-test.config.json");
        if let Ok(content) = std::fs::read_to_string(&path) {
            break Some((search_dir.clone(), content));
        }
        if !search_dir.pop() { break None; }
    };

    let Some((config_dir, content)) = config_content else {
        read_tsconfig_paths(project_root, &project_root.join("tsconfig.json"), &mut config.aliases);
        return config;
    };

    let json = match parse_json_file(&content) {
        Some(j) => j,
        None => {
            eprintln!("Warning: failed to parse hermes-test.config.json");
            read_tsconfig_paths(project_root, &project_root.join("tsconfig.json"), &mut config.aliases);
            return config;
        }
    };

    // "root" — monorepo workspace root
    if let Some(val) = json["root"].as_str() {
        config.root = Some(config_dir.join(val));
    }

    // "tsconfig" — path to tsconfig with aliases
    if let Some(val) = json["tsconfig"].as_str() {
        let tsconfig_path = config_dir.join(val);
        let tsconfig_dir = tsconfig_path.parent().unwrap_or(&config_dir);
        read_tsconfig_paths(tsconfig_dir, &tsconfig_path, &mut config.aliases);
    } else {
        read_tsconfig_paths(project_root, &project_root.join("tsconfig.json"), &mut config.aliases);
    }

    // "externals" (or "external") — modules to externalize
    let ext_key = if json["externals"].is_array() { "externals" } else { "external" };
    if let Some(arr) = json[ext_key].as_array() {
        config.externals = arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
    }

    // "shims" — module replacements
    if let Some(obj) = json["shims"].as_object() {
        for (key, val) in obj {
            if let Some(val_str) = val.as_str() {
                if val_str.starts_with("hermes-test/shims/") {
                    let builtin_name = val_str.trim_start_matches("hermes-test/shims/").to_string();
                    config.wrapper_shims.push((key.clone(), builtin_name));
                } else {
                    let resolved = config_dir.join(val_str).to_string_lossy().to_string();
                    config.shims.push((key.clone(), resolved));
                }
            }
        }
    }

    // "split" — enable vendor/group bundle splitting
    if json["split"].as_bool() == Some(true) {
        config.split = true;
    }

    // "testMatch" — custom test file pattern
    if let Some(val) = json["testMatch"].as_str() {
        config.test_match = Some(val.to_string());
    }

    // "coverageThreshold" — minimum coverage % (e.g. 65)
    if let Some(val) = json["coverageThreshold"].as_f64() {
        config.coverage_threshold = Some(val);
    }

    // Auto-detect native modules by scanning for ios/android dirs.
    let alias_prefixes: Vec<String> = config.aliases.iter().map(|(a, _)| a.clone()).collect();
    let auto_externals = detect_native_modules(project_root, &config);
    for ext in auto_externals {
        if config.externals.contains(&ext) { continue; }
        if alias_prefixes.iter().any(|a| ext.starts_with(a) || a.starts_with(&ext)) { continue; }
        // Skip dev/build tooling that happens to have native dirs (pattern-based)
        if ext == "detox"
            || ext.starts_with("expo-dev-")
            || ext.starts_with("expo-module")
            || ext.contains("autolinking")
            || ext.starts_with("jest-")
        { continue; }
        config.externals.push(ext);
    }

    config
}

/// Detect native modules by checking for ios/ or android/ directories.
pub fn detect_native_modules(project_root: &Path, cfg: &BundleConfig) -> Vec<String> {
    let mut native = Vec::new();
    let nm_dirs: Vec<PathBuf> = {
        let mut dirs = vec![project_root.join("node_modules")];
        if let Some(ref root) = cfg.root {
            let root_nm = root.join("node_modules");
            if root_nm.is_dir() && !dirs.contains(&root_nm) {
                dirs.push(root_nm);
            }
        }
        dirs
    };

    for nm_dir in &nm_dirs {
        if let Ok(entries) = std::fs::read_dir(nm_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') { continue; }
                if name.starts_with('@') {
                    let scope_dir = entry.path();
                    if let Ok(sub_entries) = std::fs::read_dir(&scope_dir) {
                        for sub in sub_entries.flatten() {
                            let sub_name = sub.file_name().to_string_lossy().to_string();
                            if sub_name.starts_with('.') { continue; }
                            if is_native_package(&sub.path()) {
                                let full = format!("{name}/{sub_name}");
                                if !native.contains(&full) {
                                    native.push(full);
                                }
                            }
                        }
                    }
                } else if is_native_package(&entry.path()) {
                    if !native.contains(&name) {
                        native.push(name);
                    }
                }
            }
        }
    }
    native
}

fn is_native_package(pkg_dir: &Path) -> bool {
    pkg_dir.join("ios").is_dir()
        || pkg_dir.join("android").is_dir()
        || has_podspec(pkg_dir)
        || has_expo_plugin(pkg_dir)
}

fn has_podspec(dir: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if let Some(name) = e.file_name().to_str() {
                if name.ends_with(".podspec") { return true; }
            }
        }
    }
    false
}

pub fn has_expo_plugin(dir: &Path) -> bool {
    dir.join("app.plugin.js").is_file()
}
