use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// SWC class transform was evaluated but rejected:
// - Requires 3 scoped thread-locals (GLOBALS, HANDLER, HELPERS)
// - inject_helpers() emits require('@swc/helpers') calls incompatible with Hermes
// - Full SWC codegen re-emits the entire bundle (changes whitespace, breaks other patches)
// Our regex-based fix_all_class_extends() handles all known patterns at <1ms with zero deps.

#[derive(Clone, Copy, PartialEq)]
pub enum Bundler {
    Esbuild,
    Metro,
}

/// Bundle an entry file. Tries esbuild first (fast), falls back to Metro.
pub fn bundle_auto(
    entry_file: &Path,
    project_root: &Path,
    external_modules: &[String],
) -> Result<String, String> {
    if let Ok(path) = find_esbuild(project_root) {
        return bundle_esbuild(entry_file, &path, external_modules);
    }
    bundle_metro(entry_file, project_root)
}

pub fn bundle_with(
    bundler: Bundler,
    entry_file: &Path,
    project_root: &Path,
    external_modules: &[String],
) -> Result<String, String> {
    match bundler {
        Bundler::Esbuild => {
            let path = find_esbuild(project_root)
                .map_err(|_| "esbuild not found. Install it: bun add -d esbuild".to_string())?;
            bundle_esbuild(entry_file, &path, external_modules)
        }
        Bundler::Metro => bundle_metro(entry_file, project_root),
    }
}

fn find_esbuild(project_root: &Path) -> Result<PathBuf, ()> {
    let local = project_root.join("node_modules/.bin/esbuild");
    if local.exists() {
        return Ok(local);
    }
    let mut dir = project_root.parent();
    while let Some(d) = dir {
        let candidate = d.join("node_modules/.bin/esbuild");
        if candidate.exists() {
            return Ok(candidate);
        }
        dir = d.parent();
    }
    if Command::new("esbuild").arg("--version").output().is_ok() {
        return Ok(PathBuf::from("esbuild"));
    }
    Err(())
}

/// Read path aliases from tsconfig.json "compilerOptions.paths" and hermes-test.config.json.
/// Returns Vec<(alias, target_path)> for esbuild --alias flags.
#[derive(Clone)]
pub struct BundleConfig {
    pub aliases: Vec<(String, String)>,
    externals: Vec<String>,
    shims: Vec<(String, String)>, // (module_name, file_path) — replacement shims
    wrapper_shims: Vec<(String, String)>, // (module_name, builtin_name) — hermes-test/shims/*
    root: Option<PathBuf>,
    pub split: bool,
    pub test_match: Option<String>, // e.g. ".hermes.test.ts" — only discover matching files
}

/// Extract a string value from JSON: "key": "value"
fn json_string_value(json: &str, key: &str) -> Option<String> {
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
fn json_string_array(json: &str, key: &str) -> Option<Vec<String>> {
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
fn json_object_entries(json: &str, key: &str) -> Option<Vec<(String, String)>> {
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
fn read_tsconfig_paths(base_dir: &Path, tsconfig_path: &Path, aliases: &mut Vec<(String, String)>) {
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

    config
}


fn bundle_esbuild(
    entry_file: &Path,
    esbuild_path: &Path,
    external_modules: &[String],
) -> Result<String, String> {
    let project_root = entry_file.parent().unwrap_or(Path::new("."));
    let cfg = read_config(project_root);
    bundle_esbuild_with_config(entry_file, esbuild_path, external_modules, &cfg, false)
}

/// Bundle with a custom config (e.g. shadow-wrapped aliases).
pub fn bundle_auto_with_config(
    entry_file: &Path,
    project_root: &Path,
    external_modules: &[String],
    cfg: &BundleConfig,
) -> Result<String, String> {
    if let Ok(path) = find_esbuild(project_root) {
        return bundle_esbuild_with_config(entry_file, &path, external_modules, cfg, false);
    }
    bundle_metro(entry_file, project_root)
}

/// Bundle with esbuild using provided config (avoids re-reading from disk).
/// When `packages_external` is true, adds --packages=external to externalize all node_modules.
fn bundle_esbuild_with_config(
    entry_file: &Path,
    esbuild_path: &Path,
    external_modules: &[String],
    cfg: &BundleConfig,
    packages_external: bool,
) -> Result<String, String> {
    let mut cmd = Command::new(esbuild_path);
    cmd.arg(entry_file)
        .arg("--bundle")
        .arg("--format=iife")
        .arg("--target=es2020")
        // No --minify — our Hermes compat patches match unminified esbuild output patterns.
        .arg("--supported:async-await=false")
        .arg("--define:process.env.NODE_ENV=\"test\"")
        .arg("--define:process.env.JEST_WORKER_ID=\"1\"")
        .arg("--define:global=globalThis")
        .arg("--loader:.js=jsx")
        .arg("--loader:.png=empty")
        .arg("--loader:.jpg=empty")
        .arg("--loader:.gif=empty")
        .arg("--loader:.svg=empty")
        .arg("--external:console");

    if packages_external {
        cmd.arg("--packages=external");
    }

    // Monorepo: add node_modules paths for resolution.
    {
        let mut node_paths = Vec::new();
        let project_nm = entry_file.parent().unwrap_or(Path::new(".")).join("node_modules");
        if project_nm.is_dir() {
            node_paths.push(project_nm.to_string_lossy().to_string());
        }
        if let Some(ref root) = cfg.root {
            let root_nm = root.join("node_modules");
            if root_nm.is_dir() {
                node_paths.push(root_nm.to_string_lossy().to_string());
            }
        }
        if !node_paths.is_empty() {
            cmd.env("NODE_PATH", node_paths.join(":"));
        }
    }

    // Path aliases from tsconfig (resolved by config).
    // Skip aliases when:
    // 1. The package is in the externals list (native modules)
    // 2. Any mockModule path is a sub-path of this alias — esbuild aliases run BEFORE
    //    external checks, so mocked imports would get inlined instead of intercepted.
    //    Skipping the alias means ALL sub-paths go through __require → mock shim.
    for (alias, target) in &cfg.aliases {
        let is_externalized = cfg.externals.iter().any(|e| {
            let e_base = e.trim_end_matches('*').trim_end_matches('/');
            alias == e_base || alias.starts_with(&format!("{e_base}/"))
                || (e.ends_with('*') && alias.starts_with(e_base))
        });
        let has_mocked_subpath = external_modules.iter().any(|m| {
            m == alias || m.starts_with(&format!("{alias}/"))
        });
        if !is_externalized && !has_mocked_subpath {
            cmd.arg(format!("--alias:{alias}={target}"));
        }
    }

    // Default externals: hermes-test (thin re-export from __HT), React Native (Flow syntax)
    cmd.arg("--external:hermes-test");
    cmd.arg("--external:@marcuzgabriel/hermes-test");
    // Alias hermes-test/store to the actual file so it gets BUNDLED (not externalized).
    // esbuild aliases run before external checks, so this resolves before the external match.
    {
        let store_paths = [
            entry_file.parent().unwrap_or(std::path::Path::new(".")).join("node_modules/hermes-test/src/store.ts"),
            entry_file.parent().unwrap_or(std::path::Path::new(".")).join("node_modules/@marcuzgabriel/hermes-test/src/store.ts"),
        ];
        for sp in &store_paths {
            if sp.exists() {
                cmd.arg(format!("--alias:hermes-test/store={}", sp.to_string_lossy()));
                cmd.arg(format!("--alias:@marcuzgabriel/hermes-test/store={}", sp.to_string_lossy()));
                break;
            }
        }
        // Also check project root node_modules
        if let Some(ref root) = cfg.root {
            let root_store = root.join("node_modules/hermes-test/src/store.ts");
            if root_store.exists() {
                cmd.arg(format!("--alias:hermes-test/store={}", root_store.to_string_lossy()));
            }
        }
    }
    for ext in &["react-native", "react-native/*", "@react-native/*"] {
        cmd.arg(format!("--external:{ext}"));
    }

    // Config externals — for wildcard patterns like `pkg/*`, also externalize `pkg` itself
    for ext in &cfg.externals {
        cmd.arg(format!("--external:{ext}"));
        if ext.ends_with("/*") {
            // Also externalize bare import: `@foo/bar/*` → also `@foo/bar`
            let base = &ext[..ext.len() - 2];
            cmd.arg(format!("--external:{base}"));
        } else if !ext.ends_with('*') {
            cmd.arg(format!("--external:{ext}/*"));
        }
    }

    // Mock module externals
    for ext in external_modules {
        cmd.arg(format!("--external:{ext}"));
    }

    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run esbuild: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("esbuild failed: {stderr}"));
    }

    let code =
        String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 from esbuild: {e}"))?;

    let mut code = code.to_string();

    // Patch esbuild runtime helpers for Hermes compat
    code = patch_esbuild_for_hermes(&code);

    // Inject __require shim when there are external modules that need runtime resolution
    let has_externals = !external_modules.is_empty() || !cfg.externals.is_empty()
        || packages_external || code.contains("Dynamic require of");
    if has_externals {
        code = inject_mock_require_shim(&code);
    }

    // Hoist mockModule() calls before require() calls so aliased shadow-wrapper mocks
    // are registered before the module initializers run (captures dispatch, getState, etc.)
    let hoisted = hoist_mock_modules(&code);
    if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
        let _ = std::fs::write("/tmp/ht_bundle_hoisted.js", &hoisted);
        let _ = std::fs::write("/tmp/ht_bundle_original.js", &code);
    }
    code = hoisted;

    Ok(code)
}

/// Compile JS to Hermes bytecode in-process via linked Hermes compiler.
/// No subprocess, no temp files. Returns None if bundle has no classes.
pub fn compile_to_bytecode(code: &str, _context_path: &Path) -> Option<Vec<u8>> {
    // Only needed for bundles with class syntax
    if !code.contains("= class ") && !code.contains("= class{") {
        return None;
    }

    match crate::hermes::compile_bytecode(code, "bundle.js") {
        Ok(bytecode) => Some(bytecode),
        Err(e) => {
            eprintln!("WARNING: hermesc bytecode compilation failed: {e}");
            None
        },
    }
}

/// Compile to bytecode with disk cache. Returns (bytecode, cache_hit).
/// Cache key: hash of JS source. Cache dir: project_root/.hermes-test-cache/
pub fn compile_to_bytecode_cached(
    code: &str,
    project_root: &Path,
    prefix: &str,
) -> Option<(Vec<u8>, bool)> {
    if !code.contains("= class ") && !code.contains("= class{") {
        return None;
    }

    let hash = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        code.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };

    let cache_dir = project_root.join(".hermes-test-cache");
    let cache_path = cache_dir.join(format!("{prefix}-{hash}.hbc"));

    // Try cache hit
    if let Ok(bytecode) = std::fs::read(&cache_path) {
        return Some((bytecode, true));
    }

    // Cache miss — compile and save
    match crate::hermes::compile_bytecode(code, "bundle.js") {
        Ok(bytecode) => {
            let _ = std::fs::create_dir_all(&cache_dir);
            // Clean old cache files for this prefix
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name.starts_with(prefix) && name.ends_with(".hbc") && name != cache_path.file_name().unwrap().to_string_lossy().as_ref() {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
            let _ = std::fs::write(&cache_path, &bytecode);
            Some((bytecode, false))
        }
        Err(e) => {
            eprintln!("WARNING: hermesc bytecode compilation failed: {e}");
            None
        }
    }
}


