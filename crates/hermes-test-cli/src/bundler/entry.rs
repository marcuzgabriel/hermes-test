use std::path::{Path, PathBuf};

use super::config::BundleConfig;

/// Find all test files. Uses testMatch pattern from config if set,
/// otherwise matches *.test.ts, *.test.tsx, *.test.js, *.test.jsx
pub fn find_test_files(root: &Path) -> Vec<PathBuf> {
    let cfg = super::config::read_config(root);
    find_test_files_with_pattern(root, cfg.test_match.as_deref())
}

pub fn find_test_files_with_pattern(root: &Path, pattern: Option<&str>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk_dir(root, &mut files, pattern);
    files.sort();
    files
}

fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>, pattern: Option<&str>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() {
            if name_str == "node_modules"
                || name_str == ".git"
                || name_str == "vendor"
                || name_str == "target"
                || name_str == ".hermes-test-cache"
            {
                continue;
            }
            walk_dir(&path, files, pattern);
        } else if let Some(pat) = pattern {
            // Custom pattern: match suffix (e.g. ".hermes.test.ts") + tsx variant
            if name_str.ends_with(pat)
                || (pat.ends_with(".ts") && name_str.ends_with(&format!("{}x", pat)))
            {
                files.push(path);
            }
        } else if name_str.ends_with(".test.ts")
            || name_str.ends_with(".test.tsx")
            || name_str.ends_with(".test.js")
            || name_str.ends_with(".test.jsx")
        {
            files.push(path);
        }
    }
}

/// Scan test files for ht.mock() and ht.shallow() calls and return module paths.
/// ht.shallow() scans the target component file and adds all its internal imports
/// so shadow wrappers are created for them.
pub fn find_mock_modules(test_files: &[PathBuf]) -> Vec<String> {
    find_mock_modules_with_aliases(test_files, &[])
}

pub fn find_mock_modules_with_aliases(test_files: &[PathBuf], aliases: &[String]) -> Vec<String> {
    find_mock_modules_with_alias_pairs(test_files, aliases, &[])
}

