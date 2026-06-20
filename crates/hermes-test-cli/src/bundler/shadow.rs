use std::path::{Path, PathBuf};

use super::config::BundleConfig;

fn hermes_temp_root(project_root: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    project_root.to_string_lossy().hash(&mut hasher);
    let project_hash = hasher.finish();
    let pid = std::process::id();
    let root = std::env::temp_dir().join(format!("hermes-test-work-{project_hash:x}-{pid}"));
    let _ = std::fs::create_dir_all(&root);
    root
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

    let temp_root = hermes_temp_root(project_root);
    for (alias, mocked_paths) in &alias_mocks {
        let Some((_alias_name, target)) = cfg.aliases.iter().find(|(a, _)| a == alias) else { continue };

        let shadow_dir = temp_root.join(format!("shadow-{}", alias.replace('/', "-").replace('@', "")));
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

    let shim_dir = hermes_temp_root(project_root).join("pkg-shims");
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
var _fnCache = {{}};
var _h = {{
  get: function(t, p) {{
    if (p === '__esModule') return true;
    if (typeof p === 'symbol') return undefined;
    var fm = globalThis.__HT_file_mocks;
    var f = globalThis.__currentTestFile;
    var mocks = fm && f && fm[f];
    var m = mocks && mocks['{pkg}'];
    if (m && p in m) return m[p];
    var real = _getReal()[p];
    if (typeof real === 'function' && !_fnCache[p]) {{
      _fnCache[p] = new Proxy(real, {{ apply: function(target, thisArg, args) {{
        var fm2 = globalThis.__HT_file_mocks;
        var f2 = globalThis.__currentTestFile;
        var m2 = fm2 && f2 && fm2[f2] && fm2[f2]['{pkg}'];
        if (m2 && p in m2) return m2[p].apply(thisArg, args);
        return target.apply(thisArg, args);
      }} }});
    }}
    return _fnCache[p] || real;
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

    let shim_dir = hermes_temp_root(project_root).join("wrapper-shims");
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
pub fn find_hermes_test_shims_dirs(project_root: &Path, cfg: &BundleConfig) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let nm_candidates = [
        "node_modules/hermes-test/shims",
        "node_modules/hermes-test/src/shims",
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
pub fn resolve_shim_content(shims_dirs: &[PathBuf], name: &str) -> Option<String> {
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
pub fn create_shadow_tree(
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
var _fnCache = {{}};
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
    var real = _getReal()[p];
    if (typeof real === 'function' && !_fnCache[p]) {{
      _fnCache[p] = new Proxy(real, {{ apply: function(target, thisArg, args) {{
        var fm2 = globalThis.__HT_file_mocks;
        var f2 = globalThis.__currentTestFile;
        var m2 = fm2 && f2 && fm2[f2] && fm2[f2]['{}'];
        if (m2 && p in m2) return m2[p].apply(thisArg, args);
        return target.apply(thisArg, args);
      }} }});
    }}
    return _fnCache[p] || real;
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
                    mock_path_str,
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