/// Patch esbuild's __require to route externalized modules through __HT_mocks.
/// Simple approach: replace the "throw" line inside __require with a registry lookup.
fn inject_mock_require_shim(code: &str) -> String {
    // esbuild's __require contains this exact throw statement for unsupported externals:
    //   throw Error('Dynamic require of "' + x + '" is not supported');
    // Replace it with a __HT_mocks lookup.
    // Return a Proxy for externalized modules so that properties added later
    // via mockModule() are visible even though import destructuring already ran.
    // This solves the ESM import hoisting problem: `import {X} from 'mod'` runs
    // before `mockModule('mod', () => ({X: ...}))` but the Proxy delegates reads
    // to the live mock registry entry.
    // esbuild may use different variable names (x, x2, etc.) depending on version.
    let throw_re = regex::Regex::new(
        r#"throw Error\('Dynamic require of "' \+ (\w+) \+ '" is not supported'\)"#
    ).unwrap();

    if !throw_re.is_match(&code) {
        eprintln!("WARNING: __require shim pattern not found — externalized modules may not work");
        return code.to_string();
    }
    // Hoist __noop outside __require so it's created once, not per call.
    // Proxy-based noop: any property access, function call, or `new` returns __HT_noop,
    // enabling infinite chains like `noop.foo().bar.baz()` without throwing.
    let code = code.replacen(
        "throw Error('Dynamic require",
        "var __HT_noop_fn = function(){}; var __HT_noop = typeof Proxy !== 'undefined' ? new Proxy(__HT_noop_fn, { get: function(t,p) { if (p === Symbol.toPrimitive) return function() { return ''; }; if (p === 'valueOf' || p === 'toJSON') return function() { return 0; }; if (p === 'toString' || p === 'toLocaleString') return function() { return ''; }; if (p === Symbol.iterator) return function() { return { next: function() { return { done: true }; } }; }; if (p === 'length' || p === 'size') return 0; if (p === 'then' || p === '$$typeof' || p === '_isAMomentObject' || p === '__esModule') return undefined; if (p === 'constructor') return Object; if (typeof p === 'symbol') return undefined; return __HT_noop; }, apply: function() { return __HT_noop; }, construct: function() { return {}; }, ownKeys: function(t) { return Object.getOwnPropertyNames(t); }, getOwnPropertyDescriptor: function(t, p) { return Object.getOwnPropertyDescriptor(t, p) || { configurable: true, enumerable: false, writable: true, value: undefined }; } }) : function() {}; throw Error('Dynamic require",
        1,
    );

    throw_re.replace(&code, |caps: &regex::Captures| {
        let v = &caps[1];
        // Proxy with get trap that checks per-file mocks first, then global mocks.
        // Per-file mocks: __HT_file_mocks[__currentTestFile][path] — set by mockModule()
        // Global mocks: __HT_mocks[path] — fallback for backward compat
        // The Proxy's get trap checks per-file mocks first (for mock isolation),
        // then global mocks. For aliased mocks, __require receives the resolved path
        // (e.g. "/abs/src/hooks") but mockModule registers under the original path
        // (e.g. "@scope/pkg/hooks"). __HT_mock_aliases maps resolved → original.
        format!(
            r#"{{ var __r = globalThis.__HT_mocks || (globalThis.__HT_mocks = {{}}); var __k = {v}.replace(/^\.\//, ''); var __t = __r[{v}] || __r[__k] || __r['./' + __k] || {{}}; return typeof Proxy !== 'undefined' ? new Proxy(__t, {{ get: function(t,p) {{ if (p === Symbol.toPrimitive || p === 'then' || p === '$$typeof') return undefined; if (p === '__esModule') return true; var __fm = globalThis.__HT_file_mocks; var __cf = globalThis.__currentTestFile; var __pf = __fm && __cf && __fm[__cf]; var __al = globalThis.__HT_mock_aliases || {{}}; var __orig = __al[{v}] || __al[__k]; var __m = (__pf && (__pf[{v}] || __pf[__k] || __pf['./' + __k] || (__orig && __pf[__orig]))) || __r[{v}] || __r[__k] || __r['./' + __k]; if (p === 'default') {{ var __d = __m && __m['default']; return __d !== undefined ? __d : (__m || t); }} var val = __m ? __m[p] : t[p]; return val !== undefined ? val : __HT_noop; }}, apply: function() {{ return __HT_noop; }}, construct: function() {{ return {{}}; }}, ownKeys: function(t) {{ return Object.getOwnPropertyNames(t); }}, getOwnPropertyDescriptor: function(t, p) {{ return Object.getOwnPropertyDescriptor(t, p) || {{ configurable: true, enumerable: false, writable: true, value: undefined }}; }} }}) : __t }}"#,
        )
    }).to_string()
}

/// Hoist mockModule() calls before init_*() calls in esbuild's bundled output.
/// Hoist mockModule() calls before init_*() / require() calls so that when a module's
/// initializer runs (e.g. `const { dispatch, getState } = store`), the mock is already
/// registered in __HT_file_mocks and the shadow-wrapper Proxy returns the mock value.
fn hoist_mock_modules(code: &str) -> String {
    // Pattern: (0, import_hermes_test.mockModule)("path", () => ({ ... }));
    // or: (0, import_hermes_test2.mockModule)("path", () => ({ ... }));
    // We need to find these, extract them, and move them before init_*() calls.

    let mut result = String::with_capacity(code.len());
    let bytes = code.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // Process each __commonJS or __esm block that contains test files
    // Look for the function body pattern: `"filename.test.ts"(exports) {` or `"filename.test.ts"() {`
    while i < len {
        // Find test file function bodies inside __commonJS/__esm
        // Pattern: "something.test.ts"(exports) { or "something.test.ts"() {
        if let Some(pos) = code[i..].find(".test.ts\"(") {
            let abs_pos = i + pos;
            // Find the opening brace of this function body
            if let Some(brace_offset) = code[abs_pos..].find('{') {
                let body_start = abs_pos + brace_offset + 1;
                // Find the end of this function body by counting braces
                let body_end = find_matching_brace(code, body_start);
                if body_end > body_start {
                    // Copy everything up to body_start
                    result.push_str(&code[i..body_start]);

                    // Process this function body: extract mockModule calls and hoist them
                    let body = &code[body_start..body_end];
                    let hoisted = hoist_mocks_in_body(body);
                    if std::env::var("HT_DEBUG_BUNDLE").is_ok() && hoisted != body {
                        eprintln!("[HT_HOIST] Modified body at offset {abs_pos} (body len: {})", body.len());
                    }
                    result.push_str(&hoisted);

                    i = body_end;
                    continue;
                }
            }
            // Couldn't process, copy up to and past this match
            result.push_str(&code[i..abs_pos + pos + 10]);
            i = abs_pos + pos + 10;
        } else {
            // No more test file blocks
            result.push_str(&code[i..]);
            break;
        }
    }

    result
}

/// Find the position of the matching closing brace for an opening brace at `start`.
/// `start` should be the position right after the opening `{`.
/// Returns the position of the closing `}`.
fn find_matching_brace(code: &str, start: usize) -> usize {
    let bytes = code.as_bytes();
    let mut depth = 1;
    let mut j = start;
    while j < bytes.len() && depth > 0 {
        match bytes[j] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return j;
                }
            }
            b'"' | b'\'' | b'`' => {
                // Skip string literals
                let quote = bytes[j];
                j += 1;
                while j < bytes.len() {
                    if bytes[j] == b'\\' {
                        j += 1; // skip escaped char
                    } else if bytes[j] == quote {
                        break;
                    }
                    j += 1;
                }
            }
            b'/' => {
                // Skip comments
                if j + 1 < bytes.len() {
                    if bytes[j + 1] == b'/' {
                        // Line comment
                        while j < bytes.len() && bytes[j] != b'\n' {
                            j += 1;
                        }
                        continue;
                    } else if bytes[j + 1] == b'*' {
                        // Block comment
                        j += 2;
                        while j + 1 < bytes.len() {
                            if bytes[j] == b'*' && bytes[j + 1] == b'/' {
                                j += 1;
                                break;
                            }
                            j += 1;
                        }
                    }
                }
            }
            _ => {}
        }
        j += 1;
    }
    j
}

/// Extract a balanced parenthesized expression starting at `start` (position of opening paren).
/// Returns the position after the closing paren (including trailing semicolon/newline).
fn extract_call_end(code: &str, start: usize) -> usize {
    let bytes = code.as_bytes();
    let mut depth = 0;
    let mut j = start;
    while j < bytes.len() {
        match bytes[j] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    j += 1;
                    // Skip trailing semicolon and newline
                    while j < bytes.len() && (bytes[j] == b';' || bytes[j] == b'\n' || bytes[j] == b'\r' || bytes[j] == b' ') {
                        j += 1;
                    }
                    return j;
                }
            }
            b'"' | b'\'' | b'`' => {
                let quote = bytes[j];
                j += 1;
                while j < bytes.len() {
                    if bytes[j] == b'\\' {
                        j += 1;
                    } else if bytes[j] == quote {
                        break;
                    }
                    j += 1;
                }
            }
            _ => {}
        }
        j += 1;
    }
    j
}

