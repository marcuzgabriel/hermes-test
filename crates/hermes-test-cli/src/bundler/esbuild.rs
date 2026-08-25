use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::config::BundleConfig;
use super::patches::{patch_esbuild_for_hermes, inject_mock_require_shim, hoist_mock_modules};

// Historical note: an SWC/AST class transform was evaluated and rejected
// (thread-locals, helper injection, full re-emit breaking other patches), and a
// regex-based class downleveler served for a year. Since the V1 engine (July
// 2026) classes are engine-native and the downleveler is deleted — the whole
// debate is moot. See fixtures/class-extends/ for the engine-conformance guards.

/// Bundle result that optionally includes a source map.
pub struct BundleResult {
    pub code: String,
    pub source_map: Option<sourcemap::SourceMap>,
    /// Number of lines in the bundle before patches were applied.
    pub pre_patch_line_count: u32,
}

/// Find react-reconciler by walking up from the test files' directory.
/// Prefers the location closest to where the user's React is installed,
/// so the reconciler version matches the user's React version.
fn find_react_reconciler(project_dir: &Path, config_root: Option<&Path>, test_files: &[PathBuf]) -> Option<PathBuf> {
    let target = "react-reconciler";

    // 1. Find from test files' directory (closest to user's React)
    if let Some(first_test) = test_files.first() {
        let mut dir = first_test.parent();
        while let Some(d) = dir {
            let candidate = d.join("node_modules").join(target);
            if candidate.is_dir() {
                return Some(candidate);
            }
            dir = d.parent();
        }
    }

    // 2. Walk up from project dir (entry file location)
    let mut dir = Some(project_dir);
    while let Some(d) = dir {
        let candidate = d.join("node_modules").join(target);
        if candidate.is_dir() {
            return Some(candidate);
        }
        dir = d.parent();
    }

    // 3. Check hermes-test's own node_modules
    if let Some(root) = config_root {
        for sub in &["packages/hermes-test/node_modules", "node_modules/hermes-test/node_modules"] {
            let candidate = root.join(sub).join(target);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }

    None
}

/// The `@esbuild/<platform>` package that carries the native binary for this build.
fn esbuild_platform_pkg() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("@esbuild/darwin-arm64"),
        ("macos", "x86_64") => Some("@esbuild/darwin-x64"),
        ("linux", "x86_64") => Some("@esbuild/linux-x64"),
        ("linux", "aarch64") => Some("@esbuild/linux-arm64"),
        _ => None,
    }
}

/// True if `path` starts with a Mach-O or ELF magic number. esbuild's JS shim
/// (`esbuild/bin/esbuild`) starts with `#!/usr/bin/env node` and fails this.
fn is_native_executable(path: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let mut magic = [0u8; 4];
    if std::io::Read::read_exact(&mut f, &mut magic).is_err() {
        return false;
    }
    matches!(
        magic,
        [0x7F, b'E', b'L', b'F']
            | [0xFE, 0xED, 0xFA, 0xCE]
            | [0xFE, 0xED, 0xFA, 0xCF]
            | [0xCE, 0xFA, 0xED, 0xFE]
            | [0xCF, 0xFA, 0xED, 0xFE]
            | [0xCA, 0xFE, 0xBA, 0xBE]
    )
}

/// Resolve the native esbuild binary under `node_modules`, or `None` if only a
/// JS shim (or nothing) is there. Returns the canonical path so callers never
/// hand a symlink to `ESBUILD_BINARY_PATH`.
fn native_esbuild_in(node_modules: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(pkg) = esbuild_platform_pkg() {
        candidates.push(node_modules.join(pkg).join("bin/esbuild"));
    }
    candidates.push(node_modules.join(".bin/esbuild"));
    candidates.push(node_modules.join("esbuild/bin/esbuild"));
    candidates.into_iter().find_map(|c| {
        let real = std::fs::canonicalize(&c).ok()?;
        is_native_executable(&real).then_some(real)
    })
}

