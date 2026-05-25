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
}

/// Extract a string value from JSON: "key": "value"
pub fn json_string_value(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let start = json.find(&pattern)?;
    let after = &json[start + pattern.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    if !rest.starts_with('"') { return None; }
    let val_end = rest[1..].find('"')?;
    Some(rest[1..1 + val_end].to_string())
}

/// Extract a string array from JSON: "key": ["a", "b"]
pub fn json_string_array(json: &str, key: &str) -> Option<Vec<String>> {
    let pattern = format!("\"{key}\"");
    let start = json.find(&pattern)?;
    let after = &json[start + pattern.len()..];
    let arr_start = after.find('[')?;
    let arr_end = after[arr_start..].find(']')?;
    let arr = &after[arr_start + 1..arr_start + arr_end];
    Some(arr.split(',')
        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Extract key-value pairs from a JSON object: "key": { "a": "b", "c": "d" }
pub fn json_object_entries(json: &str, key: &str) -> Option<Vec<(String, String)>> {
    let pattern = format!("\"{key}\"");
    let start = json.find(&pattern)?;
    let after = &json[start + pattern.len()..];
    let brace_start = after.find('{')?;
    let rest = &after[brace_start..];
    let mut depth = 0;
    let mut end = 0;
    for (i, ch) in rest.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => { depth -= 1; if depth == 0 { end = i; break; } }
            _ => {}
        }
    }
    let inner = &rest[1..end];
    let re = regex::Regex::new(r#""([^"]+)"\s*:\s*"([^"]+)""#).ok()?;
    Some(re.captures_iter(inner)
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .collect())
}

/// Read "paths" from a tsconfig.json file and add as esbuild aliases.
/// Follows "extends" to read paths from parent tsconfigs (standard TS behavior).
pub fn read_tsconfig_paths(base_dir: &Path, tsconfig_path: &Path, aliases: &mut Vec<(String, String)>) {
    let Ok(content) = std::fs::read_to_string(tsconfig_path) else { return };

    // Follow "extends" first — parent paths are overridden by child paths
    if let Some(extends_val) = json_string_value(&content, "extends") {
        let parent_path = tsconfig_path.parent().unwrap_or(base_dir).join(&extends_val);
        if parent_path.exists() {
            let parent_dir = parent_path.parent().unwrap_or(base_dir);
            read_tsconfig_paths(parent_dir, &parent_path, aliases);
        }
    }

    // Parse "paths" from this tsconfig
    let Some(paths_start) = content.find("\"paths\"") else { return };
    let Some(brace_start) = content[paths_start..].find('{') else { return };
    let rest = &content[paths_start + brace_start..];

    let mut depth = 0;
    let mut end = 0;
    for (i, ch) in rest.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => { depth -= 1; if depth == 0 { end = i + 1; break; } }
            _ => {}
        }
    }
    let paths_block = &rest[..end];
    let mut i = 0;
    while let Some(key_start) = paths_block[i..].find('"') {
        let ks = i + key_start + 1;
        let Some(key_end) = paths_block[ks..].find('"') else { break };
        let key = &paths_block[ks..ks + key_end];
        let after_key = ks + key_end + 1;
        let Some(val_start) = paths_block[after_key..].find('"') else { break };
        let vs = after_key + val_start + 1;
        let Some(val_end) = paths_block[vs..].find('"') else { break };
        let val = &paths_block[vs..vs + val_end];

        let alias = key.trim_end_matches("/*").trim_end_matches('*');
        let target = val.trim_end_matches("/*").trim_end_matches('*');
        if !alias.is_empty() && !target.is_empty() {
            let target_path = if target.starts_with("./") || target.starts_with("../") {
                let joined = base_dir.join(target);
                joined.canonicalize().unwrap_or(joined).to_string_lossy().to_string()
            } else {
                target.to_string()
            };
            aliases.push((alias.to_string(), target_path));
        }
        i = vs + val_end + 1;
    }
}