/// Within a single function body, move init_*() calls for non-hermes modules to AFTER
/// the last mockModule() call. This ensures that:
/// 1. Variable declarations (mockDispatch, mockGetState etc.) execute before mockModule() factories
/// 2. mockModule() registers its mock values before modules initialize (init_*() runs)
/// 3. When a module's initializer captures values like `const { dispatch } = store`, the mock is live
///
/// Strategy: "push init_* calls down" rather than "pull mockModule calls up".
/// This preserves the relative order of variable declarations and mockModule calls.
fn hoist_mocks_in_body(body: &str) -> String {
    // Find all mockModule calls to determine if hoisting is needed
    let mock_pattern = ".mockModule)(";
    if !body.contains(mock_pattern) {
        if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
            eprintln!("[HOIST_BODY] no mockModule calls found in body (len={})", body.len());
        }
        return body.to_string();
    }
    if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
        eprintln!("[HOIST_BODY] found mockModule calls, body len={}", body.len());
    }

    // Find the last mockModule call's end position
    let mut last_mock_end = 0;
    let mut search_start = 0;
    while let Some(pos) = body[search_start..].find(mock_pattern) {
        let abs_pos = search_start + pos;
        let outer_call_start = abs_pos + mock_pattern.len() - 1;
        let outer_end = extract_call_end(body, outer_call_start);
        // Extend to include trailing semicolon and newline
        let mut end = outer_end;
        let bytes = body.as_bytes();
        if end < bytes.len() && bytes[end] == b';' { end += 1; }
        if end < bytes.len() && bytes[end] == b'\n' { end += 1; }
        if end > last_mock_end { last_mock_end = end; }
        search_start = outer_end;
    }

    if last_mock_end == 0 {
        if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
            eprintln!("[HOIST_BODY] last_mock_end=0, no mocks found");
        }
        return body.to_string();
    }

    if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
        eprintln!("[HOIST_BODY] last_mock_end={}", last_mock_end);
    }

    // Find init_*() calls that appear BEFORE last_mock_end and are not init_hermes*
    // These need to be moved to AFTER last_mock_end.
    // Note: Rust regex crate doesn't support lookahead, so we filter out hermes* manually.
    // Pattern: `      init_SomeName();\n` (with leading whitespace)
    let init_re = match regex::Regex::new(r"(?m)^([ \t]*)(init_\w+)\(\);?\n?") {
        Ok(re) => re,
        Err(_) => return body.to_string(),
    };

    // Collect init_* ranges that are before last_mock_end and are not hermes-test internals
    let mut init_ranges: Vec<(usize, usize, &str)> = Vec::new();
    for m in init_re.find_iter(body) {
        // Skip hermes-test internal inits like init_hermes_test
        let text = m.as_str().trim_start();
        if text.starts_with("init_hermes") { continue; }
        if m.start() < last_mock_end {
            init_ranges.push((m.start(), m.end(), m.as_str()));
        }
    }

    if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
        eprintln!("[HOIST_BODY] init_ranges count={}", init_ranges.len());
        for (s, e, t) in &init_ranges {
            eprintln!("[HOIST_BODY]   init at {}..{}: {:?}", s, e, t.trim());
        }
    }

    if init_ranges.is_empty() {
        return body.to_string();
    }

    // Rebuild body: copy everything, skipping init_* calls before last_mock_end,
    // then insert the collected init_* calls right after last_mock_end.
    let mut result = String::with_capacity(body.len() + 64);
    let mut pos = 0;
    let mut collected_inits = String::new();

    for &(start, end, text) in &init_ranges {
        result.push_str(&body[pos..start]);
        collected_inits.push_str(text);
        if !text.ends_with('\n') { collected_inits.push('\n'); }
        pos = end;
    }

    // Copy up to last_mock_end (might include some content after the last init_* we skipped)
    // We need to handle the case where last_mock_end > pos
    result.push_str(&body[pos..last_mock_end]);

    // Insert collected init_* calls after all mockModule calls
    result.push_str(&collected_inits);

    // Copy the rest
    result.push_str(&body[last_mock_end..]);

    result
}

/// Find the insertion point for hoisted mocks in a function body.
/// Mocks must go AFTER the hermes-test require (so mockModule is defined)
/// but BEFORE any init_*() calls (so mocks are registered before modules load).
fn find_mock_insert_point(body: &str) -> usize {
    // Strategy: find the end of the hermes-test require line, insert after it.
    // Patterns: `__require("hermes-test")` or `__require("@marcuzgabriel/hermes-test")`
    let hermes_patterns = [
        r#"__require("hermes-test")"#,
        r#"__require("@marcuzgabriel/hermes-test")"#,
    ];
    let mut after_require = 0;
    for pat in &hermes_patterns {
        if let Some(pos) = body.find(pat) {
            // Find end of this statement (next semicolon + newline)
            let rest = &body[pos..];
            if let Some(semi) = rest.find(";\n") {
                let candidate = pos + semi + 2; // after ";\n"
                if candidate > after_require {
                    after_require = candidate;
                }
            } else if let Some(nl) = rest.find('\n') {
                let candidate = pos + nl + 1;
                if candidate > after_require {
                    after_require = candidate;
                }
            }
        }
    }

    // If we found a hermes require, also skip past any init_hermes* calls
    if after_require > 0 {
        let init_re = regex::Regex::new(r"(?m)^[ \t]*init_hermes\w*\(\);?\n?").unwrap();
        let mut pos = after_require;
        while let Some(m) = init_re.find(&body[pos..]) {
            if m.start() == 0 || body[pos..pos + m.start()].trim().is_empty() {
                pos += m.end();
            } else {
                break;
            }
        }
        return pos;
    }

    // Fallback: find first non-hermes init_*() call
    let init_re = regex::Regex::new(r"(?m)^[ \t]*(init_\w+)\(\)").unwrap();
    for m in init_re.find_iter(body) {
        let matched = m.as_str().trim();
        if !matched.starts_with("init_hermes") {
            return m.start();
        }
    }

    0
}

/// Downlevel ALL `class Foo extends Expr { ... }` patterns to function-based constructors.
///
/// Hermes has two bugs with class-extends:
/// 1. TDZ bug: `class X extends Variable` crashes when Variable is a local/parameter
///    (e.g. zod v4: `class Definition extends Parent {}` where Parent = params?.Parent ?? Object)
/// 2. Native super bug: `super()` in `class X extends Array` discards the return value
///
/// Fix: replace every class-extends with Reflect.construct-based function.
/// Same pattern as Babel's `_wrapNativeSuper` and SWC's `_wrap_native_super`.
///
/// Handles three forms:
/// - Assignment with named class: `Name = class InternalName extends Expr {`
/// - Assignment with anonymous class: `var Name = class extends Expr {`
/// - Class declaration: `class Name extends Expr {` (inside function scope)
fn fix_all_class_extends(code: &str) -> String {
    // Collect all matches from three patterns, storing (start, end, var_name, internal_name, parent_expr)
    struct ClassMatch {
        start: usize,
        end: usize,       // end of the full class (including trailing ;)
        var_name: String,
        internal_name: String,
        parent_expr: String,
        body: String,      // everything between the opening { and closing }
    }

    let mut matches: Vec<ClassMatch> = Vec::new();

    // JS identifiers can contain $ — use [\w$] instead of \w
    // Pattern A: `Name = class InternalName extends Expr {`
    let re_a = regex::Regex::new(
        r"([\w$]+)\s*=\s*class\s+([\w$]+)\s+extends\s+([\w$][\w$.]*)\s*\{"
    ).unwrap();

    // Pattern B: `Name = class extends Expr {` (anonymous)
    let re_b = regex::Regex::new(
        r"([\w$]+)\s*=\s*class\s+extends\s+([\w$][\w$.]*)\s*\{"
    ).unwrap();

    // Pattern C: class declaration `class Name extends Expr {` (not preceded by `= `)
    let re_c = regex::Regex::new(
        r"(?:^|[\n;{}\s])class\s+([\w$]+)\s+extends\s+([\w$][\w$.]*)\s*\{"
    ).unwrap();

    // Helper: find matching close brace from position after opening {
    fn find_class_end(code: &str, body_start: usize) -> usize {
        let mut depth = 1;
        for (i, ch) in code[body_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return body_start + i + 1;
                    }
                }
                _ => {}
            }
        }
        code.len()
    }

    // Collect pattern A matches
    for cap in re_a.captures_iter(code) {
        let full = cap.get(0).unwrap();
        let body_start = full.end();
        let mut class_end = find_class_end(code, body_start);
        if class_end < code.len() && code.as_bytes()[class_end] == b';' {
            class_end += 1;
        }
        let body = code[body_start..class_end].to_string();
        matches.push(ClassMatch {
            start: full.start(),
            end: class_end,
            var_name: cap[1].to_string(),
            internal_name: cap[2].to_string(),
            parent_expr: cap[3].to_string(),
            body,

        });
    }

    // Collect pattern B matches (skip if overlaps with pattern A)
    for cap in re_b.captures_iter(code) {
        let full = cap.get(0).unwrap();
        if matches.iter().any(|m| m.start <= full.start() && full.start() < m.end) {
            continue; // Already matched by pattern A
        }
        let body_start = full.end();
        let mut class_end = find_class_end(code, body_start);
        if class_end < code.len() && code.as_bytes()[class_end] == b';' {
            class_end += 1;
        }
        let var_name = cap[1].to_string();
        let body = code[body_start..class_end].to_string();
        matches.push(ClassMatch {
            start: full.start(),
            end: class_end,
            var_name: var_name.clone(),
            internal_name: var_name,
            parent_expr: cap[2].to_string(),
            body,

        });
    }

    // Collect pattern C matches (class declarations, skip overlaps)
    for cap in re_c.captures_iter(code) {
        let full = cap.get(0).unwrap();
        // Adjust start to skip the leading whitespace/newline character
        let actual_start = code[full.start()..full.end()]
            .find("class")
            .map(|i| full.start() + i)
            .unwrap_or(full.start());
        if matches.iter().any(|m| m.start <= actual_start && actual_start < m.end) {
            continue;
        }
        // Also skip if preceded by `= ` (would be pattern A/B)
        if actual_start >= 2 && &code[actual_start - 2..actual_start] == "= " {
            continue;
        }
        let body_start = full.end();
        let mut class_end = find_class_end(code, body_start);
        if class_end < code.len() && code.as_bytes()[class_end] == b';' {
            class_end += 1;
        }
        let var_name = cap[1].to_string();
        let body = code[body_start..class_end].to_string();
        matches.push(ClassMatch {
            start: actual_start,
            end: class_end,
            var_name: var_name.clone(),
            internal_name: var_name,
            parent_expr: cap[2].to_string(),
            body,

        });
    }

    if matches.is_empty() {
        return code.to_string();
    }

    // Sort by start position descending so we can replace from end to start
    matches.sort_by(|a, b| b.start.cmp(&a.start));

    let mut result = code.to_string();

    for m in &matches {
        let var_name = &m.var_name;
        let parent = &m.parent_expr;
        let class_body = &m.body;

        // Parse class body for constructor and methods
        let mut methods = Vec::new();
        let mut constructor_body: Option<(String, String)> = None; // (params, body)
        let mut pos = 0;

        while pos < class_body.len() {
            let remaining = &class_body[pos..];

            // Skip whitespace and closing braces
            if remaining.starts_with('}') || remaining.starts_with('\n') || remaining.starts_with(' ') || remaining.starts_with(';') {
                pos += 1;
                continue;
            }

            // Constructor
            if remaining.starts_with("constructor") {
                let after_kw = &remaining[11..];
                if let Some(paren_start) = after_kw.find('(') {
                    let params_start = 11 + paren_start + 1;
                    // Find matching close paren
                    let mut pd = 1;
                    let mut params_end = pos + params_start;
                    for (i, ch) in class_body[pos + params_start..].char_indices() {
                        match ch {
                            '(' => pd += 1,
                            ')' => { pd -= 1; if pd == 0 { params_end = pos + params_start + i; break; } }
                            _ => {}
                        }
                    }
                    let params = class_body[pos + params_start..params_end].to_string();

                    // Find constructor body
                    if let Some(brace) = class_body[params_end..].find('{') {
                        let body_start = params_end + brace + 1;
                        let mut d = 1;
                        let mut body_end = body_start;
                        for (i, ch) in class_body[body_start..].char_indices() {
                            match ch {
                                '{' => d += 1,
                                '}' => { d -= 1; if d == 0 { body_end = body_start + i; break; } }
                                _ => {}
                            }
                        }
                        let body = class_body[body_start..body_end].to_string();
                        constructor_body = Some((params, body));
                        pos = body_end + 1;
                    } else {
                        pos += 1;
                    }
                } else {
                    pos += 1;
                }
                continue;
            }

            // Static or instance methods
            let is_static = remaining.starts_with("static ");
            let method_remaining = if is_static { &remaining[7..] } else { remaining };

            // Skip Symbol.species getter — we add our own for Array subclasses
            if is_static && method_remaining.starts_with("get [Symbol.species]") {
                if let Some(brace) = remaining.find('{') {
                    let mut d = 1;
                    let start = pos + brace + 1;
                    for (i, ch) in class_body[start..].char_indices() {
                        match ch {
                            '{' => d += 1,
                            '}' => { d -= 1; if d == 0 { pos = start + i + 1; break; } }
                            _ => {}
                        }
                    }
                } else {
                    pos += 1;
                }
                continue;
            }

            // Match: `methodName(` — use paren-matching for params (handles nested parens/braces)
            let method_start_re = regex::Regex::new(r"^([\w$]+)\s*\(").unwrap();
            if let Some(cap) = method_start_re.captures(method_remaining) {
                let name = cap.get(1).unwrap().as_str();
                // Skip JS keywords — these are statements, not method definitions
                if matches!(name, "if" | "for" | "while" | "switch" | "catch" | "with" | "do"
                    | "return" | "throw" | "try" | "new" | "delete" | "typeof" | "void"
                    | "function" | "var" | "let" | "const" | "class") {
                    pos += 1;
                    continue;
                }
                let paren_start = pos + (if is_static { 7 } else { 0 }) + cap.get(0).unwrap().end();
                // Find matching close paren with balanced matching
                let mut pd = 1;
                let mut paren_end = paren_start;
                for (i, ch) in class_body[paren_start..].char_indices() {
                    match ch {
                        '(' => pd += 1,
                        ')' => { pd -= 1; if pd == 0 { paren_end = paren_start + i; break; } }
                        _ => {}
                    }
                }
                let params = &class_body[paren_start..paren_end];
                // After `)`, expect whitespace then `{`
                let after_paren = &class_body[paren_end + 1..];
                let trimmed = after_paren.trim_start();
                if !trimmed.starts_with('{') {
                    pos += 1;
                    continue;
                }
                let brace_offset = paren_end + 1 + (after_paren.len() - trimmed.len()) + 1;
                let abs_body_start = brace_offset;

                let mut d = 1;
                let mut body_end = abs_body_start;
                for (i, ch) in class_body[abs_body_start..].char_indices() {
                    match ch {
                        '{' => d += 1,
                        '}' => { d -= 1; if d == 0 { body_end = abs_body_start + i; break; } }
                        _ => {}
                    }
                }

                let body = &class_body[abs_body_start..body_end];
                let body = body.replace("super.", &format!("{parent}.prototype."));
                // Replace ALL references to internal class name with var_name
                // e.g. _I18n.createInstance → I18n.createInstance, new _Tuple → new Tuple
                let body = if m.internal_name != *var_name {
                    body.replace(&m.internal_name, var_name)
                } else {
                    body
                };

                let target = if is_static { var_name.to_string() } else { format!("{var_name}.prototype") };
                methods.push(format!("{target}.{name} = function({params}) {{{body}}};"));

                pos = body_end + 1;
                continue;
            }

            pos += 1;
        }

        let methods_str = methods.join("\n");

        // Build constructor function
        let constructor_fn = if let Some((params, body)) = &constructor_body {
            // Replace internal class name references with var_name
            // e.g. _Tuple.prototype → Tuple.prototype, new _Tuple → new Tuple
            let body = if m.internal_name != *var_name {
                body.replace(&m.internal_name, var_name)
            } else {
                body.clone()
            };
            // Transform super(...args) → Reflect.construct(Parent, [args], ClassName)
            // and this → _this for property assignments after super()
            // Use paren-matching to handle nested parens in super() args
            let transformed = if let Some(super_pos) = body.find("super(") {
                let args_start = super_pos + 6; // after "super("
                // Find matching close paren
                let mut depth = 1;
                let mut args_end = args_start;
                for (i, ch) in body[args_start..].char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => { depth -= 1; if depth == 0 { args_end = args_start + i; break; } }
                        _ => {}
                    }
                }
                let args = body[args_start..args_end].trim();
                let super_call_end = args_end + 1; // include closing paren
                let reflect_call = if args.is_empty() {
                    format!("var _this = Reflect.construct({parent}, [], {var_name})")
                } else {
                    format!("var _this = Reflect.construct({parent}, [{args}], {var_name})")
                };
                format!("{}{}{}", &body[..super_pos], reflect_call, &body[super_call_end..])
            } else {
                // No super call — just wrap body
                body.clone()
            };
            let has_super = body.contains("super(");
            if has_super {
                // Replace `this` as whole word with `_this` (handles this.x, this, etc.)
                let this_re = regex::Regex::new(r"\bthis\b").unwrap();
                let transformed = this_re.replace_all(&transformed, "_this").to_string();
                format!("function {var_name}({params}) {{{transformed}\n    return _this;\n  }}")
            } else {
                // No super call — just use Reflect.construct
                format!("function {var_name}({params}) {{\n    return Reflect.construct({parent}, [], {var_name});\n  }}")
            }
        } else {
            // No constructor — default: forward all args to parent via Reflect.construct
            format!("function {var_name}() {{\n    return Reflect.construct({parent}, Array.prototype.slice.call(arguments), new.target || {var_name});\n  }}")
        };

        // For Array subclasses, add Symbol.species
        let species = if parent == "Array" {
            format!("\n  Object.defineProperty({var_name}, Symbol.species, {{ get: function() {{ return {var_name}; }} }});")
        } else {
            String::new()
        };

        // For class declarations (Pattern C), need `var` since there's no existing assignment
        let is_class_decl = m.start < code.len()
            && code[m.start..].starts_with("class ");
        let decl_prefix = if is_class_decl { "var " } else { "" };

        let replacement = format!(
            r#"{decl_prefix}{var_name} = (function() {{
  {constructor_fn}
  {var_name}.prototype = Object.create({parent}.prototype, {{
    constructor: {{ value: {var_name}, writable: true, configurable: true }}
  }});
  Object.setPrototypeOf({var_name}, {parent});{species}
  {methods_str}
  return {var_name};
}})();"#
        );

        result = format!("{}{}{}", &result[..m.start], replacement, &result[m.end..]);
    }

    result
}