/// Locate the native esbuild binary. Refuses esbuild's JS shim: the plugin
/// resolver exports the result as `ESBUILD_BINARY_PATH`, and a shim that sees
/// that variable pointing at itself re-execs forever (`--service --ping`
/// fork loop) instead of failing. Broken installs now fail with a message.
pub fn find_esbuild(project_root: &Path) -> Result<PathBuf, String> {
    let mut shim_seen: Option<PathBuf> = None;
    let mut dir = Some(project_root);
    while let Some(d) = dir {
        let nm = d.join("node_modules");
        if let Some(bin) = native_esbuild_in(&nm) {
            return Ok(bin);
        }
        let shim = nm.join(".bin/esbuild");
        if shim_seen.is_none() && shim.exists() {
            shim_seen = Some(shim);
        }
        dir = d.parent();
    }

    if let Some(bin) = native_esbuild_on_path() {
        return Ok(bin);
    }

    Err(match shim_seen {
        Some(shim) => format!(
            "esbuild native binary not found: {} is esbuild's JS shim and {} is missing. \
             The install was incomplete (optional dependency not downloaded, or node/bun \
             architecture mismatch). Run: rm -rf node_modules && bun install",
            shim.display(),
            esbuild_platform_pkg().unwrap_or("the platform package"),
        ),
        None => "esbuild not found. Install it: bun add -d esbuild".to_string(),
    })
}

fn native_esbuild_on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|p| {
        let real = std::fs::canonicalize(p.join("esbuild")).ok()?;
        is_native_executable(&real).then_some(real)
    })
}

/// Extract inline source map from bundle, returning (code_without_map, parsed_map).
fn extract_inline_sourcemap(code: &str) -> (String, Option<sourcemap::SourceMap>) {
    let marker = "//# sourceMappingURL=data:application/json;base64,";
    if let Some(pos) = code.rfind(marker) {
        let b64 = code[pos + marker.len()..].trim();
        let clean_code = code[..pos].trim_end().to_string();
        // Decode base64 source map
        if let Ok(decoded) = base64_decode(b64) {
            if let Ok(sm) = sourcemap::SourceMap::from_reader(&decoded[..]) {
                return (clean_code, Some(sm));
            }
        }
        (clean_code, None)
    } else {
        (code.to_string(), None)
    }
}

/// Simple base64 decoder (no external crate needed).
fn base64_decode(input: &str) -> Result<Vec<u8>, ()> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in input.as_bytes() {
        if b == b'=' || b == b'\n' || b == b'\r' { continue; }
        let val = TABLE.iter().position(|&c| c == b).ok_or(())? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 { bits -= 8; out.push((buf >> bits) as u8); buf &= (1 << bits) - 1; }
    }
    Ok(out)
}

