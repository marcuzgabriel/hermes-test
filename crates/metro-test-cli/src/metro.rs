use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

fn bundle_esbuild(
    entry_file: &Path,
    esbuild_path: &Path,
    external_modules: &[String],
) -> Result<String, String> {
    let mut cmd = Command::new(esbuild_path);
    cmd.arg(entry_file)
        .arg("--bundle")
        .arg("--format=iife")
        .arg("--target=es2020")
        .arg("--supported:async-await=false")
        .arg("--define:process.env.NODE_ENV=\"test\"")
        .arg("--define:global=globalThis");

    // Externalize mocked modules so they resolve through __require → __mockRegistry
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

    let mut code = patch_esbuild_for_hermes(&code);

    // If there are external mocked modules, inject the __require shim
    if !external_modules.is_empty() {
        code = inject_mock_require_shim(&code);
    }

    // No SWC needed — VM Runtime::run() handles classes correctly,
    // and Array.isArray polyfill handles class-extends-Array.

    Ok(code)
}
/// Pre-compile JS to Hermes bytecode via hermesc.
/// Works around JSI evaluateJavaScript bug where ES6 class transformation
/// fails for large files. hermesc compiles correctly via a different code path.
/// Returns raw bytecode bytes that evaluateJavaScript can load directly.
/// Pre-compile JS to Hermes bytecode via hermesc for large bundles with classes.
/// Returns Some(bytecode) if compilation succeeded, None to fall back to source eval.
fn transpile_with_swc(code: &str, context_path: &Path) -> Result<String, String> {
    if !code.contains("node_modules/immer/") && !code.contains("node_modules/@reduxjs/toolkit/") {
        return Ok(code.to_string());
    }
    let swc = find_swc(context_path);
    let swc = match swc {
        Some(p) => p,
        None => return Ok(code.to_string()),
    };
    let swcrc = context_path.parent().unwrap_or(context_path).join(".metro-test-swcrc.json");
    let _ = std::fs::write(&swcrc, r#"{"jsc":{"target":"es5"},"module":{"type":"es6"}}"#);

    let mut child = Command::new(&swc)
        .arg("--filename").arg("bundle.js")
        .arg("--config-file").arg(&swcrc)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| format!("SWC: {e}"))?;

    use std::io::Write;
    child.stdin.as_mut().unwrap().write_all(code.as_bytes()).map_err(|e| format!("SWC stdin: {e}"))?;
    drop(child.stdin.take());
    let out = child.wait_with_output().map_err(|e| format!("SWC: {e}"))?;
    let _ = std::fs::remove_file(&swcrc);

    if !out.status.success() {
        return Ok(code.to_string());
    }
    String::from_utf8(out.stdout).map_err(|e| format!("SWC: {e}"))
}

fn find_swc(ctx: &Path) -> Option<PathBuf> {
    let local = ctx.join("node_modules/.bin/swc");
    if local.exists() { return Some(local); }
    let mut dir = ctx.parent();
    while let Some(d) = dir {
        let c = d.join("node_modules/.bin/swc");
        if c.exists() { return Some(c); }
        dir = d.parent();
    }
    if Command::new("swc").arg("--version").output().is_ok() { return Some(PathBuf::from("swc")); }
    None
}

pub fn compile_to_bytecode(code: &str, context_path: &Path) -> Option<Vec<u8>> {
    // Only needed for large bundles with class syntax
    if code.len() < 60_000 || !code.contains("class ") {
        return None;
    }

    let hermesc = find_hermesc()?;

    let src_path = context_path.parent().unwrap_or(context_path).join(".metro-test-src.js");
    let hbc_path = context_path.parent().unwrap_or(context_path).join(".metro-test-src.hbc");
    std::fs::write(&src_path, code).ok()?;

    let output = Command::new(&hermesc)
        .arg("-Xes6-class")
        .arg("-emit-binary")
        .arg("-out")
        .arg(&hbc_path)
        .arg(&src_path)
        .stderr(Stdio::piped())
        .output()
        .ok()?;

    let _ = std::fs::remove_file(&src_path);

    if !output.status.success() {
        let _ = std::fs::remove_file(&hbc_path);
        return None;
    }

    let bytecode = std::fs::read(&hbc_path).ok()?;
    let _ = std::fs::remove_file(&hbc_path);
    Some(bytecode)
}

