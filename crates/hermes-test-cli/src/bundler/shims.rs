use std::path::{Path, PathBuf};

use super::config::BundleConfig;

pub(crate) fn hermes_temp_root(project_root: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    project_root.to_string_lossy().hash(&mut hasher);
    let project_hash = hasher.finish();
    let pid = std::process::id();
    let root = std::env::temp_dir().join(format!("hermes-test-work-{project_hash:x}-{pid}"));
    let _ = std::fs::create_dir_all(&root);
    root
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
