use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// oxc — available for future AST transforms when needed
// use oxc::{allocator::Allocator, codegen::Codegen, parser::Parser,
//     semantic::SemanticBuilder, span::SourceType, transformer::{TransformOptions, Transformer}};

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
struct BundleConfig {
    aliases: Vec<(String, String)>,
    externals: Vec<String>,
    root: Option<PathBuf>,
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

/// Read "paths" from a tsconfig.json file and add as esbuild aliases.
fn read_tsconfig_paths(base_dir: &Path, tsconfig_path: &Path, aliases: &mut Vec<(String, String)>) {
    let Ok(content) = std::fs::read_to_string(tsconfig_path) else { return };
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
fn read_config(project_root: &Path) -> BundleConfig {
    let mut config = BundleConfig { aliases: Vec::new(), externals: Vec::new(), root: None };

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

    // "external" — modules to externalize
    if let Some(items) = json_string_array(&content, "external") {
        config.externals = items;
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

    let mut cmd = Command::new(esbuild_path);
    cmd.arg(entry_file)
        .arg("--bundle")
        .arg("--format=iife")
        .arg("--target=es2020")
        .arg("--supported:async-await=false")
        .arg("--define:process.env.NODE_ENV=\"test\"")
        .arg("--define:global=globalThis")
        .arg("--loader:.js=jsx")
        .arg("--loader:.png=empty")
        .arg("--loader:.jpg=empty")
        .arg("--loader:.gif=empty")
        .arg("--loader:.svg=empty")
        .arg("--external:console");

    // Monorepo: if config specifies root, add its node_modules to NODE_PATH
    if let Some(ref root) = cfg.root {
        let nm = root.join("node_modules");
        if nm.is_dir() {
            cmd.env("NODE_PATH", nm.to_string_lossy().to_string());
        }
    }

    // Path aliases from tsconfig (resolved by config)
    for (alias, target) in &cfg.aliases {
        cmd.arg(format!("--alias:{alias}={target}"));
    }

    // Default externals: React Native uses Flow syntax
    for ext in &["react-native", "react-native/*", "@react-native/*"] {
        cmd.arg(format!("--external:{ext}"));
    }

    // Config externals
    for ext in &cfg.externals {
        cmd.arg(format!("--external:{ext}"));
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
        || code.contains("Dynamic require of");
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


/// Patch esbuild's __require to route externalized modules through __mockRegistry.
/// Simple approach: replace the "throw" line inside __require with a registry lookup.
fn inject_mock_require_shim(code: &str) -> String {
    // esbuild's __require contains this exact throw statement for unsupported externals:
    //   throw Error('Dynamic require of "' + x + '" is not supported');
    // Replace it with a __mockRegistry lookup.
    code.replacen(
        r#"throw Error('Dynamic require of "' + x + '" is not supported')"#,
        r#"{ var __r = globalThis.__mockRegistry; if (__r) { if (__r[x]) return __r[x]; var __s = x.replace(/^\.\//, ''); if (__r[__s]) return __r[__s]; if (__r['./' + __s]) return __r['./' + __s]; } return {} }"#,
        1,
    )
}

/// Fix ALL `class Foo extends Array { ... }` patterns in bundled code.
///
/// Hermes bug: `super()` in derived Array classes compiles to `Array.call(this, ...args)`
/// which discards the return value. The instance is a plain JSObject, not a JSArray,
/// breaking Array.isArray, concat, splice, spread, etc.
///
/// Fix: replace each class-extends-Array with a Reflect.construct-based function.
/// This is the same pattern used by:
///   - Babel: `_wrapNativeSuper` + `_construct` helpers
///     https://github.com/babel/babel/blob/main/packages/babel-helpers/src/helpers/wrapNativeSuper.ts
///   - SWC: `_wrap_native_super` + `_construct` helpers
///     https://unpkg.com/@swc/helpers/esm/_wrap_native_super.js
///   - WebReflection's original design:
///     https://github.com/nicolo-ribaudo/babel/blob/main/packages/babel-helpers/src/helpers/wrapNativeSuper.ts
///
/// All three produce: `Reflect.construct(Array, args, DerivedClass)` which creates
/// a real JSArray with the subclass prototype chain.
fn fix_class_extends_array(code: &str) -> String {
    // Match: `Name = class [_OptionalInternalName] extends Array {`
    let re = regex::Regex::new(
        r"(\w+)\s*=\s*class\s+(_?\w+)\s+extends\s+Array\s*\{"
    ).unwrap();

    let mut result = code.to_string();
    // Process matches in reverse to preserve offsets
    let matches: Vec<_> = re.captures_iter(code).collect();

    for cap in matches.iter().rev() {
        let full = cap.get(0).unwrap();
        let var_name = &cap[1];
        let internal_name = &cap[2];

        // Find the matching closing `};` for the class
        let mut depth = 1;
        let mut class_end = full.end();
        for (i, ch) in code[full.end()..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        class_end = full.end() + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        // Include trailing semicolon
        if class_end < code.len() && code.as_bytes()[class_end] == b';' {
            class_end += 1;
        }

        // Extract class body to find instance methods (we need to fix super. references)
        let class_body = &code[full.end()..class_end];

        // Build replacement: function-based constructor + prototype methods
        // Extract methods from class body, transform super. → Array.prototype.
        let mut methods = Vec::new();
        let mut pos = 0;
        let body_bytes = class_body.as_bytes();

        // Simple state machine to find top-level method definitions
        while pos < class_body.len() {
            // Skip to next method (look for identifier followed by `(`)
            // Methods are at depth 0 in the class body (depth starts at 0 since we excluded the opening {)
            let remaining = &class_body[pos..];

            // Find `constructor(` or `methodName(` or `static get [Symbol.species](` etc.
            if remaining.starts_with("constructor") {
                // Skip constructor — we replace it with Reflect.construct
                // Find its closing brace
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

            // Check for method patterns at top level
            let is_static = remaining.starts_with("static ");
            let method_remaining = if is_static { &remaining[7..] } else { remaining };

            // Look for `get [Symbol.species]()` pattern
            if is_static && method_remaining.starts_with("get [Symbol.species]") {
                // Skip — we'll add our own Symbol.species
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

            // Match: `methodName(...args) {`
            let method_name_re = regex::Regex::new(r"^(\w+)\s*\(([^)]*)\)\s*\{").unwrap();
            if let Some(m) = method_name_re.captures(method_remaining) {
                let name = m.get(1).unwrap().as_str();
                let params = m.get(2).unwrap().as_str();
                let brace_end = m.get(0).unwrap().end();
                let abs_body_start = pos + (if is_static { 7 } else { 0 }) + brace_end;

                // Find matching closing brace
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
                // Fix super.X → Array.prototype.X
                let body = body.replace("super.", "Array.prototype.");
                // Fix internal class name references: new _Tuple → new Tuple
                let body = body.replace(&format!("new {internal_name}"), &format!("new {var_name}"));

                let target = if is_static { var_name.to_string() } else { format!("{var_name}.prototype") };
                methods.push(format!("{target}.{name} = function({params}) {{{body}}};"));

                pos = body_end + 1;
                continue;
            }

            pos += 1;
        }

        let methods_str = methods.join("\n");

        let replacement = format!(
            r#"{var_name} = (function() {{
  function {var_name}() {{
    return Reflect.construct(Array, Array.prototype.slice.call(arguments), new.target || {var_name});
  }}
  {var_name}.prototype = Object.create(Array.prototype, {{
    constructor: {{ value: {var_name}, writable: true, configurable: true }}
  }});
  Object.setPrototypeOf({var_name}, Array);
  Object.defineProperty({var_name}, Symbol.species, {{ get: function() {{ return {var_name}; }} }});
  {methods_str}
  return {var_name};
}})();"#
        );

        result = format!("{}{}{}", &result[..full.start()], replacement, &result[class_end..]);
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

    // Patch 3: Fix ALL `class extends Array` patterns.
    // Hermes bug: super() in derived class compiles to Array.call(this, ...args) and discards
    // the return value. The instance is a plain JSObject (not JSArray), breaking Array.isArray,
    // concat, splice, spread, etc. Fix: replace with Reflect.construct-based function that
    // creates real JSArray instances with the correct prototype chain.
    let code = fix_class_extends_array(&code);

    let code_str = code;

    if code_str.len() == original_len {
        eprintln!("WARNING: esbuild patches did not match — Hermes for-let-of bug may cause failures");
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

/// Find all test files matching *.test.ts, *.test.tsx, *.test.js, *.test.jsx
pub fn find_test_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk_dir(root, &mut files);
    files.sort();
    files
}

fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>) {
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
            walk_dir(&path, files);
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
) -> String {
    let mut entry = String::new();

    // Array.isArray polyfill is now in the harness polyfills.js (runs before bundle).
    // class-extends-Array fix is in fix_class_extends_array() (post-process step).

    // Pre-register mock module placeholders BEFORE anything loads.
    // mockModule() in the test file will populate these objects with actual spies.
    // The __require shim returns these same objects, so the hook sees the mocks.
    if !mock_modules.is_empty() {
        entry.push_str("globalThis.__mockRegistry = globalThis.__mockRegistry || {};\n");
        for path in mock_modules {
            // Pre-create the registry entry as an empty object.
            // mockModule() will copy spy properties onto it later.
            entry.push_str(&format!(
                "globalThis.__mockRegistry['{}'] = globalThis.__mockRegistry['{}'] || {{}};\n",
                path, path
            ));
        }
    }

    // Only bootstrap React if test files actually need it (saves ~40ms for pure tests)
    if needs_react(test_files) {
        entry.push_str(
            r#"try {
  globalThis.__React = require('react');
  globalThis.__ReactTestRenderer = require('react-test-renderer');
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
var __results = globalThis.__metroTest.runTests();
globalThis.__metroTestResults = JSON.stringify({
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