fn find_hermesc() -> Option<PathBuf> {
    // Check the vendor build directory
    let vendor_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .join("vendor/hermes/build/bin/hermesc");
    if vendor_path.exists() {
        return Some(vendor_path);
    }
    // Global
    if Command::new("hermesc").arg("--version").output().is_ok() {
        return Some(PathBuf::from("hermesc"));
    }
    None
}

/// Rewrite ES6 class expressions to ES5 function constructors.
/// Hermes has a 64KB bytecode compiler bug: class expressions return `undefined`
/// when compilation unit > ~64KB. This rewrites them to functions (zero overhead).
///
/// Handles: `Foo = class { constructor(x) { this.y = x; } };`
///       → `Foo = function(x) { this.y = x; };`
///
/// And: `Foo = class extends Bar { constructor() { super(); ... } };`
///    → `Foo = function() { Bar.call(this); ... }; Object.setPrototypeOf(Foo.prototype, Bar.prototype);`
fn desugar_classes(code: &str) -> String {
    // Only process if there are class expressions (not declarations)
    if !code.contains("= class ") && !code.contains("= class{") {
        return code.to_string();
    }

    let re = regex::Regex::new(
        r"(\w+)\s*=\s*class\s*(?:(_?\w+)\s+)?(?:extends\s+(\w+)\s*)?\{"
    ).unwrap();

    let mut result = code.to_string();
    let mut offset: isize = 0;

    let matches: Vec<_> = re.captures_iter(code).collect();

    for cap in &matches {
        let full_match = cap.get(0).unwrap();
        let var_name = &cap[1];
        let extends_class = cap.get(3).map(|m| m.as_str());

        // Find the constructor inside this class
        let class_start = full_match.end();
        let ctor_re = regex::Regex::new(r"constructor\s*\(([^)]*)\)\s*\{").unwrap();

        if let Some(ctor_match) = ctor_re.find(&code[class_start..]) {
            let ctor_caps = ctor_re.captures(&code[class_start..]).unwrap();
            let params = ctor_caps.get(1).map_or("", |m| m.as_str());
            let ctor_body_start = class_start + ctor_match.end();

            // Find matching closing brace for constructor body
            let mut depth = 1;
            let mut ctor_body_end = ctor_body_start;
            for (i, ch) in code[ctor_body_start..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            ctor_body_end = ctor_body_start + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            let mut ctor_body = code[ctor_body_start..ctor_body_end].to_string();

            // Handle super() calls for extends
            if let Some(parent) = extends_class {
                ctor_body = ctor_body.replace("super()", &format!("{parent}.call(this)"));
                // Handle super(...args)
                let super_re = regex::Regex::new(r"super\(([^)]+)\)").unwrap();
                ctor_body = super_re.replace_all(&ctor_body, &format!("{parent}.call(this, $1)")).to_string();
            }

            // Find the end of the class (closing brace + semicolon)
            let mut class_depth = 1;
            let mut class_end = class_start;
            for (i, ch) in code[class_start..].char_indices() {
                match ch {
                    '{' => class_depth += 1,
                    '}' => {
                        class_depth -= 1;
                        if class_depth == 0 {
                            class_end = class_start + i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            // Build the replacement
            let replacement = if let Some(parent) = extends_class {
                format!(
                    "{var_name} = function({params}) {{{ctor_body}}};\n      Object.setPrototypeOf({var_name}.prototype, {parent}.prototype)",
                )
            } else {
                format!("{var_name} = function({params}) {{{ctor_body}}}")
            };

            // Apply the replacement with offset tracking
            let replace_start = (full_match.start() as isize + offset) as usize;
            let replace_end = (class_end as isize + offset) as usize;

            result = format!(
                "{}{}{}",
                &result[..replace_start],
                replacement,
                &result[replace_end..]
            );

            offset += replacement.len() as isize - (class_end - full_match.start()) as isize;
        }
    }

    result
}

/// Patch esbuild's __require to route externalized modules through __mockRegistry.
/// Simple approach: replace the "throw" line inside __require with a registry lookup.
fn inject_mock_require_shim(code: &str) -> String {
    // esbuild's __require contains this exact throw statement for unsupported externals:
    //   throw Error('Dynamic require of "' + x + '" is not supported');
    // Replace it with a __mockRegistry lookup.
    code.replacen(
        r#"throw Error('Dynamic require of "' + x + '" is not supported')"#,
        r#"{ var __r = globalThis.__mockRegistry; if (__r) { if (__r[x]) return __r[x]; var __s = x.replace(/^\.\//, ''); if (__r[__s]) return __r[__s]; if (__r['./' + __s]) return __r['./' + __s]; } throw Error('No mock registered for "' + x + '"') }"#,
        1,
    )
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

    // Patch 3: Replace RTK's Tuple (class extends Array) with a plain Array subtype.
    // Hermes's class-extends-Array is broken: Array.isArray returns false, concat
    // corrupts elements, Symbol.species doesn't work. Replace the entire class with
    // a function-based approach that creates real arrays with extra methods.
    let tuple_re = regex::Regex::new(
        r"(?s)Tuple = class _Tuple extends Array \{.*?\n\s*\};"
    ).unwrap();
    let code = tuple_re.replace(&code, r#"Tuple = function Tuple() {
        var arr = Array.prototype.slice.call(arguments);
        arr.concat = function() {
          var r = arr.slice();
          for (var i = 0; i < arguments.length; i++) {
            var a = arguments[i];
            if (Array.isArray(a)) { for (var j = 0; j < a.length; j++) r.push(a[j]); }
            else r.push(a);
          }
          return new Tuple(r);
        };
        arr.prepend = function() {
          var args = Array.prototype.slice.call(arguments);
          if (args.length === 1 && Array.isArray(args[0])) {
            return new Tuple(args[0].concat(arr));
          }
          return new Tuple(args.concat(arr));
        };
        if (arguments.length === 1 && Array.isArray(arguments[0])) {
          arr = arguments[0].slice();
          arr.concat = Tuple.prototype.concat;
          arr.prepend = Tuple.prototype.prepend;
        }
        return arr;
      };
      Tuple.prototype.concat = function() {
        var r = Array.prototype.slice.call(this);
        for (var i = 0; i < arguments.length; i++) {
          var a = arguments[i];
          if (Array.isArray(a)) { for (var j = 0; j < a.length; j++) r.push(a[j]); }
          else r.push(a);
        }
        r.concat = Tuple.prototype.concat;
        r.prepend = Tuple.prototype.prepend;
        return r;
      };
      Tuple.prototype.prepend = function() {
        var args = Array.prototype.slice.call(arguments);
        var self = Array.prototype.slice.call(this);
        if (args.length === 1 && Array.isArray(args[0])) {
          var r = args[0].concat(self);
        } else {
          var r = args.concat(self);
        }
        r.concat = Tuple.prototype.concat;
        r.prepend = Tuple.prototype.prepend;
        return r;
      };"#).to_string();

    if code.len() == original_len {
        eprintln!("WARNING: esbuild patches did not match — Hermes for-let-of bug may cause failures");
    }

    code
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
                || name_str == ".metro-test-cache"
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

/// Scan test files for mockModule('path', ...) calls and return the module paths.
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

    // Hermes class-extends-Array is fundamentally broken (isArray, concat, Symbol.species).
    // Instead of polyfilling each method, replace RTK's Tuple class with a plain Array wrapper
    // that just adds concat/prepend methods. Injected into the bundle IIFE.
    entry.push_str(r#"(function(){
  var o=Array.isArray;
  Array.isArray=function(a){
    if(o(a))return true;
    if(a&&typeof a==='object'&&typeof a.length==='number'){
      var p=Object.getPrototypeOf(a);
      while(p&&p!==Object.prototype){if(p===Array.prototype)return true;p=Object.getPrototypeOf(p)}
    }
    return false;
  };
})();
"#);

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

    // Import test files
    for file in test_files {
        let path = file.to_string_lossy();
        let require_path = if path.starts_with('/') || path.starts_with("./") {
            path.to_string()
        } else {
            format!("./{path}")
        };
        entry.push_str(&format!("require('{}');\n", require_path));
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

    let metafile_path = project_root.join(".metro-test-meta.json");
    let outfile_path = project_root.join(".metro-test-bundle.js");

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

    // SWC transpile for bundles importing immer/RTK
    code = transpile_with_swc(&code, entry_file)?;

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