/// Patch esbuild's runtime helpers for Hermes compatibility + mockability.
///
/// Two issues:
/// 1. Hermes bug: `for (let key of arr)` with arrow closures captures by reference,
///    not by value. All getters end up returning the last key's value.
///    Fix: rewrite __copyProps to use var + bind.
///
/// 2. esbuild creates non-configurable ESM namespace getters, making useMock impossible.
///    Fix: add configurable:true to __copyProps and __export.
fn patch_esbuild_for_hermes(code: &str) -> String {
    let original_len = code.len();
    let code_kb = code.len() / 1024;

    // Patch 1: Fix Hermes for-let-of closure bug in __copyProps
    let code = code.replacen(
        "for (let key of __getOwnPropNames(from))\n        if (!__hasOwnProp.call(to, key) && key !== except)\n          __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });",
        "var keys = __getOwnPropNames(from);\n      for (var i = 0; i < keys.length; i++) {\n        var key = keys[i];\n        if (!__hasOwnProp.call(to, key) && key !== except)\n          __defProp(to, key, { get: ((k) => from[k]).bind(null, key), enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable, configurable: true });\n      }",
        1,
    );

    // Patch 2: Make __export configurable for useMock
    let code = code.replacen(
        "{ get: all[name], enumerable: true }",
        "{ get: all[name], enumerable: true, configurable: true }",
        1,
    );

    // Patch 3: Make __toESM return mock Proxies directly (skip copy).
    // Our __require returns Proxies with __esModule=true for externalized modules.
    // __toESM normally copies properties into a new object, which destroys Proxy behavior.
    // Fix: insert early return at the start of __toESM.
    // Note: esbuild may rename `mod` to `mod2`, `mod3` etc. to avoid conflicts.
    let code = {
        let toesm_re = regex::Regex::new(r"var __toESM = \((\w+), isNodeMode, target\) => \(").unwrap();
        if let Some(caps) = toesm_re.captures(&code) {
            let mod_var = caps[1].to_string();
            let patched = toesm_re.replace(&code, |_caps: &regex::Captures| {
                format!("var __toESM = ({mod_var}, isNodeMode, target) => ({mod_var} && {mod_var}.__esModule ? {mod_var} : (")
            }).to_string();
            // Add closing paren for the ternary — right before the final ");".
            patched.replacen(
                &format!("{mod_var}\n  ));"),
                &format!("{mod_var}\n  )));"),
                1,
            )
        } else {
            code
        }
    };

    // Patch 4: Downlevel ALL `class extends Expr` patterns to function-based constructors.
    // Hermes bugs: (1) TDZ crash with `class X extends Variable` when Variable is local,
    // (2) super() in Array subclasses discards return value. Fix both by converting all
    // class-extends to Reflect.construct-based functions.
    let code = fix_all_class_extends(&code);

    let code_str = code;

    // Only warn if the unpatched for-let-of pattern is still present
    if code_str.contains("for (let key of __getOwnPropNames(from))") {
        eprintln!("WARNING: esbuild for-let-of patch did not match ({code_kb}KB bundle) — Hermes closure bug may cause failures");
    }

    code_str
}

/// Bundle test files via Metro's programmatic API (one-shot mode).
pub fn bundle_metro(entry_file: &Path, project_root: &Path) -> Result<String, String> {
    let bundler_script = generate_bundler_script(entry_file, project_root);

    let mut child = Command::new("node")
        .arg("-e")
        .arg(&bundler_script)
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn Metro bundler: {e}"))?;

    let mut stdout = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .map_err(|e| format!("Failed to read Metro output: {e}"))?;

    let status = child.wait().map_err(|e| format!("Metro process failed: {e}"))?;

    if !status.success() {
        let mut stderr = String::new();
        if let Some(mut err) = child.stderr.take() {
            let _ = err.read_to_string(&mut stderr);
        }
        return Err(format!(
            "Metro bundling failed (exit {}): {stderr}",
            status.code().unwrap_or(-1)
        ));
    }

    Ok(stdout)
}

fn generate_bundler_script(entry_file: &Path, project_root: &Path) -> String {
    let entry = entry_file.to_string_lossy();
    let root = project_root.to_string_lossy();

    format!(
        r#"
const Metro = require('metro');

async function main() {{
  const config = await Metro.loadConfig({{
    cwd: '{root}',
  }});

  config.maxWorkers = 1;
  config.reporter = {{ update() {{}} }};

  const result = await Metro.runBuild(config, {{
    entry: '{entry}',
    out: undefined,
    platform: 'ios',
    dev: true,
    minify: false,
  }});

  process.stdout.write(result.code);
}}

main().catch(e => {{
  process.stderr.write(e.message + '\n' + (e.stack || ''));
  process.exit(1);
}});
"#
    )
}