/// Assemble the esbuild CLI args (everything after the entry path) and the
/// NODE_PATH env value. Shared by the CLI invocation and the plugin bundler
/// (which parses these exact flag strings into JS API options) so the two
/// modes can never drift apart on flags.
pub(crate) fn assemble_esbuild_args(
    entry_file: &Path,
    external_modules: &[String],
    cfg: &BundleConfig,
    packages_external: bool,
    sourcemap_inline: bool,
) -> (Vec<String>, Option<String>) {
    let mut args: Vec<String> = vec![
        "--bundle".into(),
        "--format=iife".into(),
        "--target=es2020".into(),
        // No --minify — our Hermes compat patches match unminified esbuild output patterns.
        "--supported:async-await=false".into(),
        "--define:process.env.NODE_ENV=\"test\"".into(),
        "--define:process.env.JEST_WORKER_ID=\"1\"".into(),
        "--define:global=globalThis".into(),
        "--jsx=automatic".into(),
        "--loader:.js=jsx".into(),
        "--loader:.png=empty".into(),
        "--loader:.jpg=empty".into(),
        "--loader:.gif=empty".into(),
        "--loader:.svg=empty".into(),
        // Every other non-code extension (fonts, media, …) is handled by the asset catch-all in
        // plugin_build.cjs — no list to maintain here.
    ]; // console is a global in Hermes, not externalized

    if sourcemap_inline {
        args.push("--sourcemap=inline".into());
    }

    if packages_external {
        args.push("--packages=external".into());
    }

    // Monorepo: add node_modules paths for resolution.
    let mut node_path_env: Option<String> = None;
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
            node_path_env = Some(node_paths.join(":"));
        }
    }

    // Path aliases from tsconfig (resolved by config).
    // Skip aliases when:
    // 1. The package is in the externals list (native modules)
    // 2. Any mock() path is a sub-path of this alias — esbuild aliases run BEFORE
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
            args.push(format!("--alias:{alias}={target}"));
        }
    }

    // Externalize hermes-test itself (thin re-export from __HT runtime)
    args.push("--external:hermes-test".into());
    // Alias hermes-test/store to the actual file so it gets BUNDLED (not externalized).
    // esbuild aliases run before external checks, so this resolves before the external match.
    {
        let store_paths = [
            entry_file.parent().unwrap_or(std::path::Path::new(".")).join("node_modules/hermes-test/src/store.ts"),
        ];
        for sp in &store_paths {
            if sp.exists() {
                args.push(format!("--alias:hermes-test/store={}", sp.to_string_lossy()));
                break;
            }
        }
        if let Some(ref root) = cfg.root {
            let root_store = root.join("node_modules/hermes-test/src/store.ts");
            if root_store.exists() {
                args.push(format!("--alias:hermes-test/store={}", root_store.to_string_lossy()));
            }
        }
    }
    // Alias react-reconciler to its resolved path so esbuild can find it
    // regardless of package manager layout (bun, pnpm, yarn workspaces).
    // The reconciler is bundled into the test bundle alongside the user's React,
    // ensuring version compatibility.
    // We read test file paths from the entry to find the closest react-reconciler.
    {
        // Extract test file paths from the entry content to locate the closest react-reconciler
        let entry_content = std::fs::read_to_string(entry_file).unwrap_or_default();
        let test_files: Vec<PathBuf> = entry_content.lines()
            .filter_map(|l| {
                // Match: require('./path/to/file.test.tsx')
                if let Some(start) = l.find("require('") {
                    let rest = &l[start + 9..];
                    if let Some(end) = rest.find("')") {
                        let path = &rest[..end];
                        if path.contains(".test.") {
                            return Some(PathBuf::from(path));
                        }
                    }
                }
                None
            })
            .collect();
        if let Some(rec_path) = find_react_reconciler(
            entry_file.parent().unwrap_or(Path::new(".")),
            cfg.root.as_deref(),
            &test_files,
        ) {
            args.push(format!("--alias:react-reconciler={}", rec_path.to_string_lossy()));
            let constants = rec_path.join("constants.js");
            if constants.exists() {
                args.push(format!("--alias:react-reconciler/constants={}", constants.to_string_lossy()));
            }
        }
    }

    // react-native uses Flow syntax that esbuild can't parse — always external.
    // All other native packages are auto-detected or user-configured.
    for ext in &["react-native", "react-native/*"] {
        args.push(format!("--external:{ext}"));
    }

    // Config externals — for wildcard patterns like `pkg/*`, also externalize `pkg` itself
    for ext in &cfg.externals {
        args.push(format!("--external:{ext}"));
        if ext.ends_with("/*") {
            // Also externalize bare import: `@foo/bar/*` → also `@foo/bar`
            let base = &ext[..ext.len() - 2];
            args.push(format!("--external:{base}"));
        } else if !ext.ends_with('*') {
            args.push(format!("--external:{ext}/*"));
        }
    }

    // User file shims ("shims": {"pkg": "./path/to/shim.js"}) replace a module at runtime
    // via __HT_mocks — so the real package must NOT be bundled, otherwise esbuild inlines it
    // (and chokes on its assets / Flow syntax) and the shim silently never applies.
    for (module_name, _) in &cfg.shims {
        if cfg.externals.iter().any(|e| e == module_name) {
            continue;
        }
        // Exact module only: a shim for `pkg` must not swallow a real `pkg/sub` import.
        args.push(format!("--external:{module_name}"));
    }

    // Mock module externals
    for ext in external_modules {
        args.push(format!("--external:{ext}"));
    }

    (args, node_path_env)
}