pub fn find_mock_modules_with_alias_pairs(test_files: &[PathBuf], aliases: &[String], alias_pairs: &[(String, String)]) -> Vec<String> {
    let mut mocks = Vec::new();
    let re_mock = regex::Regex::new(r#"ht\.mock\(\s*['"]([^'"]+)['"]\s*,"#).ok();
    let re_shallow = regex::Regex::new(r#"ht\.shallow\(\s*['"]([^'"]+)['"]\s*\)"#).ok();

    for file in test_files {
        let content = match std::fs::read_to_string(file) { Ok(c) => c, Err(_) => continue };

        // Explicit ht.mock() paths
        if let Some(ref re) = re_mock {
            for cap in re.captures_iter(&content) {
                let path = cap[1].to_string();
                if !mocks.contains(&path) { mocks.push(path); }
            }
        }

        // ht.shallow() — scan component for JSX-rendered imports
        if let Some(ref _re) = re_shallow {
            let auto_mocks = scan_shallow_auto_mocks_with_pairs(file, aliases, alias_pairs);
            for (path, _, _) in auto_mocks {
                if !mocks.contains(&path) { mocks.push(path); }
            }
        }
    }
    mocks
}

/// Scan a test file's ht.shallow() target for components used in JSX.
/// For modules that contain at least one JSX component, ALL named imports are collected.
/// JSX components get createElement stubs; non-JSX exports (hooks, utilities) get
/// generic function stubs. This prevents barrel-file crashes from breaking fallthrough.
/// Returns (module_path, jsx_names, other_names) for auto-mock generation.
pub fn scan_shallow_auto_mocks(test_file: &Path, aliases: &[String]) -> Vec<(String, Vec<String>, Vec<String>)> {
    scan_shallow_auto_mocks_with_pairs(test_file, aliases, &[])
}

pub fn scan_shallow_auto_mocks_with_pairs(test_file: &Path, aliases: &[String], alias_pairs: &[(String, String)]) -> Vec<(String, Vec<String>, Vec<String>)> {
    let content = match std::fs::read_to_string(test_file) { Ok(c) => c, Err(_) => return vec![] };
    let re_shallow = match regex::Regex::new(r#"ht\.shallow\(\s*['"]([^'"]+)['"]\s*\)"#) {
        Ok(r) => r, Err(_) => return vec![],
    };
    let shallow_path = match re_shallow.captures(&content) {
        Some(cap) => cap[1].to_string(), None => return vec![],
    };
    // Try relative resolution first, then alias resolution
    let comp_file = match resolve_relative_file(test_file, &shallow_path) {
        Some(f) => f,
        None => {
            // Try alias resolution: find matching alias and resolve to target path
            let mut resolved = None;
            for (alias, target) in alias_pairs {
                if shallow_path.starts_with(alias.as_str()) {
                    let remainder = &shallow_path[alias.len()..];
                    let remainder = remainder.trim_start_matches('/');
                    let target_path = Path::new(target).join(remainder);
                    // Try with extensions
                    for ext in &[".tsx", ".ts", ".jsx", ".js"] {
                        let c = target_path.with_extension(&ext[1..]);
                        if c.exists() { resolved = Some(c); break; }
                    }
                    if resolved.is_none() {
                        for idx in &["index.tsx", "index.ts", "index.jsx", "index.js"] {
                            let c = target_path.join(idx);
                            if c.exists() { resolved = Some(c); break; }
                        }
                    }
                    if resolved.is_some() { break; }
                }
            }
            match resolved { Some(f) => f, None => return vec![] }
        },
    };
    let comp_source = match std::fs::read_to_string(&comp_file) { Ok(c) => c, Err(_) => return vec![] };

    // Step 1: Find all component names used in JSX: <Name or </Name
    let re_jsx = regex::Regex::new(r#"<\s*/?\s*([A-Z]\w*)\b"#).unwrap();
    let mut jsx_idents: std::collections::HashSet<String> = std::collections::HashSet::new();
    for cap in re_jsx.captures_iter(&comp_source) {
        jsx_idents.insert(cap[1].to_string());
    }
    if jsx_idents.is_empty() { return vec![]; }

    // Step 2: Collect imports — modules with at least one JSX ident get ALL imports
    let re_mock = regex::Regex::new(r#"ht\.mock\(\s*['"]([^'"]+)['"]\s*,"#).unwrap();
    let explicit: Vec<String> = re_mock.captures_iter(&content).map(|c| c[1].to_string()).collect();

    let re_default = regex::Regex::new(r#"import\s+(\w+)\s+from\s+['"]([^'"]+)['"]"#).unwrap();
    let re_named = regex::Regex::new(r#"import\s+\{([^}]+)\}\s+from\s+['"]([^'"]+)['"]"#).unwrap();
    // Combined: import Default, { Named1, Named2 } from 'path'
    let re_combined = regex::Regex::new(r#"import\s+(\w+)\s*,\s*\{([^}]+)\}\s+from\s+['"]([^'"]+)['"]"#).unwrap();

    // First pass: find which modules have JSX components
    let mut modules_with_jsx: std::collections::HashSet<String> = std::collections::HashSet::new();

    for cap in re_default.captures_iter(&comp_source) {
        let name = cap[1].to_string();
        let path = cap[2].to_string();
        if jsx_idents.contains(&name) { modules_with_jsx.insert(path); }
    }
    for cap in re_named.captures_iter(&comp_source) {
        let spec = &cap[1];
        let path = cap[2].to_string();
        for item in spec.split(',') {
            let item = item.trim();
            if item.is_empty() || item.starts_with("type ") { continue; }
            let local = item.split(" as ").last().unwrap_or(item).trim();
            if jsx_idents.contains(local) { modules_with_jsx.insert(path.clone()); break; }
        }
    }
    // Combined: import Default, { Named } from 'path'
    for cap in re_combined.captures_iter(&comp_source) {
        let default_name = cap[1].to_string();
        let spec = &cap[2];
        let path = cap[3].to_string();
        if jsx_idents.contains(&default_name) { modules_with_jsx.insert(path.clone()); }
        for item in spec.split(',') {
            let item = item.trim();
            if item.is_empty() || item.starts_with("type ") { continue; }
            let local = item.split(" as ").last().unwrap_or(item).trim();
            if jsx_idents.contains(local) { modules_with_jsx.insert(path.clone()); break; }
        }
    }

    // Second pass: for modules with JSX, collect ALL imports (jsx + other)
    let mut result: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();

    for cap in re_default.captures_iter(&comp_source) {
        let name = cap[1].to_string();
        let path = cap[2].to_string();
        if !modules_with_jsx.contains(&path) { continue; }
        if explicit.contains(&path) { continue; }
        if path == "react" || path.starts_with("react/") { continue; }
        if path == "react-native" || path.starts_with("react-native/") { continue; }

        let is_jsx = jsx_idents.contains(&name);
        if let Some((_, jsx, other)) = result.iter_mut().find(|(p, _, _)| p == &path) {
            if is_jsx { if !jsx.contains(&name) { jsx.push(name); } }
            else { if !other.contains(&name) { other.push(name); } }
        } else {
            if is_jsx { result.push((path, vec![name], vec![])); }
            else { result.push((path, vec![], vec![name])); }
        }
    }

    for cap in re_named.captures_iter(&comp_source) {
        let spec = &cap[1];
        let path = cap[2].to_string();
        if !modules_with_jsx.contains(&path) { continue; }
        if explicit.contains(&path) { continue; }
        if path == "react" || path.starts_with("react/") { continue; }
        if path == "react-native" || path.starts_with("react-native/") { continue; }

        let mut jsx_names: Vec<String> = Vec::new();
        let mut other_names: Vec<String> = Vec::new();
        for item in spec.split(',') {
            let item = item.trim();
            if item.is_empty() || item.starts_with("type ") { continue; }
            let local = item.split(" as ").last().unwrap_or(item).trim();
            if jsx_idents.contains(local) { jsx_names.push(local.to_string()); }
            else { other_names.push(local.to_string()); }
        }
        if jsx_names.is_empty() && other_names.is_empty() { continue; }

        if let Some((_, existing_jsx, existing_other)) = result.iter_mut().find(|(p, _, _)| p == &path) {
            for n in jsx_names { if !existing_jsx.contains(&n) { existing_jsx.push(n); } }
            for n in other_names { if !existing_other.contains(&n) { existing_other.push(n); } }
        } else {
            result.push((path, jsx_names, other_names));
        }
    }

    // Combined: import Default, { Named1, Named2 } from 'path'
    for cap in re_combined.captures_iter(&comp_source) {
        let default_name = cap[1].to_string();
        let spec = &cap[2];
        let path = cap[3].to_string();
        if !modules_with_jsx.contains(&path) { continue; }
        if explicit.contains(&path) { continue; }
        if path == "react" || path.starts_with("react/") { continue; }
        if path == "react-native" || path.starts_with("react-native/") { continue; }

        let mut jsx_names: Vec<String> = Vec::new();
        let mut other_names: Vec<String> = Vec::new();

        // Default import
        if jsx_idents.contains(&default_name) { jsx_names.push(default_name); }
        else { other_names.push(default_name); }

        // Named imports
        for item in spec.split(',') {
            let item = item.trim();
            if item.is_empty() || item.starts_with("type ") { continue; }
            let local = item.split(" as ").last().unwrap_or(item).trim();
            if jsx_idents.contains(local) { jsx_names.push(local.to_string()); }
            else { other_names.push(local.to_string()); }
        }

        if let Some((_, existing_jsx, existing_other)) = result.iter_mut().find(|(p, _, _)| p == &path) {
            for n in jsx_names { if !existing_jsx.contains(&n) { existing_jsx.push(n); } }
            for n in other_names { if !existing_other.contains(&n) { existing_other.push(n); } }
        } else {
            result.push((path, jsx_names, other_names));
        }
    }

    result
}

/// Resolve a relative path from a source file to a target file.
fn resolve_relative_file(from_file: &Path, relative: &str) -> Option<PathBuf> {
    let dir = from_file.parent()?;
    let base = dir.join(relative);
    for ext in &[".tsx", ".ts", ".jsx", ".js"] {
        let c = base.with_extension(&ext[1..]);
        if c.exists() { return Some(c); }
    }
    for idx in &["index.tsx", "index.ts", "index.jsx", "index.js"] {
        let c = base.join(idx);
        if c.exists() { return Some(c); }
    }
    if base.exists() { return Some(base); }
    None
}

/// Check if any test files (or their imports) need React.
/// Scans file contents for react-related imports or harness hooks.
pub fn needs_react(test_files: &[PathBuf]) -> bool {
    for file in test_files {
        if let Ok(content) = std::fs::read_to_string(file) {
            if content.contains("renderHook")
                || content.contains("render(")
                || content.contains("from 'react")
                || content.contains("from \"react")
                || content.contains("require('react")
                || content.contains("require(\"react")
                || content.contains("waitFor")
                || content.contains("useEffect")
                || content.contains("useState")
            {
                return true;
            }
        }
    }
    false
}

/// Compute a relative path from `from_dir` to `to_file`.

/// Generate a synthetic entry file that imports the harness and all test files.
/// `transforms` maps original test file paths to pre-transformed temp paths.
/// `shallow_auto_mocks` contains (module_path, jsx_names, other_names) from ht.shallow() scanning.
/// These are injected as ht.mock() calls that create component stubs.
pub fn generate_entry(
    test_files: &[PathBuf],
    _harness_path: Option<&Path>,
    mock_modules: &[String],
    cfg: &BundleConfig,
    transforms: &[(PathBuf, PathBuf)],
    project_root: Option<&Path>,
) -> String {
    generate_entry_with_shallow(test_files, _harness_path, mock_modules, cfg, transforms, project_root, &[])
}

pub fn generate_entry_with_shallow(
    test_files: &[PathBuf],
    _harness_path: Option<&Path>,
    mock_modules: &[String],
    cfg: &BundleConfig,
    transforms: &[(PathBuf, PathBuf)],
    project_root: Option<&Path>,
    shallow_auto_mocks: &[(String, Vec<String>, Vec<String>)],
) -> String {
    let mut entry = String::new();

    // Common globals expected by RN libraries
    entry.push_str("if (typeof globalThis.__DEV__ === 'undefined') globalThis.__DEV__ = false;\n");

    // Console is handled by the harness (print()-based). No duplicate interceptor needed.

    // Patch URLSearchParams: (1) constructor to accept plain objects, (2) add iterator.
    // Hermes's native URLSearchParams doesn't support object constructor or [Symbol.iterator],
    // causing "iterator method is not callable" with Object.fromEntries(urlSearchParams).
    entry.push_str(r#"(function() {
  var _Orig = globalThis.URLSearchParams;
  if (!_Orig) return;
  globalThis.URLSearchParams = function URLSearchParams(init) {
    if (init && typeof init === 'object' && !(init instanceof _Orig) && !Array.isArray(init) && typeof init !== 'string') {
      var pairs = [];
      var keys = Object.keys(init);
      for (var i = 0; i < keys.length; i++) {
        if (init[keys[i]] !== undefined) pairs.push([keys[i], String(init[keys[i]])]);
      }
      return new _Orig(pairs);
    }
    return new _Orig(init);
  };
  globalThis.URLSearchParams.prototype = _Orig.prototype;
  if (!_Orig.prototype[Symbol.iterator]) {
    _Orig.prototype[Symbol.iterator] = function() {
      var entries = [];
      this.forEach(function(v, k) { entries.push([k, v]); });
      var idx = 0;
      return { next: function() { return idx < entries.length ? { value: entries[idx++], done: false } : { done: true }; } };
    };
  }
  if (!_Orig.prototype.entries) {
    _Orig.prototype.entries = function() { return this[Symbol.iterator](); };
  }
})();
"#);

    // Register hermes-test as a mock so `import { test } from 'hermes-test'` resolves to __HT
    entry.push_str("globalThis.__HT_mocks = globalThis.__HT_mocks || {};\n");
    entry.push_str("globalThis.__HT_mocks['hermes-test'] = globalThis.__HT;\n");

    // Load shims for native modules. User shims (from config) override built-in defaults.
    // Built-in shims are embedded in the hermes-test binary.
    {
        let builtin_shims = vec![
            ("react-native", include_str!("../../../../packages/hermes-test/src/shims/react-native.js")),
        ];

        // Collect user-configured shim module names
        let user_shim_modules: Vec<&str> = cfg.shims.iter().map(|(k, _)| k.as_str()).collect();

        // Load built-in shims (skip if user provided a custom one)
        for (module_name, shim_source) in &builtin_shims {
            if !user_shim_modules.contains(module_name) {
                entry.push_str(&format!(
                    "globalThis.__HT_mocks['{}'] = (function() {{ {} }})();\n",
                    module_name,
                    // Wrap as: var m = {}; ... ; return m; → but our shims use module.exports
                    // So wrap as IIFE with module.exports shim
                    format!("var module = {{ exports: {{}} }}; {}; return module.exports;", shim_source)
                ));
            }
        }

        // Load user-configured shims via require (esbuild will bundle them)
        for (module_name, shim_path) in &cfg.shims {
            entry.push_str(&format!(
                "globalThis.__HT_mocks['{}'] = require('{}');\n",
                module_name, shim_path
            ));
        }
    }

    // Pre-register mock module placeholders BEFORE anything loads.
    // Each placeholder is a live Proxy that checks __HT_file_mocks on every property
    // access. This is critical: shared modules (e.g. keyValueStorage) cache the result
    // of __require("externalized-pkg") at module init time. If the placeholder is a
    // plain {}, per-file mocks registered later can't intercept property access.
    // By making it a Proxy, property access is always live — the Proxy checks
    // __currentTestFile's mock at access time, not at init time.
    if !mock_modules.is_empty() {
        entry.push_str("globalThis.__HT_mocks = globalThis.__HT_mocks || {};\n");
        for path in mock_modules {
            entry.push_str(&format!(
                r#"globalThis.__HT_mocks['{path}'] = globalThis.__HT_mocks['{path}'] || (typeof Proxy !== 'undefined' ? new Proxy({{}}, {{
  get: function(t, p) {{
    if (p === '__esModule') return true;
    if (typeof p === 'symbol') return void 0;
    var fm = globalThis.__HT_file_mocks;
    var f = globalThis.__currentTestFile;
    var m = fm && f && fm[f] && fm[f]['{path}'];
    if (m && p in m) return m[p];
    return t[p];
  }},
  set: function(t, p, v) {{ t[p] = v; return true; }}
}}) : {{}});
"#,
            ));
        }
    }

    // Only bootstrap React if test files actually need it (saves ~40ms for pure tests)
    if needs_react(test_files) {
        entry.push_str(
            r#"try {
  globalThis.__HT_React = require('react');
  // IS_REACT_ACT_ENVIRONMENT managed by act() in hooks.ts
} catch(e) {}
try {
  var __htRec = require('react-reconciler');
  globalThis.__HT_Reconciler = typeof __htRec === 'function' ? __htRec : (__htRec.default || __htRec);
  var __htRecC = require('react-reconciler/constants');
  globalThis.__HT_ReconcilerConstants = __htRecC.__esModule ? __htRecC : (__htRecC.default || __htRecC);
} catch(e) {}
"#,
        );
    }

    // Import test files — tag each with its filename so the harness can track source file
    // Use pre-transformed temp files for test files with aliased mocks.
    for file in test_files {
        let actual_file = transforms.iter()
            .find(|(orig, _)| orig == file)
            .map(|(_, temp)| temp.clone())
            .unwrap_or_else(|| file.clone());
        let path = actual_file.to_string_lossy();
        let require_path = if path.starts_with('/') || path.starts_with("./") {
            path.to_string()
        } else {
            format!("./{path}")
        };
        // Use relative path from project root as unique file ID to avoid basename collisions
        let file_id = project_root
            .and_then(|root| file.strip_prefix(root).ok())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| file.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string()));
        // __currentTestFilePath has the full require path for snapshot file resolution
        entry.push_str(&format!(
            "if (globalThis.__HT && globalThis.__HT.resetMockModulePatches) globalThis.__HT.resetMockModulePatches();\nglobalThis.__currentTestFile = '{file_id}';\nglobalThis.__currentTestFilePath = '{require_path}';\n",
        ));

        // Store ALL component imports on global so error hints can resolve
        // function names to module paths (e.g. "formatLocalePrice" → "@scope/utils/string").
        {
            let has_shallow = std::fs::read_to_string(file).map_or(false, |c| c.contains("ht.shallow("));
            if has_shallow {
                if let Some(ref re_sh) = regex::Regex::new(r#"ht\.shallow\(\s*['"]([^'"]+)['"]\s*\)"#).ok() {
                    if let Some(cap) = re_sh.captures(&std::fs::read_to_string(file).unwrap_or_default()) {
                        if let Some(comp_file) = resolve_relative_file(file, &cap[1]) {
                            if let Ok(comp_src) = std::fs::read_to_string(&comp_file) {
                                let re_def = regex::Regex::new(r#"import\s+(\w+)\s+from\s+['"]([^'"]+)['"]"#).unwrap();
                                let re_named = regex::Regex::new(r#"import\s+\{([^}]+)\}\s+from\s+['"]([^'"]+)['"]"#).unwrap();
                                entry.push_str("globalThis.__HT_shallow_imports = globalThis.__HT_shallow_imports || {};\n");
                                for cap in re_def.captures_iter(&comp_src) {
                                    entry.push_str(&format!(
                                        "globalThis.__HT_shallow_imports['{}'] = '{}';\n", &cap[1], &cap[2],
                                    ));
                                }
                                for cap in re_named.captures_iter(&comp_src) {
                                    let path = &cap[2];
                                    for item in cap[1].split(',') {
                                        let item = item.trim();
                                        if item.is_empty() || item.starts_with("type ") { continue; }
                                        let local = item.split(" as ").last().unwrap_or(item).trim();
                                        entry.push_str(&format!(
                                            "globalThis.__HT_shallow_imports['{local}'] = '{path}';\n",
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Inject auto-mock ht.mock() calls for ht.shallow() discovered imports.
        // JSX components get Proxy stubs that support dot-notation (e.g. Animated.View).
        // Non-JSX exports get generic function stubs to prevent barrel-file crash fallthrough.
        for (mod_path, jsx_names, other_names) in shallow_auto_mocks {
            let mut exports = String::new();
            // JSX component stubs: Proxy that returns sub-component stubs for dot access
            for name in jsx_names {
                exports.push_str(&format!(
                    "r['{name}'] = typeof Proxy !== 'undefined' ? new Proxy(function(p) {{ return R.createElement('{name}', p); }}, {{ get: function(t, k) {{ if (typeof k === 'symbol') return t[k]; if (k === 'prototype' || k === 'name' || k === 'length' || k === 'caller' || k === 'arguments' || k === 'displayName' || k === 'contextTypes' || k === 'childContextTypes' || k === 'getDerivedStateFromProps' || k === 'getDerivedStateFromError' || k === 'contextType' || k === 'defaultProps' || k === 'propTypes' || k === '$$typeof' || k === '__emotion_real' || k === '__docgenInfo' || k === 'render') return k === 'displayName' ? '{name}' : t[k]; return function(p) {{ return R.createElement('{name}.' + k, p); }}; }} }}) : function(p) {{ return R.createElement('{name}', p); }};",
                ));
            }
            // Non-JSX stubs: hooks/utilities get chainable Proxy stubs
            // (supports patterns like FadeIn.duration(700).delay(100))
            for name in other_names {
                exports.push_str(&format!(
                    "r['{name}'] = typeof Proxy !== 'undefined' ? new Proxy(function() {{ return {{}}; }}, {{ get: function(t, k) {{ if (typeof k === 'symbol') return t[k]; var self = r['{name}']; return function() {{ return self; }}; }} }}) : function() {{ return {{}}; }}; ",
                ));
            }
            // Default export = first JSX name (for default imports)
            if let Some(first) = jsx_names.first() {
                exports.push_str(&format!("r['default'] = r['{}']; ", first));
            }
            entry.push_str(&format!(
                "ht.mock('{mod_path}', function() {{ var R = globalThis.__HT_React; var r = {{}}; {exports}return r; }});\n",
            ));
        }

        entry.push_str(&format!(
            "try {{ require('{require_path}'); }} catch(e) {{ if (globalThis.__HT) globalThis.__HT.registerCrash('{file_id}', String(e && e.stack || e && e.message || e)); }}\n",
        ));
    }

    // Run all registered tests and stash results on a global
    entry.push_str(
        r#"
var __results = globalThis.__HT.runTests();
globalThis.__HT_results = JSON.stringify({
  tests: __results,
  passed: __results.filter(function(t) { return t.status === 'pass'; }).length,
  failed: __results.filter(function(t) { return t.status === 'fail'; }).length,
  skipped: __results.filter(function(t) { return t.status === 'skip'; }).length,
  total: __results.length
});
"#,
    );

    entry
}

/// Minimal entry for a group: just test file requires (no setup, no runner).
fn generate_group_entry(test_files: &[PathBuf], mock_modules: &[String], project_root: Option<&Path>, shallow_auto_mocks: &[(String, Vec<String>, Vec<String>)]) -> String {
    let mut entry = String::new();

    // Reset console log collector for this group
    entry.push_str("globalThis.__HT_logs = [];\n");

    // Re-register mock placeholders as live Proxies (same as generate_entry)
    if !mock_modules.is_empty() {
        for path in mock_modules {
            entry.push_str(&format!(
                r#"globalThis.__HT_mocks['{path}'] = globalThis.__HT_mocks['{path}'] || (typeof Proxy !== 'undefined' ? new Proxy({{}}, {{
  get: function(t, p) {{
    if (p === '__esModule') return true;
    if (typeof p === 'symbol') return void 0;
    var fm = globalThis.__HT_file_mocks;
    var f = globalThis.__currentTestFile;
    var m = fm && f && fm[f] && fm[f]['{path}'];
    if (m && p in m) return m[p];
    return t[p];
  }},
  set: function(t, p, v) {{ t[p] = v; return true; }}
}}) : {{}});
"#,
            ));
        }
    }

    for file in test_files {
        let path = file.to_string_lossy();
        let require_path = if path.starts_with('/') || path.starts_with("./") {
            path.to_string()
        } else {
            format!("./{path}")
        };
        let file_id = project_root
            .and_then(|root| file.strip_prefix(root).ok())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| file.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string()));

        entry.push_str(&format!(
            "if (globalThis.__HT && globalThis.__HT.resetMockModulePatches) globalThis.__HT.resetMockModulePatches();\nglobalThis.__currentTestFile = '{}';\n",
            file_id
        ));

        // Inject auto-mock ht.mock() calls for ht.shallow() discovered imports
        for (mod_path, jsx_names, other_names) in shallow_auto_mocks {
            let mut exports = String::new();
            for name in jsx_names {
                exports.push_str(&format!(
                    "r['{name}'] = typeof Proxy !== 'undefined' ? new Proxy(function(p) {{ return R.createElement('{name}', p); }}, {{ get: function(t, k) {{ if (typeof k === 'symbol') return t[k]; if (k === 'prototype' || k === 'name' || k === 'length' || k === 'caller' || k === 'arguments' || k === 'displayName' || k === 'contextTypes' || k === 'childContextTypes' || k === 'getDerivedStateFromProps' || k === 'getDerivedStateFromError' || k === 'contextType' || k === 'defaultProps' || k === 'propTypes' || k === '$$typeof' || k === '__emotion_real' || k === '__docgenInfo' || k === 'render') return k === 'displayName' ? '{name}' : t[k]; return function(p) {{ return R.createElement('{name}.' + k, p); }}; }} }}) : function(p) {{ return R.createElement('{name}', p); }};",
                ));
            }
            for name in other_names {
                exports.push_str(&format!(
                    "r['{name}'] = typeof Proxy !== 'undefined' ? new Proxy(function() {{ return {{}}; }}, {{ get: function(t, k) {{ if (typeof k === 'symbol') return t[k]; var self = r['{name}']; return function() {{ return self; }}; }} }}) : function() {{ return {{}}; }}; ",
                ));
            }
            if let Some(first) = jsx_names.first() {
                exports.push_str(&format!("r['default'] = r['{}']; ", first));
            }
            entry.push_str(&format!(
                "ht.mock('{mod_path}', function() {{ var R = globalThis.__HT_React; var r = {{}}; {exports}return r; }});\n",
            ));
        }

        entry.push_str(&format!(
            "try {{ require('{}'); }} catch(e) {{ if (globalThis.__HT) globalThis.__HT.registerCrash('{}', String(e && e.stack || e && e.message || e)); }}\n",
            require_path, file_id
        ));
    }

    entry
}

/// Public wrapper for generate_group_entry (used by watch mode and split mode).
pub fn generate_group_entry_pub(test_files: &[PathBuf], mock_modules: &[String], project_root: Option<&Path>, shallow_auto_mocks: &[(String, Vec<String>, Vec<String>)]) -> String {
    generate_group_entry(test_files, mock_modules, project_root, shallow_auto_mocks)
}

/// Compute a cache key from source file mtimes + config + test file list.
/// Public alias for single-bundle caching in main.rs.
pub fn compute_single_bundle_cache_key(
    test_files: &[PathBuf],
    project_root: &Path,
    mock_modules: &[String],
    cfg: &BundleConfig,
) -> String {
    compute_bundle_cache_key(test_files, project_root, mock_modules, cfg)
}

pub fn compute_bundle_cache_key(
    test_files: &[PathBuf],
    project_root: &Path,
    mock_modules: &[String],
    cfg: &BundleConfig,
) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    // Hash test file paths and mtimes
    for f in test_files {
        f.to_string_lossy().hash(&mut hasher);
        if let Ok(meta) = std::fs::metadata(f) {
            if let Ok(mtime) = meta.modified() { mtime.hash(&mut hasher); }
        }
    }

    // Hash all source files under project_root/src (mtimes only for speed)
    let src_dir = project_root.join("src");
    if src_dir.is_dir() {
        let mut src_files = Vec::new();
        collect_source_mtimes(&src_dir, &mut src_files);
        src_files.sort();
        for (path, mtime) in &src_files {
            path.hash(&mut hasher);
            mtime.hash(&mut hasher);
        }
    }

    // Hash config mtime
    let config_path = project_root.join("hermes-test.config.json");
    if let Ok(meta) = std::fs::metadata(&config_path) {
        if let Ok(mtime) = meta.modified() { mtime.hash(&mut hasher); }
    }

    // Hash mock modules + externals
    for m in mock_modules { m.hash(&mut hasher); }
    cfg.externals.len().hash(&mut hasher);
    cfg.split.hash(&mut hasher);

    format!("{:016x}", hasher.finish())
}

pub fn collect_source_mtimes(dir: &Path, out: &mut Vec<(String, u64)>) {
    let entries = match std::fs::read_dir(dir) { Ok(e) => e, Err(_) => return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let n = name.to_string_lossy();
        if path.is_dir() {
            if n == "node_modules" { continue; }
            collect_source_mtimes(&path, out);
        } else if n.ends_with(".ts") || n.ends_with(".tsx") || n.ends_with(".js") {
            if let Ok(meta) = std::fs::metadata(&path) {
                if let Ok(mtime) = meta.modified() {
                    let secs = mtime.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                    out.push((path.to_string_lossy().to_string(), secs));
                }
            }
        }
    }
}
