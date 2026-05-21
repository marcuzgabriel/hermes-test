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
pub struct BundleConfig {
    aliases: Vec<(String, String)>,
    externals: Vec<String>,
    shims: Vec<(String, String)>, // (module_name, file_path)
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
                base_dir.join(target).to_string_lossy().to_string()
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
    let mut config = BundleConfig { aliases: Vec::new(), externals: Vec::new(), shims: Vec::new(), root: None, split: false, test_match: None };

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
    if let Some(obj) = json_object_entries(&content, "shims") {
        for (key, val) in obj {
            let resolved = config_dir.join(&val).to_string_lossy().to_string();
            config.shims.push((key, resolved));
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
    // Skip aliases for packages that are externalized or mocked — esbuild aliases override
    // externals, so if a package is in both, the alias wins and the code gets bundled.
    // We want externals/mocks to win so they can be intercepted by __require at runtime.
    for (alias, target) in &cfg.aliases {
        let is_externalized = cfg.externals.iter().any(|e| {
            let e_base = e.trim_end_matches('*').trim_end_matches('/');
            alias == e_base || alias.starts_with(&format!("{e_base}/"))
                || (e.ends_with('*') && alias.starts_with(e_base))
        });
        // Also skip if any mockModule path starts with this alias
        let is_mocked = external_modules.iter().any(|m| {
            m == alias || m.starts_with(&format!("{alias}/"))
        });
        if !is_externalized && !is_mocked {
            cmd.arg(format!("--alias:{alias}={target}"));
        }
    }

    // Default externals: hermes-test (thin re-export from __HT), React Native (Flow syntax)
    for ext in &["hermes-test", "@marcuzgabriel/hermes-test", "react-native", "react-native/*", "@react-native/*"] {
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
    // Hoist __noop outside __require so it's created once, not per call
    let code = code.replacen(
        "throw Error('Dynamic require",
        "var __HT_noop = function() { return __HT_noop; }; throw Error('Dynamic require",
        1,
    );

    throw_re.replace(&code, |caps: &regex::Captures| {
        let v = &caps[1];
        // Proxy with get (property access) + construct (new) traps.
        // get: returns mock values or __HT_noop for unknown properties.
        // construct: allows `new ExternalModule()` to return a plain object (mock constructor).
        format!(
            r#"{{ var __r = globalThis.__HT_mocks || (globalThis.__HT_mocks = {{}}); var __k = {v}.replace(/^\.\//, ''); if (!__r[__k] && !__r[{v}]) __r[{v}] = {{}}; var __t = __r[{v}] || __r[__k] || __r['./' + __k] || {{}}; return typeof Proxy !== 'undefined' ? new Proxy(__t, {{ get: function(t,p) {{ if (p === Symbol.toPrimitive || p === 'then' || p === '$$typeof') return undefined; if (p === '__esModule') return true; var __rr = globalThis.__HT_mocks; var __m = __rr[{v}] || __rr[__k] || __rr['./' + __k]; if (p === 'default') {{ var __d = __m && __m['default']; return __d !== undefined ? __d : (__m || t); }} var val = __m ? __m[p] : t[p]; return val !== undefined ? val : __HT_noop; }}, apply: function() {{ return __HT_noop; }}, construct: function() {{ return {{}}; }} }}) : __t }}"#,
        )
    }).to_string()
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

/// Generate a synthetic entry file that imports the harness and all test files.
pub fn generate_entry(
    test_files: &[PathBuf],
    _harness_path: Option<&Path>,
    mock_modules: &[String],
    cfg: &BundleConfig,
) -> String {
    let mut entry = String::new();

    // Array.isArray polyfill is now in the harness polyfills.js (runs before bundle).
    // class-extends-Array fix is in fix_all_class_extends() (post-process step).

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
    // mockModule() in the test file will populate these objects with actual spies.
    // The __require shim returns these same objects, so the hook sees the mocks.
    if !mock_modules.is_empty() {
        entry.push_str("globalThis.__HT_mocks = globalThis.__HT_mocks || {};\n");
        for path in mock_modules {
            // Pre-create the registry entry as an empty object.
            // mockModule() will copy spy properties onto it later.
            entry.push_str(&format!(
                "globalThis.__HT_mocks['{}'] = globalThis.__HT_mocks['{}'] || {{}};\n",
                path, path
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
    for file in test_files {
        let path = file.to_string_lossy();
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
            "globalThis.__currentTestFile = '{}';\nrequire('{}');\n",
            display_name, require_path
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

/// Bundle with vendor/group splitting to avoid Hermes super-linear scaling.
/// Vendor bundle: all node_modules. Group bundles: only local test code.
pub fn bundle_split(
    test_files: &[PathBuf],
    project_root: &Path,
    mock_modules: &[String],
    cfg: &BundleConfig,
) -> Result<SplitBundle, String> {
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

    // Step 1: Bundle each group with --packages=external (fast, local code only)
    for (i, chunk) in test_files.chunks(group_size).enumerate() {
        let entry = generate_group_entry(chunk, mock_modules);
        let entry_path = project_root.join(format!(".hermes-test-group-{i}.js"));
        std::fs::write(&entry_path, &entry)
            .map_err(|e| format!("Failed to write group entry: {e}"))?;

        let code = bundle_esbuild_with_config(
            &entry_path, &esbuild_path, mock_modules, cfg, true,
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

        let vendor_code = bundle_esbuild_with_config(
            &vendor_entry_path, &esbuild_path, mock_modules, cfg, false,
        )?;
        let _ = std::fs::remove_file(&vendor_entry_path);
        vendor_code
    };

    Ok(SplitBundle { vendor, groups: group_bundles })
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

    // Re-register mock placeholders (needed for __require shim in each group IIFE)
    if !mock_modules.is_empty() {
        for path in mock_modules {
            entry.push_str(&format!(
                "globalThis.__HT_mocks['{}'] = globalThis.__HT_mocks['{}'] || {{}};\n",
                path, path
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
            "globalThis.__currentTestFile = '{}';\nrequire('{}');\n",
            display_name, require_path
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

    // Pre-register mock module placeholders
    if !mock_modules.is_empty() {
        for path in mock_modules {
            code.push_str(&format!(
                "globalThis.__HT_mocks['{}'] = globalThis.__HT_mocks['{}'] || {{}};\n",
                path, path
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