/// JS build script for plugin-resolver mode, embedded in the binary.
const PLUGIN_BUILD_CJS: &str = include_str!("plugin_build.cjs");

/// Locate esbuild's JS API entry (lib/main.js) — same search order as the binary.
fn find_esbuild_lib(project_root: &Path) -> Option<PathBuf> {
    let local = project_root.join("node_modules/esbuild/lib/main.js");
    if local.exists() {
        return Some(local);
    }
    let mut dir = project_root.parent();
    while let Some(d) = dir {
        let candidate = d.join("node_modules/esbuild/lib/main.js");
        if candidate.exists() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

/// JS runtime for the plugin build script. Prefers the runtime already executing
/// the bin launcher (HT_JS_RUNTIME = process.execPath, set by bin/hermes-test.js),
/// then bun, then node.
fn find_js_runtime() -> Option<String> {
    if let Ok(rt) = std::env::var("HT_JS_RUNTIME") {
        if !rt.is_empty() && Path::new(&rt).exists() {
            return Some(rt);
        }
    }
    for candidate in ["bun", "node"] {
        if Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some(candidate.to_string());
        }
    }
    None
}

fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\.+*?()|[]{}^$-".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Bundle via esbuild's JS API with the ht-mocks onResolve plugin (the only resolver).
/// `file_wrappers` maps resolved absolute target paths (relative + alias mocks) and
/// `text_wrappers` import-specifier texts (alias mocks, package mocks, barrel
/// ancestors) to their generated wrapper files.
/// Flags are assembled by the same function as CLI mode; the build script parses
/// those exact strings, so flag behavior cannot drift between modes.
pub fn bundle_via_plugin_with_config(
    entry_file: &Path,
    project_root: &Path,
    external_modules: &[String],
    cfg: &BundleConfig,
    file_wrappers: &[(String, String)],
    text_wrappers: &[(String, String)],
) -> Result<String, String> {
    bundle_via_plugin_inner(entry_file, project_root, external_modules, cfg, file_wrappers, text_wrappers, false, false)
}

/// Plugin bundling with inline source map for coverage — mirrors
/// bundle_esbuild_with_sourcemap: raw bundle, extract map, then patches with
/// line-delta tracking.
pub fn bundle_via_plugin_with_sourcemap(
    entry_file: &Path,
    project_root: &Path,
    external_modules: &[String],
    cfg: &BundleConfig,
    file_wrappers: &[(String, String)],
    text_wrappers: &[(String, String)],
) -> Result<BundleResult, String> {
    let raw = bundle_via_plugin_inner(entry_file, project_root, external_modules, cfg, file_wrappers, text_wrappers, true, true)?;
    let (code, sm) = extract_inline_sourcemap(&raw);
    let pre_patch_line_count = code.lines().count() as u32;
    let mut code = code;
    code = patch_esbuild_for_hermes(&code);
    let has_externals = !external_modules.is_empty() || !cfg.externals.is_empty()
        || code.contains("Dynamic require of");
    if has_externals {
        code = inject_mock_require_shim(&code);
    }
    code = hoist_mock_modules(&code);
    Ok(BundleResult { code, source_map: sm, pre_patch_line_count })
}

#[allow(clippy::too_many_arguments)]
fn bundle_via_plugin_inner(
    entry_file: &Path,
    project_root: &Path,
    external_modules: &[String],
    cfg: &BundleConfig,
    file_wrappers: &[(String, String)],
    text_wrappers: &[(String, String)],
    sourcemap_inline: bool,
    skip_patches: bool,
) -> Result<String, String> {
    // Nothing to intercept → the JS API detour buys nothing. Use the CLI path:
    // byte-identical behavior and no JS-runtime spawn for suites without mocks
    // needing wrappers. HT_PLUGIN_FORCE=1 disables the shortcut for benchmarking
    // the JS API service overhead in isolation.
    if file_wrappers.is_empty() && text_wrappers.is_empty() && std::env::var("HT_PLUGIN_FORCE").is_err() {
        return bundle_esbuild_with_config_inner(
            entry_file,
            &find_esbuild(project_root)?,
            external_modules, cfg, false, sourcemap_inline, skip_patches,
        );
    }

    let esbuild_bin = find_esbuild(project_root)?;
    let esbuild_lib = find_esbuild_lib(project_root)
        .ok_or_else(|| "esbuild JS API (node_modules/esbuild/lib/main.js) not found".to_string())?;
    let runtime = find_js_runtime()
        .ok_or_else(|| "no JS runtime found for plugin bundling (need bun or node)".to_string())?;

    let (args, node_path_env) =
        assemble_esbuild_args(entry_file, external_modules, cfg, false, sourcemap_inline);

    // Go-side pre-screen: only imports whose last segment matches a mocked
    // target's basename (or a mocked package's name) cross the Go→JS pipe.
    // Directory-resolved targets (index files) also contribute their dir name,
    // since imports of them end with the directory segment.
    let mut parts: Vec<String> = Vec::new();
    for (t, _) in file_wrappers {
        let p = Path::new(t);
        if let Some(stem) = p.file_stem().map(|s| s.to_string_lossy().to_string()) {
            if stem == "index" {
                if let Some(dir_name) = p.parent().and_then(|d| d.file_name()) {
                    parts.push(regex_escape(&dir_name.to_string_lossy()));
                }
            }
            parts.push(regex_escape(&stem));
        }
    }
    let mut text_parts: Vec<String> = Vec::new();
    for (spec, _) in text_wrappers {
        text_parts.push(regex_escape(spec));
    }
    parts.sort();
    parts.dedup();
    text_parts.sort();
    text_parts.dedup();
    let mut alts: Vec<String> = Vec::new();
    if !parts.is_empty() {
        alts.push(format!(
            "(?:^|[/\\\\])(?:{})(?:\\.(?:tsx|ts|jsx|js))?$",
            parts.join("|")
        ));
    }
    if !text_parts.is_empty() {
        alts.push(format!("^(?:{})$", text_parts.join("|")));
    }
    let filter = alts.join("|");

    let wrapper_map: serde_json::Map<String, serde_json::Value> = file_wrappers
        .iter()
        .map(|(t, w)| (t.clone(), serde_json::Value::String(w.clone())))
        .collect();
    let text_wrapper_map: serde_json::Map<String, serde_json::Value> = text_wrappers
        .iter()
        .map(|(t, w)| (t.clone(), serde_json::Value::String(w.clone())))
        .collect();
    let alias_pairs: Vec<serde_json::Value> = cfg
        .aliases
        .iter()
        .map(|(a, t)| serde_json::json!([a, t]))
        .collect();

    let temp = super::shims::hermes_temp_root(project_root);
    // The outfile must live in the PROJECT ROOT, not the temp dir: esbuild
    // computes sourcemap `sources` relative to the outfile's directory, and
    // coverage excludes any source starting with "..". A temp-dir outfile makes
    // every project file "../..."-relative (dropped from coverage) while the
    // /tmp wrapper files become bare relative paths (wrongly included).
    let out_path = project_root.join(format!(".hermes-test-plugin-out-{}.js", std::process::id()));
    let script_path = temp.join("plugin-build.cjs");
    let config_path = temp.join("plugin-build-config.json");

    // esbuild's JS API ignores the NODE_PATH env var (CLI-only) — pass explicitly.
    let node_paths: Vec<String> = node_path_env
        .as_deref()
        .map(|np| np.split(':').map(|s| s.to_string()).collect())
        .unwrap_or_default();

    let config = serde_json::json!({
        "entry": entry_file.to_string_lossy(),
        "out": out_path.to_string_lossy(),
        "esbuildLib": esbuild_lib.to_string_lossy(),
        "args": args,
        "nodePaths": node_paths,
        "wrappers": wrapper_map,
        "textWrappers": text_wrapper_map,
        "aliases": alias_pairs,
        "resolveDir": entry_file.parent().unwrap_or(project_root).to_string_lossy(),
        "filter": filter,
    });

    std::fs::write(&script_path, PLUGIN_BUILD_CJS)
        .map_err(|e| format!("failed to write plugin build script: {e}"))?;
    std::fs::write(&config_path, config.to_string())
        .map_err(|e| format!("failed to write plugin build config: {e}"))?;

    let mut cmd = Command::new(&runtime);
    cmd.arg(&script_path).arg(&config_path);
    cmd.env("ESBUILD_BINARY_PATH", &esbuild_bin);
    if let Some(np) = &node_path_env {
        cmd.env("NODE_PATH", np);
    }
    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to run JS runtime '{runtime}': {e}"))?;

    if std::env::var("HT_DEBUG_RESOLVE").is_ok() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
    let result = if !output.status.success() {
        Err(format!(
            "plugin bundling failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    } else {
        std::fs::read_to_string(&out_path)
            .map_err(|e| format!("failed to read plugin bundle output: {e}"))
    };
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&config_path);
    let _ = std::fs::remove_file(&out_path);
    let code = result?;

    if skip_patches {
        return Ok(code);
    }

    // Same patch pipeline as the CLI path.
    let mut code = patch_esbuild_for_hermes(&code);
    let has_externals = !external_modules.is_empty()
        || !cfg.externals.is_empty()
        || code.contains("Dynamic require of");
    if has_externals {
        code = inject_mock_require_shim(&code);
    }
    code = hoist_mock_modules(&code);
    Ok(code)
}

fn bundle_esbuild_with_config_inner(
    entry_file: &Path,
    esbuild_path: &Path,
    external_modules: &[String],
    cfg: &BundleConfig,
    packages_external: bool,
    sourcemap_inline: bool,
    skip_patches: bool,
) -> Result<String, String> {
    let (args, node_path_env) =
        assemble_esbuild_args(entry_file, external_modules, cfg, packages_external, sourcemap_inline);

    let mut cmd = Command::new(esbuild_path);
    cmd.arg(entry_file);
    for a in &args {
        cmd.arg(a);
    }
    if let Some(np) = &node_path_env {
        cmd.env("NODE_PATH", np);
    }

    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run esbuild: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check for unresolved native modules and suggest adding to externals
        let mut suggestions: Vec<String> = Vec::new();
        for line in stderr.lines() {
            if line.contains("Could not resolve") {
                // Extract module name from: Could not resolve "react-native-foo"
                if let Some(start) = line.find('"') {
                    if let Some(end) = line[start + 1..].find('"') {
                        let module = &line[start + 1..start + 1 + end];
                        if !module.starts_with('.') && !module.starts_with('/') {
                            suggestions.push(module.to_string());
                        }
                    }
                }
            }
        }
        if !suggestions.is_empty() {
            let hint = suggestions.iter()
                .map(|s| format!("  \"{s}\""))
                .collect::<Vec<_>>()
                .join(",\n");
            return Err(format!(
                "esbuild failed: {stderr}\n\n\
                 Hint: these modules could not be resolved. If they are native modules,\n\
                 add them to \"externals\" in hermes-test.config.json:\n\n\
                 \"externals\": [\n{hint}\n]"
            ));
        }
        return Err(format!("esbuild failed: {stderr}"));
    }

    let code =
        String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 from esbuild: {e}"))?;

    let mut code = code.to_string();

    if skip_patches {
        return Ok(code);
    }

    // Patch esbuild runtime helpers for Hermes compat
    code = patch_esbuild_for_hermes(&code);

    // Inject __require shim when there are external modules that need runtime resolution
    let has_externals = !external_modules.is_empty() || !cfg.externals.is_empty()
        || packages_external || code.contains("Dynamic require of");
    if has_externals {
        code = inject_mock_require_shim(&code);
    }

    // Hoist mock() calls before require() calls so aliased shadow-wrapper mocks
    // are registered before the module initializers run (captures dispatch, getState, etc.)
    let hoisted = hoist_mock_modules(&code);
    if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
        let _ = std::fs::write("/tmp/ht_bundle_hoisted.js", &hoisted);
        let _ = std::fs::write("/tmp/ht_bundle_original.js", &code);
    }
    code = hoisted;

    Ok(code)
}