/// Read hermes-test.config.json and resolve tsconfig paths.
/// Config: { "root": "../..", "tsconfig": "../../tsconfig.json", "external": ["zod", ...] }
pub fn read_config(project_root: &Path) -> BundleConfig {
    let mut config = BundleConfig { aliases: Vec::new(), externals: Vec::new(), shims: Vec::new(), wrapper_shims: Vec::new(), root: None, split: false, test_match: None };

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
        // No config — read tsconfig.json from project root only
        read_tsconfig_paths(project_root, &project_root.join("tsconfig.json"), &mut config.aliases);
        return config;
    };

    // "root" — monorepo workspace root
    if let Some(val) = json_string_value(&content, "root") {
        config.root = Some(config_dir.join(&val));
    }

    // "tsconfig" — path to tsconfig with aliases
    if let Some(val) = json_string_value(&content, "tsconfig") {
        let tsconfig_path = config_dir.join(&val);
        let tsconfig_dir = tsconfig_path.parent().unwrap_or(&config_dir);
        read_tsconfig_paths(tsconfig_dir, &tsconfig_path, &mut config.aliases);
    } else {
        read_tsconfig_paths(project_root, &project_root.join("tsconfig.json"), &mut config.aliases);
    }

    // "externals" (or "external") — modules to externalize
    if let Some(items) = json_string_array(&content, "externals")
        .or_else(|| json_string_array(&content, "external")) {
        config.externals = items;
    }

    // "shims" — custom shim files for native modules { "react-native": "./shims/rn.js" }
    // Values starting with "hermes-test/shims/" are built-in wrapper shims (esbuild alias mechanism).
    // Other values are user-provided replacement shims (__HT_mocks mechanism).
    if let Some(obj) = json_object_entries(&content, "shims") {
        for (key, val) in obj {
            if val.starts_with("hermes-test/shims/") {
                let builtin_name = val.trim_start_matches("hermes-test/shims/").to_string();
                config.wrapper_shims.push((key, builtin_name));
            } else {
                let resolved = config_dir.join(&val).to_string_lossy().to_string();
                config.shims.push((key, resolved));
            }
        }
    }

    // "split" — enable vendor/group bundle splitting
    if content.contains("\"split\"") && content.contains("true") {
        config.split = true;
    }

    // "testMatch" — custom test file pattern (e.g. ".hermes.test.ts")
    if let Some(val) = json_string_value(&content, "testMatch") {
        config.test_match = Some(val);
    }

    // Auto-detect native modules by scanning for ios/android dirs.
    // Skip packages that are tsconfig aliases (e.g. @topdanmark/mobile-insurance-app → ./src).
    let alias_prefixes: Vec<String> = config.aliases.iter().map(|(a, _)| a.clone()).collect();
    let auto_externals = detect_native_modules(project_root, &config);
    for ext in auto_externals {
        if config.externals.contains(&ext) { continue; }
        // Skip if this package is a tsconfig alias target (it's the app's own source)
        if alias_prefixes.iter().any(|a| ext.starts_with(a) || a.starts_with(&ext)) { continue; }
        // Skip dev/build tooling that happens to have native dirs (pattern-based, not hardcoded)
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
/// Returns package names (e.g. "react-native-reanimated", "@react-native-firebase/*").
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
        // Top-level packages: node_modules/<pkg>/
        if let Ok(entries) = std::fs::read_dir(nm_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') { continue; }
                if name.starts_with('@') {
                    // Scoped packages: node_modules/@scope/<pkg>/
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

/// Check if a package directory contains native code indicators.
pub fn is_native_package(pkg_dir: &Path) -> bool {
    pkg_dir.join("ios").is_dir()
        || pkg_dir.join("android").is_dir()
        || has_podspec(pkg_dir)
        || has_expo_plugin(pkg_dir)
}

pub fn has_podspec(dir: &Path) -> bool {
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