/// Find all test files. Uses testMatch pattern from config if set,
/// otherwise matches *.test.ts, *.test.tsx, *.test.js, *.test.jsx
pub fn find_test_files(root: &Path) -> Vec<PathBuf> {
    let cfg = read_config(root);
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
fn make_relative(from_dir: &Path, to_file: &Path) -> String {
    let from_parts: Vec<_> = from_dir.components().collect();
    let to_parts: Vec<_> = to_file.components().collect();
    // Find common prefix length
    let common = from_parts.iter().zip(to_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();
    // Go up from from_dir
    let ups = from_parts.len() - common;
    let mut result = String::new();
    for _ in 0..ups {
        result.push_str("../");
    }
    // Go down to to_file
    for (i, part) in to_parts[common..].iter().enumerate() {
        if i > 0 { result.push('/'); }
        result.push_str(&part.as_os_str().to_string_lossy());
    }
    if result.is_empty() || !result.starts_with('.') {
        format!("./{result}")
    } else {
        result
    }
}

/// Create shadow wrapper directories for aliased mock paths.
///
/// For each alias that has mocked sub-paths, creates a shadow directory tree:
/// - Non-mocked paths: symlinks to the real source
/// - Mocked paths: Proxy wrapper files that import the real module and delegate
///   to __HT_file_mocks[__currentTestFile] at access time
///
/// Returns updated BundleConfig with aliases pointing to shadow directories.
/// Caller must clean up shadow directories after bundling.
pub fn create_shadow_wrappers(
    project_root: &Path,
    mock_modules: &[String],
    cfg: &BundleConfig,
) -> (BundleConfig, Vec<PathBuf>) {
    let mut new_cfg = cfg.clone();
    let mut shadow_dirs: Vec<PathBuf> = Vec::new();

    // Group mocked paths by alias
    let mut alias_mocks: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for m in mock_modules {
        for (alias, _target) in &cfg.aliases {
            if m == alias || m.starts_with(&format!("{alias}/")) {
                alias_mocks.entry(alias.clone()).or_default().push(m.clone());
            }
        }
    }

    if alias_mocks.is_empty() {
        return (new_cfg, shadow_dirs);
    }

    for (alias, mocked_paths) in &alias_mocks {
        let Some((_alias_name, target)) = cfg.aliases.iter().find(|(a, _)| a == alias) else { continue };

        let shadow_dir = project_root.join(format!(".hermes-test-shadow-{}", alias.replace('/', "-").replace('@', "")));
        shadow_dirs.push(shadow_dir.clone());

        // Remove old shadow dir if exists, create fresh
        let _ = std::fs::remove_dir_all(&shadow_dir);

        // Create shadow by recursively symlinking from real source
        let real_dir = PathBuf::from(target);
        if !real_dir.exists() {
            continue;
        }
        create_shadow_tree(&real_dir, &shadow_dir, target, alias, &mocked_paths);

        // Add a "real" alias that wrappers use to import the actual module
        let real_alias = format!("@__ht_real/{}", alias.trim_start_matches('@'));
        new_cfg.aliases.push((real_alias.clone(), target.clone()));

        // Update the main alias to point to shadow directory
        for (a, t) in &mut new_cfg.aliases {
            if a == alias {
                *t = shadow_dir.to_string_lossy().to_string();
            }
        }
    }

    (new_cfg, shadow_dirs)
}

/// Create auto-generated Proxy shims for non-aliased mocked packages.
/// Same pattern as shadow wrappers: CJS Proxy that checks per-file mocks
/// and falls through to the real module. Replaces externalization entirely.
///
/// Returns (updated_cfg, wrapper_dir_to_cleanup, remaining_externals).
/// remaining_externals = mocks that couldn't be shimmed (config externals, shims, etc.)
pub fn create_package_shims(
    project_root: &Path,
    non_aliased_mocks: &[String],
    cfg: &BundleConfig,
) -> (BundleConfig, Option<PathBuf>, Vec<String>) {
    let mut new_cfg = cfg.clone();
    let mut remaining: Vec<String> = Vec::new();

    if non_aliased_mocks.is_empty() {
        return (new_cfg, None, remaining);
    }

    let mut shimmable: Vec<&String> = Vec::new();
    for m in non_aliased_mocks {
        let is_config_external = cfg.externals.iter().any(|e| {
            let e_base = e.trim_end_matches('*').trim_end_matches('/');
            m == e_base || m.starts_with(&format!("{e_base}/"))
                || (e.ends_with('*') && m.starts_with(e_base))
        });
        let is_shim = cfg.shims.iter().any(|(s, _)| s == m);
        let is_wrapper_shim = cfg.wrapper_shims.iter().any(|(s, _)| s == m);
        let is_relative = m.starts_with('.') || m.starts_with('/');
        let is_hardcoded = m == "react-native" || m.starts_with("react-native/")
            || m.starts_with("@react-native/");
        if is_config_external || is_shim || is_wrapper_shim || is_relative || is_hardcoded {
            remaining.push(m.clone());
        } else {
            shimmable.push(m);
        }
    }

    if shimmable.is_empty() {
        return (new_cfg, None, remaining);
    }

    let shim_dir = project_root.join(".hermes-test-pkg-shims");
    let _ = std::fs::remove_dir_all(&shim_dir);
    let _ = std::fs::create_dir_all(&shim_dir);

    for pkg in &shimmable {
        let safe_name = pkg.replace('@', "").replace('/', "__");
        let shim_path = shim_dir.join(format!("{safe_name}.js"));
        let real_alias = format!("@__ht_real_pkg/{pkg}");

        // CJS Proxy wrapper — same pattern as shadow wrappers
        let wrapper = format!(
            r#"var _loaded = null; var _loading = false;
function _getReal() {{ if (_loaded) return _loaded; if (_loading) return {{}}; _loading = true; _loaded = require("{real_alias}"); _loading = false; return _loaded; }}
var _h = {{
  get: function(t, p) {{
    if (p === '__esModule') return true;
    if (typeof p === 'symbol') return undefined;
    var fm = globalThis.__HT_file_mocks;
    var f = globalThis.__currentTestFile;
    var mocks = fm && f && fm[f];
    var m = mocks && mocks['{pkg}'];
    if (m && p in m) return m[p];
    return _getReal()[p];
  }},
  ownKeys: function(t) {{
    var r = _getReal();
    try {{ return Object.getOwnPropertyNames(r); }} catch(e) {{ return []; }}
  }},
  getOwnPropertyDescriptor: function(t, p) {{
    var r = _getReal();
    try {{
      var d = Object.getOwnPropertyDescriptor(r, p);
      if (d) {{ return {{ configurable: true, enumerable: d.enumerable, writable: true, value: d.get ? d.get() : d.value }}; }}
    }} catch(e) {{}}
    return {{ configurable: true, enumerable: false, writable: true, value: undefined }};
  }}
}};
module.exports = typeof Proxy !== 'undefined' ? new Proxy({{}}, _h) : {{}};
"#,
        );

        if std::fs::write(&shim_path, &wrapper).is_err() {
            remaining.push((*pkg).clone());
            continue;
        }

        // Alias pkg → shim file (esbuild aliases run before externals → bundled, not external)
        new_cfg.aliases.push(((*pkg).clone(), shim_path.to_string_lossy().to_string()));
        // Alias @__ht_real_pkg/pkg → real package (esbuild resolves from node_modules)
        new_cfg.aliases.push((real_alias, (*pkg).clone()));
    }

    (new_cfg, Some(shim_dir), remaining)
}

/// Create wrapper shims for built-in hermes-test ecosystem shims.
/// These are thin wrappers around real packages (not replacements) that fix
/// module identity issues and add test instrumentation.
///
/// Shim resolution is agnostic: hermes-test/shims/<name> resolves to
/// <hermes-test-package>/shims/<name>.js on disk. No hardcoded registry.
///
/// Each shim file uses `require('@__ht_real_pkg/<module>')` to access
/// the real package. The bundler adds esbuild aliases to wire this up.
///
/// Returns (updated_cfg, wrapper_dir_to_cleanup).
pub fn create_wrapper_shims(
    project_root: &Path,
    cfg: &BundleConfig,
) -> (BundleConfig, Option<PathBuf>) {
    let mut new_cfg = cfg.clone();

    if cfg.wrapper_shims.is_empty() {
        return (new_cfg, None);
    }

    // Find all candidate shims directories (node_modules, monorepo root, dev source).
    let shims_dirs = find_hermes_test_shims_dirs(project_root, cfg);

    let shim_dir = project_root.join(".hermes-test-wrapper-shims");
    let _ = std::fs::remove_dir_all(&shim_dir);
    let _ = std::fs::create_dir_all(&shim_dir);

    let mut created = false;

    for (module_name, builtin_name) in &cfg.wrapper_shims {
        // Resolve the shim file from all candidate directories
        let content = resolve_shim_content(&shims_dirs, builtin_name);
        let content = match content {
            Some(c) => c,
            None => {
                eprintln!("Warning: shim 'hermes-test/shims/{builtin_name}' not found. \
                    Looked in: {:?}", shims_dirs);
                continue;
            }
        };

        let safe_name = module_name.replace('@', "").replace('/', "__");
        let shim_path = shim_dir.join(format!("{safe_name}.js"));
        let real_alias = format!("@__ht_real_pkg/{module_name}");

        if std::fs::write(&shim_path, &content).is_err() {
            eprintln!("Warning: failed to write wrapper shim for {module_name}");
            continue;
        }

        // esbuild alias: module → shim file
        new_cfg.aliases.push((module_name.clone(), shim_path.to_string_lossy().to_string()));
        // esbuild alias: @__ht_real_pkg/module → real package
        new_cfg.aliases.push((real_alias, module_name.clone()));

        // Remove from externals if present — the real package must be bundled
        new_cfg.externals.retain(|e| {
            let e_base = e.trim_end_matches('*').trim_end_matches('/');
            !(module_name == e_base || (e.ends_with('*') && module_name.starts_with(e_base)))
        });

        created = true;
    }

    if created {
        (new_cfg, Some(shim_dir))
    } else {
        let _ = std::fs::remove_dir_all(&shim_dir);
        (new_cfg, None)
    }
}

/// Find ALL hermes-test shims directories on disk.
/// Returns all found directories — file resolution tries each in order.
fn find_hermes_test_shims_dirs(project_root: &Path, cfg: &BundleConfig) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let nm_candidates = [
        "node_modules/hermes-test/shims",
        "node_modules/hermes-test/src/shims",
        "node_modules/@marcuzgabriel/hermes-test/shims",
        "node_modules/@marcuzgabriel/hermes-test/src/shims",
    ];
    // Check project root node_modules
    for c in &nm_candidates {
        let p = project_root.join(c);
        if p.is_dir() { dirs.push(p); }
    }
    // Check monorepo root node_modules
    if let Some(ref root) = cfg.root {
        for c in &nm_candidates {
            let p = root.join(c);
            if p.is_dir() && !dirs.contains(&p) { dirs.push(p); }
        }
    }
    // Dev fallback: resolve relative to the binary (repo layout)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for ancestor in exe_dir.ancestors().skip(1) {
                let dev_shims = ancestor.join("packages/hermes-test/src/shims");
                if dev_shims.is_dir() && !dirs.contains(&dev_shims) {
                    dirs.push(dev_shims);
                    break;
                }
            }
        }
    }
    dirs
}