/// Bundle with esbuild and return the dependency graph (metafile).
/// Returns (bundle_code, map of input_file → Vec<test_files_that_import_it>).
pub fn bundle_with_depgraph(
    entry_file: &Path,
    project_root: &Path,
    test_files: &[PathBuf],
    external_modules: &[String],
) -> Result<(String, DepGraph), String> {
    let esbuild_path = find_esbuild(project_root)?;

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

    // Hoist mock() calls before require() calls so aliased shadow-wrapper mocks
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

// Public wrappers for persistent watch mode
pub fn find_esbuild_pub(project_root: &Path) -> Result<PathBuf, String> {
    find_esbuild(project_root)
}

#[cfg(test)]
mod find_esbuild_tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ht-find-esbuild-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(p: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    const MACHO64: &[u8] = &[0xCF, 0xFA, 0xED, 0xFE, 0, 0, 0, 0];
    const SHIM: &[u8] = b"#!/usr/bin/env node\nrequire('child_process').execFileSync(process.env.ESBUILD_BINARY_PATH)\n";

    #[test]
    fn detects_native_vs_shim() {
        let d = tmp("magic");
        write(&d.join("native"), MACHO64);
        write(&d.join("elf"), &[0x7F, b'E', b'L', b'F', 0, 0, 0, 0]);
        write(&d.join("shim"), SHIM);
        write(&d.join("empty"), b"");
        assert!(is_native_executable(&d.join("native")));
        assert!(is_native_executable(&d.join("elf")));
        assert!(!is_native_executable(&d.join("shim")));
        assert!(!is_native_executable(&d.join("empty")));
        assert!(!is_native_executable(&d.join("missing")));
    }

    #[test]
    fn refuses_js_shim_with_actionable_error() {
        let d = tmp("shim-only");
        write(&d.join("node_modules/.bin/esbuild"), SHIM);
        write(&d.join("node_modules/esbuild/bin/esbuild"), SHIM);
        let saved = std::env::var_os("PATH");
        std::env::set_var("PATH", "");
        let err = find_esbuild(&d).unwrap_err();
        if let Some(p) = saved { std::env::set_var("PATH", p); }
        assert!(err.contains("JS shim"), "{err}");
        assert!(err.contains("rm -rf node_modules"), "{err}");
    }

    #[test]
    fn prefers_platform_package_over_bin_shim() {
        let Some(pkg) = esbuild_platform_pkg() else { return };
        let d = tmp("platform-pkg");
        write(&d.join("node_modules/.bin/esbuild"), SHIM);
        let native = d.join("node_modules").join(pkg).join("bin/esbuild");
        write(&native, MACHO64);
        let found = find_esbuild(&d).unwrap();
        assert_eq!(found, std::fs::canonicalize(&native).unwrap());
    }

    #[test]
    fn walks_up_to_parent_node_modules() {
        let Some(pkg) = esbuild_platform_pkg() else { return };
        let root = tmp("monorepo");
        let native = root.join("node_modules").join(pkg).join("bin/esbuild");
        write(&native, MACHO64);
        let app = root.join("apps/app");
        std::fs::create_dir_all(&app).unwrap();
        assert_eq!(find_esbuild(&app).unwrap(), std::fs::canonicalize(&native).unwrap());
    }
}
