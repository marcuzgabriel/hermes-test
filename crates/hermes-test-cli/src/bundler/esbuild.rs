use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::config::{BundleConfig, read_config};
use super::patches::{patch_esbuild_for_hermes, inject_mock_require_shim, hoist_mock_modules};
use super::shadow::{create_shadow_wrappers, create_package_shims, create_wrapper_shims};
use super::entry::{generate_group_entry_pub, compute_bundle_cache_key};

// SWC class transform was evaluated but rejected:
// - Requires 3 scoped thread-locals (GLOBALS, HANDLER, HELPERS)
// - inject_helpers() emits require('@swc/helpers') calls incompatible with Hermes
// - Full SWC codegen re-emits the entire bundle (changes whitespace, breaks other patches)
// Our regex-based fix_all_class_extends() handles all known patterns at <1ms with zero deps.

/// Bundle an entry file with esbuild.
pub fn bundle_auto(
    entry_file: &Path,
    project_root: &Path,
    external_modules: &[String],
) -> Result<String, String> {
    let path = find_esbuild(project_root)
        .map_err(|_| "esbuild not found. Install it: bun add -d esbuild".to_string())?;
    bundle_esbuild(entry_file, &path, external_modules)
}

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

pub fn find_esbuild(project_root: &Path) -> Result<PathBuf, ()> {
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
    let path = find_esbuild(project_root)
        .map_err(|_| "esbuild not found. Install it: bun add -d esbuild".to_string())?;
    bundle_esbuild_with_config(entry_file, &path, external_modules, cfg, false)
}

/// Bundle with esbuild + inline source map for coverage. Returns code, parsed source map,
/// and the pre-patch line count (used to compute line delta after patches).
pub fn bundle_esbuild_with_sourcemap(
    entry_file: &Path,
    esbuild_path: &Path,
    external_modules: &[String],
    cfg: &BundleConfig,
) -> Result<BundleResult, String> {
    // Run esbuild with inline source map
    let raw = bundle_esbuild_with_config_inner(entry_file, esbuild_path, external_modules, cfg, false, true, true)?;

    // Extract inline source map from the bundle
    let (code, sm) = extract_inline_sourcemap(&raw);
    let pre_patch_line_count = code.lines().count() as u32;

    // Apply patches (these shift line numbers, we track the delta)
    let mut code = code;
    code = super::patches::patch_esbuild_for_hermes(&code);
    let has_externals = !external_modules.is_empty() || !cfg.externals.is_empty()
        || code.contains("Dynamic require of");
    if has_externals {
        code = super::patches::inject_mock_require_shim(&code);
    }
    code = super::patches::hoist_mock_modules(&code);

    Ok(BundleResult { code, source_map: sm, pre_patch_line_count })
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

/// Bundle with esbuild using provided config (avoids re-reading from disk).
/// When `packages_external` is true, adds --packages=external to externalize all node_modules.
pub fn bundle_esbuild_with_config(
    entry_file: &Path,
    esbuild_path: &Path,
    external_modules: &[String],
    cfg: &BundleConfig,
    packages_external: bool,
) -> Result<String, String> {
    bundle_esbuild_with_config_inner(entry_file, esbuild_path, external_modules, cfg, packages_external, false, false)
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

    if sourcemap_inline {
        cmd.arg("--sourcemap=inline");
    }

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

    // Externalize hermes-test itself (thin re-export from __HT runtime)
    cmd.arg("--external:hermes-test");
    // Alias hermes-test/store to the actual file so it gets BUNDLED (not externalized).
    // esbuild aliases run before external checks, so this resolves before the external match.
    {
        let store_paths = [
            entry_file.parent().unwrap_or(std::path::Path::new(".")).join("node_modules/hermes-test/src/store.ts"),
        ];
        for sp in &store_paths {
            if sp.exists() {
                cmd.arg(format!("--alias:hermes-test/store={}", sp.to_string_lossy()));
                break;
            }
        }
        if let Some(ref root) = cfg.root {
            let root_store = root.join("node_modules/hermes-test/src/store.ts");
            if root_store.exists() {
                cmd.arg(format!("--alias:hermes-test/store={}", root_store.to_string_lossy()));
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
            cmd.arg(format!("--alias:react-reconciler={}", rec_path.to_string_lossy()));
            let constants = rec_path.join("constants.js");
            if constants.exists() {
                cmd.arg(format!("--alias:react-reconciler/constants={}", constants.to_string_lossy()));
            }
        }
    }

    // react-native uses Flow syntax that esbuild can't parse — always external.
    // All other native packages are auto-detected or user-configured.
    for ext in &["react-native", "react-native/*"] {
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
        let entry = generate_group_entry_internal(chunk, mock_modules, Some(project_root));
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

/// Internal version of generate_group_entry used by bundle_split.
fn generate_group_entry_internal(test_files: &[PathBuf], mock_modules: &[String], project_root: Option<&Path>) -> String {
    generate_group_entry_pub(test_files, mock_modules, project_root)
}

/// Setup code eval'd before vendor: shims, mock placeholders, harness mocks.
fn generate_setup_code(
    _test_files: &[PathBuf],
    mock_modules: &[String],
    cfg: &BundleConfig,
) -> String {
    let mut code = String::new();

    code.push_str("if (typeof globalThis.__DEV__ === 'undefined') globalThis.__DEV__ = false;\n");
    code.push_str("globalThis.__HT_mocks = globalThis.__HT_mocks || {};\n");
    code.push_str("globalThis.__HT_mocks['hermes-test'] = globalThis.__HT;\n");

    // Built-in react-native shim (unless user provides custom one)
    let user_shim_modules: Vec<&str> = cfg.shims.iter().map(|(k, _)| k.as_str()).collect();
    if !user_shim_modules.contains(&"react-native") {
        code.push_str(&format!(
            "globalThis.__HT_mocks['react-native'] = (function() {{ var module = {{ exports: {{}} }}; {}; return module.exports; }})();\n",
            include_str!("../../../../packages/hermes-test/src/shims/react-native.js")
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
    entry.push_str(
        "try { var __htRec = require('react-reconciler'); globalThis.__HT_Reconciler = typeof __htRec === 'function' ? __htRec : (__htRec.default || __htRec); var __htRecC = require('react-reconciler/constants'); globalThis.__HT_ReconcilerConstants = __htRecC.__esModule ? __htRecC : (__htRecC.default || __htRecC); } catch(e) {}\n"
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

// Public wrappers for persistent watch mode
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