/// Resolve shim content by name. Purely file-based — no hardcoded registry.
/// Searches ALL candidate shims directories for <name>.js (not just the first dir found).
fn resolve_shim_content(shims_dirs: &[PathBuf], name: &str) -> Option<String> {
    for dir in shims_dirs {
        let path = dir.join(format!("{name}.js"));
        if let Ok(content) = std::fs::read_to_string(&path) {
            return Some(content);
        }
    }
    None
}

/// Recursively create a shadow directory tree:
/// - Directories: create real directory, recurse
/// - Files matching mocked paths: create Proxy wrapper
/// - Other files: create symlink to real file
fn create_shadow_tree(
    real_dir: &Path,
    shadow_dir: &Path,
    alias_target: &str,
    alias_name: &str,
    mocked_paths: &[String],
) {
    let _ = std::fs::create_dir_all(shadow_dir);

    let Ok(entries) = std::fs::read_dir(real_dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let real_path = entry.path();
        let shadow_path = shadow_dir.join(&name);

        if real_path.is_dir() {
            // Check if any mocked path falls under this directory
            let rel = real_path.strip_prefix(alias_target).unwrap_or(&real_path);
            let dir_mock_prefix = format!("{alias_name}/{}", rel.to_string_lossy());
            let has_mocked_child = mocked_paths.iter().any(|m| m.starts_with(&dir_mock_prefix));
            if has_mocked_child {
                // Recurse into this directory
                create_shadow_tree(&real_path, &shadow_path, alias_target, alias_name, mocked_paths);
            } else {
                // Symlink entire directory
                let _ = std::os::unix::fs::symlink(&real_path, &shadow_path);
            }
        } else {
            // Check if this file matches a mocked path.
            // Two patterns:
            // 1. Direct: hooks/redux/useRedux.ts → @scope/hooks/redux/useRedux
            // 2. Index/barrel: hooks/index.ts → @scope/hooks (directory import)
            let rel = real_path.strip_prefix(alias_target).unwrap_or(&real_path);
            let rel_no_ext = rel.with_extension("");
            let rel_str = rel_no_ext.to_string_lossy();
            let direct_path = format!("{alias_name}/{rel_str}");
            // For index files, also check the parent directory path
            let is_index = rel_str.ends_with("/index") || rel_str == "index";
            let index_path = if is_index {
                let parent = rel_str.trim_end_matches("/index").trim_end_matches("index");
                if parent.is_empty() {
                    alias_name.to_string()
                } else {
                    format!("{alias_name}/{}", parent.trim_end_matches('/'))
                }
            } else {
                String::new()
            };
            let matched_mock = mocked_paths.iter().find(|m| {
                **m == direct_path || (is_index && **m == index_path)
            });

            if let Some(mock_path_str) = matched_mock {
                // Create Proxy wrapper. Use the @__ht_real/ alias to import the
                // real module via esbuild's resolution (same bundled instance).
                let real_alias = format!("@__ht_real/{}", alias_name.trim_start_matches('@'));
                let suffix = mock_path_str.strip_prefix(alias_name).unwrap_or("");
                let require_path = format!("{real_alias}{suffix}");

                // For barrel/index files, also check sub-path mocks.
                // When a barrel re-exports from sub-modules and those sub-modules are mocked,
                // the barrel Proxy should delegate property access to the sub-path mock.
                // Example: import { useErrorHandling } from '@scope/hooks' should check
                // mocks registered for '@scope/hooks/errorHandling/useErrorHandling'.
                let barrel_prefix = format!("{mock_path_str}/");
                let has_sub_mocks = is_index && mocked_paths.iter().any(|m| m.starts_with(&barrel_prefix));

                let sub_mock_check = if has_sub_mocks {
                    // Generate code that checks all sub-path mocks under this barrel
                    format!(
                        r#"
  // Barrel sub-path mock delegation: check all mocked sub-paths
  if (mocks) {{
    for (var k in mocks) {{
      if (k.indexOf('{}') === 0 && p in mocks[k]) return mocks[k][p];
    }}
  }}"#,
                        barrel_prefix,
                    )
                } else {
                    String::new()
                };

                let wrapper = format!(
                    r#"var _loaded = null; var _loading = false;
function _getReal() {{ if (_loaded) return _loaded; if (_loading) return {{}}; _loading = true; _loaded = require("{}"); _loading = false; return _loaded; }}
var _h = {{
  get: function(t, p) {{
    if (p === '__esModule') return true;
    if (typeof p === 'symbol') return undefined;
    if (p === '__DEV__') return false;
    var fm = globalThis.__HT_file_mocks;
    var f = globalThis.__currentTestFile;
    var mocks = fm && f && fm[f];
    var m = mocks && mocks['{}'];
    if (m && p in m) return m[p];{}
    return _getReal()[p];
  }},
  ownKeys: function(t) {{
    var r = _getReal();
    try {{ return Object.getOwnPropertyNames(r); }} catch(e) {{ return []; }}
  }},
  getOwnPropertyDescriptor: function(t, p) {{
    var r = _getReal();
    try {{
      var d = Object.getOwnPropertyDescriptor(r, p);
      if (d) {{ return {{ configurable: true, enumerable: d.enumerable, writable: true, value: d.get ? d.get() : d.value }}; }}
    }} catch(e) {{}}
    return {{ configurable: true, enumerable: false, writable: true, value: undefined }};
  }}
}};
module.exports = typeof Proxy !== 'undefined' ? new Proxy({{}}, _h) : {{}};
"#,
                    require_path,
                    mock_path_str,
                    sub_mock_check,
                );
                let _ = std::fs::write(&shadow_path, wrapper);
            } else {
                // Check if this file is a barrel/index file that re-exports mocked children,
                // or sits in the same directory as a mocked sibling.
                // esbuild resolves symlinks, so relative imports from symlinked files
                // bypass the shadow tree. Copy these files so relative imports resolve
                // within the shadow directory (where proxy wrappers live).
                let same_dir_has_mock = mocked_paths.iter().any(|m| {
                    if let Some(parent) = rel_str.rfind('/') {
                        let dir_prefix = format!("{alias_name}/{}/", &rel_str[..parent]);
                        m.starts_with(&dir_prefix)
                    } else {
                        // Root level — check if any mock is at root level
                        !m.contains('/') || m.starts_with(&format!("{alias_name}/"))
                    }
                });
                let is_barrel = is_index || name_str.ends_with("index.ts") || name_str.ends_with("index.tsx") || name_str.ends_with("index.js");

                // If this barrel has mocked children, create a Proxy wrapper with
                // sub-path delegation instead of copying. This ensures barrel imports
                // like `import { X } from '@scope/helpers'` check sub-path mocks
                // even when the barrel itself isn't explicitly mocked.
                let barrel_mock_prefix = if is_barrel {
                    let barrel_alias = if is_index {
                        index_path.clone()
                    } else {
                        direct_path.clone()
                    };
                    let pfx = format!("{}/", if barrel_alias.is_empty() { &direct_path } else { &barrel_alias });
                    if mocked_paths.iter().any(|m| m.starts_with(&pfx)) {
                        Some((if barrel_alias.is_empty() { direct_path.clone() } else { barrel_alias }, pfx))
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some((barrel_alias_path, barrel_prefix)) = barrel_mock_prefix {
                    // Create Proxy wrapper for barrel with sub-path delegation
                    let real_alias = format!("@__ht_real/{}", alias_name.trim_start_matches('@'));
                    let suffix = barrel_alias_path.strip_prefix(alias_name).unwrap_or("");
                    let require_path = format!("{real_alias}{suffix}");
                    let wrapper = format!(
                        r#"var _loaded = null; var _loading = false;
function _getReal() {{ if (_loaded) return _loaded; if (_loading) return {{}}; _loading = true; _loaded = require("{}"); _loading = false; return _loaded; }}
var _h = {{
  get: function(t, p) {{
    if (p === '__esModule') return true;
    if (typeof p === 'symbol') return undefined;
    if (p === '__DEV__') return false;
    var fm = globalThis.__HT_file_mocks;
    var f = globalThis.__currentTestFile;
    var mocks = fm && f && fm[f];
    var m = mocks && mocks['{}'];
    if (m && p in m) return m[p];
    if (mocks) {{ for (var k in mocks) {{ if (k.indexOf('{}') === 0 && p in mocks[k]) return mocks[k][p]; }} }}
    return _getReal()[p];
  }},
  ownKeys: function(t) {{
    var r = _getReal();
    try {{ return Object.getOwnPropertyNames(r); }} catch(e) {{ return []; }}
  }},
  getOwnPropertyDescriptor: function(t, p) {{
    var r = _getReal();
    try {{
      var d = Object.getOwnPropertyDescriptor(r, p);
      if (d) {{ return {{ configurable: true, enumerable: d.enumerable, writable: true, value: d.get ? d.get() : d.value }}; }}
    }} catch(e) {{}}
    return {{ configurable: true, enumerable: false, writable: true, value: undefined }};
  }}
}};
module.exports = typeof Proxy !== 'undefined' ? new Proxy({{}}, _h) : {{}};
"#,
                        require_path,
                        barrel_alias_path,
                        barrel_prefix,
                    );
                    let _ = std::fs::write(&shadow_path, wrapper);
                } else if is_barrel || same_dir_has_mock {
                    // Copy: relative imports will resolve within shadow tree
                    let _ = std::fs::copy(&real_path, &shadow_path);
                } else {
                    // Symlink: safe because no mocked siblings
                    let _ = std::os::unix::fs::symlink(&real_path, &shadow_path);
                }
            }
        }
    }
}

/// Pre-transform test files that mock aliased paths. Vitest-style approach:
/// uses OXC AST parser to rewrite imports of mocked modules to lazy Proxy
/// wrappers that read from __HT_file_mocks at access time, while keeping
/// the real module available for non-test code via the active alias.
///
/// Returns a map of original_path → temp_path for transformed files.
pub fn pre_transform_test_files(
    test_files: &[PathBuf],
    mock_modules: &[String],
    cfg: &BundleConfig,
) -> Vec<(PathBuf, PathBuf)> {
    let mut transforms: Vec<(PathBuf, PathBuf)> = Vec::new();

    // Find which mock paths are aliased (those need pre-transform)
    let aliased_mocks: Vec<&String> = mock_modules.iter().filter(|m| {
        cfg.aliases.iter().any(|(alias, _)| {
            *m == alias || m.starts_with(&format!("{alias}/"))
        })
    }).collect();

    if aliased_mocks.is_empty() {
        return transforms;
    }

    for file in test_files {
        let Ok(source) = std::fs::read_to_string(file) else { continue };

        // Check if this file uses any aliased mocks
        let file_aliased_mocks: Vec<&&String> = aliased_mocks.iter()
            .filter(|m| source.contains(&format!("mockModule('{m}'")))
            .collect();

        if file_aliased_mocks.is_empty() {
            continue;
        }

        if let Some(transformed) = oxc_transform_test_file(&source, file, &file_aliased_mocks) {
            let temp_path = file.with_extension("__ht_transformed.ts");
            if std::fs::write(&temp_path, &transformed).is_ok() {
                transforms.push((file.clone(), temp_path));
            }
        }
    }

    transforms
}

/// Use OXC to parse a test file, find imports of mocked aliased paths,
/// and rewrite them to Proxy-based lazy accessors.
///
/// Transform: `import { useX, useY } from '@scope/pkg/hooks';`
/// Becomes:   (import removed, all refs to useX → __htMock0__.useX)
///
/// The Proxy reads from __HT_file_mocks[__currentTestFile]['@scope/pkg/hooks']
/// at access time, so different test files get different mocks.
fn oxc_transform_test_file(
    source: &str,
    file_path: &Path,
    aliased_mocks: &[&&String],
) -> Option<String> {
    use oxc::allocator::Allocator;
    use oxc::parser::Parser;
    use oxc::span::SourceType;

    let allocator = Allocator::default();
    let source_type = SourceType::from_path(file_path).unwrap_or_default();
    let ret = Parser::new(&allocator, source, source_type).parse();

    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }

    // Collect: for each aliased mock path, find import declarations and their local names.
    // We'll build text replacements sorted by position (reverse order for safe string splicing).
    struct ImportInfo {
        mock_path: String,
        proxy_var: String,
        import_start: u32,
        import_end: u32,
        // (local_name, span_start, span_end) for all IdentifierReferences to rewrite
        local_names: Vec<String>,
    }

    let mut imports_to_rewrite: Vec<ImportInfo> = Vec::new();

    for stmt in &ret.program.body {
        if let oxc::ast::ast::Statement::ImportDeclaration(import_decl) = stmt {
            let import_source = import_decl.source.value.as_str();
            // Check if this import's source matches any aliased mock
            let mock_idx = aliased_mocks.iter().position(|m| **m == import_source);
            if let Some(idx) = mock_idx {
                let mut local_names = Vec::new();

                // Type-only imports can be ignored (esbuild strips them)
                if import_decl.import_kind == oxc::ast::ast::ImportOrExportKind::Type {
                    continue;
                }

                // Collect named imports: import { a, b as c } from '...'
                if let Some(specifiers) = &import_decl.specifiers {
                    for spec in specifiers {
                        match spec {
                            oxc::ast::ast::ImportDeclarationSpecifier::ImportSpecifier(s) => {
                                if s.import_kind == oxc::ast::ast::ImportOrExportKind::Type {
                                    continue;
                                }
                                local_names.push(s.local.name.to_string());
                            }
                            oxc::ast::ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                                local_names.push(s.local.name.to_string());
                            }
                            oxc::ast::ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                                local_names.push(s.local.name.to_string());
                            }
                        }
                    }
                }

                if !local_names.is_empty() {
                    imports_to_rewrite.push(ImportInfo {
                        mock_path: import_source.to_string(),
                        proxy_var: format!("__htMock{}__", idx),
                        import_start: import_decl.span.start,
                        import_end: import_decl.span.end,
                        local_names,
                    });
                }
            }
        }
    }

    if imports_to_rewrite.is_empty() {
        return None;
    }

    // Now use OXC semantic analysis to find all references to imported names
    // and collect their spans for replacement.
    let semantic_ret = oxc::semantic::SemanticBuilder::new()
        .build(&ret.program);

    let semantic = &semantic_ret.semantic;
    let scoping = semantic.scoping();
    let nodes = semantic.nodes();

    // Collect all replacements: (start, end, replacement_text)
    let mut replacements: Vec<(u32, u32, String)> = Vec::new();

    // 1. Replace import declarations with Proxy wrappers
    for info in &imports_to_rewrite {
        let proxy_code = format!(
            "var {} = new Proxy({{}}, {{ get: function(_t,_p) {{ \
             var _fm = globalThis.__HT_file_mocks; \
             var _f = globalThis.__currentTestFile; \
             var _m = _fm && _f && _fm[_f] && _fm[_f]['{}']; \
             return _m ? _m[_p] : _t[_p]; }} }});",
            info.proxy_var, info.mock_path,
        );
        replacements.push((info.import_start, info.import_end, proxy_code));
    }

    // 2. Find all references to imported names and replace with proxy accessor.
    for info in &imports_to_rewrite {
        for local_name in &info.local_names {
            for symbol_id in scoping.symbol_ids() {
                if scoping.symbol_name(symbol_id) != local_name {
                    continue;
                }
                // Only top-level bindings (root/module scope)
                let scope_id = scoping.symbol_scope_id(symbol_id);
                if scope_id != scoping.root_scope_id() {
                    continue;
                }
                // Found the import binding — replace all references
                for reference in scoping.get_resolved_references(symbol_id) {
                    let node = nodes.get_node(reference.node_id());
                    use oxc::span::GetSpan;
                    let span = node.span();
                    replacements.push((
                        span.start,
                        span.end,
                        format!("{}.{}", info.proxy_var, local_name),
                    ));
                }
                break;
            }
        }
    }

    // Sort replacements in reverse order by start position for safe string splicing
    replacements.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = source.to_string();
    for (start, end, replacement) in &replacements {
        let s = *start as usize;
        let e = *end as usize;
        if s <= result.len() && e <= result.len() && s <= e {
            result = format!("{}{}{}", &result[..s], replacement, &result[e..]);
        }
    }

    Some(result)
}

