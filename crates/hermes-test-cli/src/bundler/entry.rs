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
            // Custom pattern: match suffix (e.g. ".hermes.test.ts")
            if name_str.ends_with(pat) {
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

/// Scan test files for mockModule() and jest.mock() calls and return the module paths.
/// These modules will be externalized in esbuild so that useMock can intercept them.
pub fn find_mock_modules(test_files: &[PathBuf]) -> Vec<String> {
    let mut mocks = Vec::new();
    let re_single = regex::Regex::new(r#"mockModule\(\s*['"]([^'"]+)['"]\s*,"#).ok();
    let re_double = regex::Regex::new(r#"mockModule\(\s*"([^"]+)"\s*,"#).ok();

    for file in test_files {
        if let Ok(content) = std::fs::read_to_string(file) {
            if let Some(ref re) = re_single {
                for cap in re.captures_iter(&content) {
                    let path = cap[1].to_string();
                    if !mocks.contains(&path) {
                        mocks.push(path);
                    }
                }
            }
            if let Some(ref re) = re_double {
                for cap in re.captures_iter(&content) {
                    let path = cap[1].to_string();
                    if !mocks.contains(&path) {
                        mocks.push(path);
                    }
                }
            }
        }
    }
    mocks
}

/// Check if any test files (or their imports) need React.
/// Scans file contents for react-related imports or harness hooks.
pub fn needs_react(test_files: &[PathBuf]) -> bool {
    for file in test_files {
        if let Ok(content) = std::fs::read_to_string(file) {
            if content.contains("renderHook")
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
pub fn generate_entry(
    test_files: &[PathBuf],
    _harness_path: Option<&Path>,
    mock_modules: &[String],
    cfg: &BundleConfig,
    transforms: &[(PathBuf, PathBuf)],
    project_root: Option<&Path>,
) -> String {
    let mut entry = String::new();

    // Common globals expected by RN libraries
    entry.push_str("if (typeof globalThis.__DEV__ === 'undefined') globalThis.__DEV__ = false;\n");

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
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
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
        entry.push_str(&format!(
            "if (globalThis.__HT && globalThis.__HT.resetMockModulePatches) globalThis.__HT.resetMockModulePatches();\nglobalThis.__currentTestFile = '{}';\ntry {{ require('{}'); }} catch(e) {{ if (globalThis.__HT) globalThis.__HT.registerCrash('{}', String(e && e.message || e)); }}\n",
            file_id, require_path, file_id
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
fn generate_group_entry(test_files: &[PathBuf], mock_modules: &[String], project_root: Option<&Path>) -> String {
    let mut entry = String::new();

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
        // Use relative path from project root as unique file ID to avoid basename collisions
        let file_id = project_root
            .and_then(|root| file.strip_prefix(root).ok())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| file.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string()));
        entry.push_str(&format!(
            "if (globalThis.__HT && globalThis.__HT.resetMockModulePatches) globalThis.__HT.resetMockModulePatches();\nglobalThis.__currentTestFile = '{}';\ntry {{ require('{}'); }} catch(e) {{ if (globalThis.__HT) globalThis.__HT.registerCrash('{}', String(e && e.message || e)); }}\n",
            file_id, require_path, file_id
        ));
    }

    entry
}

/// Public wrapper for generate_group_entry (used by watch mode in main.rs).
pub fn generate_group_entry_pub(test_files: &[PathBuf], mock_modules: &[String], project_root: Option<&Path>) -> String {
    generate_group_entry(test_files, mock_modules, project_root)
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