/// Generate a synthetic entry file that imports the harness and all test files.
/// `transforms` maps original test file paths to pre-transformed temp paths.
pub fn generate_entry(
    test_files: &[PathBuf],
    _harness_path: Option<&Path>,
    mock_modules: &[String],
    cfg: &BundleConfig,
    transforms: &[(PathBuf, PathBuf)],
) -> String {
    let mut entry = String::new();

    // Common globals expected by RN libraries
    entry.push_str("if (typeof globalThis.__DEV__ === 'undefined') globalThis.__DEV__ = false;\n");

    // Register hermes-test as a mock so `import { test } from 'hermes-test'` resolves to __HT
    entry.push_str("globalThis.__HT_mocks = globalThis.__HT_mocks || {};\n");
    entry.push_str("globalThis.__HT_mocks['hermes-test'] = globalThis.__HT;\n");
    entry.push_str("globalThis.__HT_mocks['@marcuzgabriel/hermes-test'] = globalThis.__HT;\n");

    // Load shims for native modules. User shims (from config) override built-in defaults.
    // Built-in shims are embedded in the hermes-test binary.
    {
        let builtin_shims = vec![
            ("react-native", include_str!("../../../packages/hermes-test/src/shims/react-native.js")),
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
        // Extract just the filename for display (e.g. "useLogin.test.ts")
        let display_name = file.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        entry.push_str(&format!(
            "if (globalThis.__HT && globalThis.__HT.resetMockModulePatches) globalThis.__HT.resetMockModulePatches();\nglobalThis.__currentTestFile = '{}';\ntry {{ require('{}'); }} catch(e) {{ if (globalThis.__HT) globalThis.__HT.registerCrash('{}', String(e && e.message || e)); }}\n",
            display_name, require_path, display_name
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

/// Bundle with esbuild and return the dependency graph (metafile).
/// Returns (bundle_code, map of input_file → Vec<test_files_that_import_it>).
pub fn bundle_with_depgraph(
    entry_file: &Path,
    project_root: &Path,
    test_files: &[PathBuf],
    external_modules: &[String],
) -> Result<(String, DepGraph), String> {
    let esbuild_path = find_esbuild(project_root)
        .map_err(|_| "esbuild not found".to_string())?;

    let metafile_path = project_root.join(".hermes-test-meta.json");
    let outfile_path = project_root.join(".hermes-test-bundle.js");

    let mut cmd = Command::new(&esbuild_path);
    cmd.arg(entry_file)
        .arg("--bundle")
        .arg("--format=iife")
        .arg("--target=es2020")
        .arg("--supported:async-await=false")
        .arg("--define:process.env.NODE_ENV=\"test\"")
        .arg("--define:process.env.JEST_WORKER_ID=\"1\"")
        .arg("--define:global=globalThis")
        .arg(format!("--metafile={}", metafile_path.to_string_lossy()))
        .arg(format!("--outfile={}", outfile_path.to_string_lossy()));

    for ext in external_modules {
        cmd.arg(format!("--external:{ext}"));
    }

    let output = cmd
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run esbuild: {e}"))?;

    if !output.status.success() {
        let _ = std::fs::remove_file(&metafile_path);
        let _ = std::fs::remove_file(&outfile_path);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("esbuild failed: {stderr}"));
    }

    let code = std::fs::read_to_string(&outfile_path)
        .map_err(|e| format!("Failed to read bundle: {e}"))?;
    let _ = std::fs::remove_file(&outfile_path);
    let mut code = patch_esbuild_for_hermes(&code);

    if !external_modules.is_empty() {
        code = inject_mock_require_shim(&code);
    }

    // Hoist mockModule() calls before require() calls so aliased shadow-wrapper mocks
    // are registered before the module initializers run
    code = hoist_mock_modules(&code);

    // Parse the metafile to build a dependency graph
    let depgraph = parse_depgraph(&metafile_path, project_root, test_files);
    let _ = std::fs::remove_file(&metafile_path);

    Ok((code, depgraph))
}

/// Maps source files → which test files depend on them.
pub type DepGraph = std::collections::HashMap<PathBuf, Vec<PathBuf>>;

fn parse_depgraph(
    metafile_path: &Path,
    project_root: &Path,
    test_files: &[PathBuf],
) -> DepGraph {
    let mut graph: DepGraph = std::collections::HashMap::new();

    let meta_str = match std::fs::read_to_string(metafile_path) {
        Ok(s) => s,
        Err(_) => return graph,
    };

    let meta: serde_json::Value = match serde_json::from_str(&meta_str) {
        Ok(v) => v,
        Err(_) => return graph,
    };

    // esbuild metafile structure: { "inputs": { "src/foo.ts": { "imports": [{ "path": "src/bar.ts" }] } } }
    let inputs = match meta["inputs"].as_object() {
        Some(o) => o,
        None => return graph,
    };

    // Build reverse map: for each source file, which test files transitively import it?
    let test_file_strs: Vec<String> = test_files
        .iter()
        .filter_map(|f| {
            // Canonicalize to absolute, then strip project root to get relative path
            let abs = std::fs::canonicalize(f).unwrap_or_else(|_| project_root.join(f));
            abs.strip_prefix(project_root)
                .ok()
                .map(|rel| rel.to_string_lossy().to_string())
        })
        .collect();

    for test_rel in &test_file_strs {
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![test_rel.clone()];

        while let Some(current) = stack.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }

            if let Some(input) = inputs.get(&current) {
                if let Some(imports) = input["imports"].as_array() {
                    for imp in imports {
                        if let Some(path) = imp["path"].as_str() {
                            if !path.contains("node_modules") {
                                stack.push(path.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Every file in `visited` maps back to this test file
        let test_path = project_root.join(test_rel);
        for dep in visited {
            let dep_path = project_root.join(&dep);
            graph.entry(dep_path).or_default().push(test_path.clone());
        }
    }

    graph
}

// --- Bundle splitting for large test suites ---

pub struct SplitBundle {
    pub vendor: String,
    pub groups: Vec<String>,
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

fn compute_bundle_cache_key(
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

fn collect_source_mtimes(dir: &Path, out: &mut Vec<(String, u64)>) {
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

fn load_cached_split(project_root: &Path, cache_key: &str) -> Option<SplitBundle> {
    let cache_dir = project_root.join(".hermes-test-cache");
    let vendor = std::fs::read_to_string(cache_dir.join(format!("split-vendor-{cache_key}.js"))).ok()?;
    let manifest: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(cache_dir.join(format!("split-manifest-{cache_key}.json"))).ok()?
    ).ok()?;
    let mut groups = Vec::new();
    for name in &manifest {
        groups.push(std::fs::read_to_string(cache_dir.join(name)).ok()?);
    }
    Some(SplitBundle { vendor, groups })
}

fn save_cached_split(project_root: &Path, cache_key: &str, split: &SplitBundle) {
    let cache_dir = project_root.join(".hermes-test-cache");
    let _ = std::fs::create_dir_all(&cache_dir);
    // Clean old split cache files
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let n = entry.file_name();
            let n = n.to_string_lossy();
            if n.starts_with("split-") && !n.contains(cache_key) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let _ = std::fs::write(cache_dir.join(format!("split-vendor-{cache_key}.js")), &split.vendor);
    let mut group_names = Vec::new();
    for (i, group) in split.groups.iter().enumerate() {
        let name = format!("split-group-{cache_key}-{i}.js");
        let _ = std::fs::write(cache_dir.join(&name), group);
        group_names.push(name);
    }
    let _ = std::fs::write(
        cache_dir.join(format!("split-manifest-{cache_key}.json")),
        serde_json::to_string(&group_names).unwrap_or_default(),
    );
}

/// Bundle with vendor/group splitting to avoid Hermes super-linear scaling.
/// Uses disk cache keyed on source file mtimes — skips esbuild when nothing changed.
pub fn bundle_split(
    test_files: &[PathBuf],
    project_root: &Path,
    mock_modules: &[String],
    cfg: &BundleConfig,
) -> Result<SplitBundle, String> {
    // Check esbuild output cache first
    let cache_key = compute_bundle_cache_key(test_files, project_root, mock_modules, cfg);
    if let Some(cached) = load_cached_split(project_root, &cache_key) {
        return Ok(cached);
    }

    let esbuild_path = find_esbuild(project_root)
        .map_err(|_| "esbuild not found. Install it: bun add -d esbuild".to_string())?;

    let group_size = 10;
    let mut group_bundles = Vec::new();
    let mut all_packages = std::collections::HashSet::new();

    // Modules excluded from vendor (shimmed, externalized, or handled by harness)
    let excluded: std::collections::HashSet<&str> = {
        let mut set = std::collections::HashSet::new();
        set.insert("hermes-test");
        set.insert("@marcuzgabriel/hermes-test");
        set.insert("react-native");
        set.insert("console");
        for ext in &cfg.externals {
            set.insert(ext.as_str());
        }
        set
    };

    // Create wrapper shims for built-in ecosystem shims (hermes-test/shims/*)
    let (wrapper_cfg, wrapper_shim_dir) = create_wrapper_shims(project_root, cfg);
    // Create shadow wrappers for aliased mock paths (same as single-bundle mode)
    let (shadow_cfg, shadow_dirs) = create_shadow_wrappers(project_root, mock_modules, &wrapper_cfg);
    // Filter out aliased mock paths — shadow wrappers handle them
    let non_aliased_mocks: Vec<String> = mock_modules.iter().filter(|m| {
        !cfg.aliases.iter().any(|(alias, _)| *m == alias || m.starts_with(&format!("{alias}/")))
    }).cloned().collect();
    // Create package shims for non-aliased mocks (same Proxy pattern as shadow wrappers)
    let (shim_cfg, shim_dir, remaining_externals) =
        create_package_shims(project_root, &non_aliased_mocks, &shadow_cfg);

    // Step 1: Bundle each group with --packages=external (fast, local code only)
    for (i, chunk) in test_files.chunks(group_size).enumerate() {
        let entry = generate_group_entry(chunk, mock_modules);
        let entry_path = project_root.join(format!(".hermes-test-group-{i}.js"));
        std::fs::write(&entry_path, &entry)
            .map_err(|e| format!("Failed to write group entry: {e}"))?;

        let code = bundle_esbuild_with_config(
            &entry_path, &esbuild_path, &remaining_externals, &shim_cfg, true,
        )?;
        let _ = std::fs::remove_file(&entry_path);

        // Extract __require("...") calls to discover needed packages
        // Skip relative paths (mock modules like ./useIsLoading) — those stay external
        for pkg in extract_required_packages(&code) {
            if pkg.starts_with('.') || pkg.starts_with('/') {
                continue;
            }
            if !excluded.contains(pkg.as_str())
                && !cfg.externals.iter().any(|e| pkg_matches_external(&pkg, e))
            {
                all_packages.insert(pkg);
            }
        }

        group_bundles.push(code);
    }

    // Step 2: Build vendor bundle with all discovered packages
    let setup = generate_setup_code(test_files, mock_modules, cfg);
    let packages: Vec<String> = all_packages.into_iter().collect();

    let mut packages = packages;
    packages.sort(); // Deterministic order for cache stability

    let vendor = if packages.is_empty() {
        // No packages to vendor — just run setup code raw
        setup
    } else {
        let vendor_entry = generate_vendor_entry(&packages, &setup);
        let vendor_entry_path = project_root.join(".hermes-test-vendor.js");
        std::fs::write(&vendor_entry_path, &vendor_entry)
            .map_err(|e| format!("Failed to write vendor entry: {e}"))?;

        // Vendor must NOT skip aliases — it needs to bundle the real source code
        // so __HT_mocks has real implementations for aliased paths.
        // Use empty mock_modules so aliases aren't skipped (line 326-328).
        let vendor_code = bundle_esbuild_with_config(
            &vendor_entry_path, &esbuild_path, &[], cfg, false,
        )?;
        let _ = std::fs::remove_file(&vendor_entry_path);
        vendor_code
    };

    // Clean up shadow dirs, shim dirs, and wrapper shim dir
    for dir in &shadow_dirs { let _ = std::fs::remove_dir_all(dir); }
    if let Some(ref d) = shim_dir { let _ = std::fs::remove_dir_all(d); }
    if let Some(ref d) = wrapper_shim_dir { let _ = std::fs::remove_dir_all(d); }

    let result = SplitBundle { vendor, groups: group_bundles };
    save_cached_split(project_root, &cache_key, &result);
    Ok(result)
}

fn pkg_matches_external(pkg: &str, external: &str) -> bool {
    if external.ends_with('*') {
        pkg.starts_with(&external[..external.len() - 1])
    } else {
        pkg == external || pkg.starts_with(&format!("{external}/"))
    }
}

/// Minimal entry for a group: just test file requires (no setup, no runner).
fn generate_group_entry(test_files: &[PathBuf], mock_modules: &[String]) -> String {
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
        let display_name = file.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        entry.push_str(&format!(
            "if (globalThis.__HT && globalThis.__HT.resetMockModulePatches) globalThis.__HT.resetMockModulePatches();\nglobalThis.__currentTestFile = '{}';\ntry {{ require('{}'); }} catch(e) {{ if (globalThis.__HT) globalThis.__HT.registerCrash('{}', String(e && e.message || e)); }}\n",
            display_name, require_path, display_name
        ));
    }

    entry
}

/// Setup code eval'd before vendor: shims, mock placeholders, harness mocks.
fn generate_setup_code(
    test_files: &[PathBuf],
    mock_modules: &[String],
    cfg: &BundleConfig,
) -> String {
    let mut code = String::new();

    code.push_str("globalThis.__HT_mocks = globalThis.__HT_mocks || {};\n");
    code.push_str("globalThis.__HT_mocks['hermes-test'] = globalThis.__HT;\n");
    code.push_str("globalThis.__HT_mocks['@marcuzgabriel/hermes-test'] = globalThis.__HT;\n");

    // Built-in react-native shim (unless user provides custom one)
    let user_shim_modules: Vec<&str> = cfg.shims.iter().map(|(k, _)| k.as_str()).collect();
    if !user_shim_modules.contains(&"react-native") {
        code.push_str(&format!(
            "globalThis.__HT_mocks['react-native'] = (function() {{ var module = {{ exports: {{}} }}; {}; return module.exports; }})();\n",
            include_str!("../../../packages/hermes-test/src/shims/react-native.js")
        ));
    }

    // Pre-register mock module placeholders as live Proxies
    if !mock_modules.is_empty() {
        for path in mock_modules {
            code.push_str(&format!(
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

    code
}

/// Vendor entry: requires all discovered packages and registers on __HT_mocks.
fn generate_vendor_entry(packages: &[String], setup_code: &str) -> String {
    let mut entry = String::new();

    // Setup code runs first inside the vendor IIFE
    entry.push_str(setup_code);
    entry.push('\n');

    // React bootstrap — vendor bundles the real React
    entry.push_str(
        "try { var __htReact = require('react'); globalThis.__HT_React = __htReact; globalThis.__HT_mocks['react'] = __htReact; globalThis.IS_REACT_ACT_ENVIRONMENT = true; } catch(e) {}\n"
    );

    // Require each discovered package and register on __HT_mocks
    for pkg in packages {
        if pkg == "react" {
            continue; // Already handled above
        }
        entry.push_str(&format!(
            "try {{ globalThis.__HT_mocks['{}'] = require('{}'); }} catch(e) {{}}\n",
            pkg, pkg
        ));
    }

    entry
}

/// Extract package names from __require("...") calls in bundled code.
// Public wrappers for persistent watch mode
pub fn generate_group_entry_pub(test_files: &[PathBuf], mock_modules: &[String]) -> String {
    generate_group_entry(test_files, mock_modules)
}

pub fn find_esbuild_pub(project_root: &Path) -> Result<PathBuf, ()> {
    find_esbuild(project_root)
}

pub fn bundle_esbuild_with_config_pub(
    entry_file: &Path,
    esbuild_path: &Path,
    external_modules: &[String],
    cfg: &BundleConfig,
    packages_external: bool,
) -> Result<String, String> {
    bundle_esbuild_with_config(entry_file, esbuild_path, external_modules, cfg, packages_external)
}

fn extract_required_packages(code: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"__require\("([^"]+)"\)"#).unwrap();
    let mut packages = Vec::new();
    for cap in re.captures_iter(code) {
        let pkg = cap[1].to_string();
        if !packages.contains(&pkg) {
            packages.push(pkg);
        }
    }
    packages
}

