//! In-process bundling via the Rolldown Rust crate — the candidate successor
//! to the esbuild JS-API pipeline (hardening assessment, enhancement 7).
//!
//! What this buys over esbuild (spike-proven, July 2026):
//! - No Node spawn, no plugin_build.cjs, no JSON config round-trip
//!   (~0.5s measured cold overhead on the esbuild JS-API path)
//! - Mock delivery as a native Rust resolve_id hook instead of a JS plugin
//! - Scope-hoisted ESM output: zero interop helpers for pure-ESM code, so the
//!   mockability patches only concern the CJS boundary
//!
//! Status: EXPERIMENTAL — not wired into the run path. The fixture test below
//! proves the mock-delivery primitive (resolution-time wrapper redirection)
//! end-to-end in real Hermes. Phase 1 (bundling the examples app) comes next;
//! see the migration plan in hardening-assessment.md before extending.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use rolldown::plugin::__inner::SharedPluginable;
use rolldown::plugin::{
    HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext,
};
use rolldown::{Bundler, BundlerOptions, InputItem, OutputFormat};
use rolldown_common::Output;

/// Resolution-time mock delivery: imports whose resolved target is a mocked
/// file get redirected to their wrapper. This is the Rust twin of
/// plugin_build.cjs's onResolve — the "receptionist" from the architecture
/// docs, minus the process boundary.
#[derive(Debug)]
struct HtMockResolver {
    /// specifier suffix → wrapper absolute path.
    /// Phase 1 keeps esbuild-parity matching rules out of scope; suffix
    /// matching is enough to prove delivery. The real port must replicate
    /// the identity-vs-specifier-text split documented in mock-resolution.md.
    wrappers: HashMap<String, String>,
    /// esbuild --alias parity: (name, target). Exact or name/-prefixed
    /// specifiers redirect to target (shims, tsconfig aliases).
    aliases: Vec<(String, String)>,
    /// Bare-specifier alias targets (@__ht_real_pkg/x -> x) must re-resolve
    /// from the PROJECT, not from the temp shim dir that imported them.
    resolve_anchor: String,
}

impl Plugin for HtMockResolver {
    fn name(&self) -> Cow<'static, str> {
        "ht-mock-resolver".into()
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> HookResolveIdReturn {
        if args.importer.is_some() {
            for (suffix, wrapper) in &self.wrappers {
                if args.specifier.ends_with(suffix.as_str()) {
                    return Ok(Some(HookResolveIdOutput::from_id(wrapper.as_str())));
                }
            }
        }
        for (name, target) in &self.aliases {
            let rewritten = if args.specifier == name.as_str() {
                Some(target.clone())
            } else {
                args.specifier
                    .strip_prefix(&format!("{name}/"))
                    .map(|rest| format!("{target}/{rest}"))
            };
            if let Some(new_spec) = rewritten {
                // Exact file targets (shims) can be answered directly; anything
                // else re-enters the resolver so package-exports/extension
                // resolution still happens (rollup-style ctx.resolve).
                if std::path::Path::new(&new_spec).is_file() {
                    return Ok(Some(HookResolveIdOutput::from_id(new_spec.as_str())));
                }
                let anchor;
                let importer = if std::path::Path::new(&new_spec).is_absolute() {
                    args.importer
                } else {
                    anchor = format!("{}/__ht_resolve_anchor__.js", self.resolve_anchor);
                    Some(anchor.as_str())
                };
                match _ctx.resolve(&new_spec, importer, None).await {
                    Ok(Ok(resolved)) => {
                        return Ok(Some(HookResolveIdOutput::from_id(resolved.id.as_str())));
                    }
                    _ => {
                        return Ok(Some(HookResolveIdOutput::from_id(new_spec.as_str())));
                    }
                }
            }
        }
        Ok(None)
    }
}

/// Bundle `entry` into a single IIFE chunk, redirecting mocked imports to
/// wrapper files. Synchronous facade over rolldown's async API (the CLI is
/// synchronous throughout).
///
/// `externals`: esbuild-parity semantics — exact match, `pkg/…` sub-paths,
/// and trailing-`*` prefixes.
pub fn bundle_via_rolldown(
    entry: &Path,
    cwd: &Path,
    wrappers: HashMap<String, String>,
    externals: Vec<String>,
    aliases: Vec<(String, String)>,
) -> Result<String, String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime: {e}"))?;

    rt.block_on(async move {
        let ext = std::sync::Arc::new(externals);
        let is_external = rolldown_common::IsExternal::Fn(Some(Arc::new(move |spec, _importer, _resolved| {
            let ext = Arc::clone(&ext);
            let spec = spec.to_string();
            Box::pin(async move {
                Ok(ext.iter().any(|e| {
                    if let Some(prefix) = e.strip_suffix('*') {
                        spec.starts_with(prefix)
                    } else {
                        spec == *e || spec.starts_with(&format!("{e}/"))
                    }
                }))
            })
        })));

        let mut define = rolldown_utils::indexmap::FxIndexMap::default();
        define.insert("process.env.NODE_ENV".to_string(), "\"test\"".to_string());
        define.insert("process.env.JEST_WORKER_ID".to_string(), "\"1\"".to_string());
        define.insert("global".to_string(), "globalThis".to_string());

        let options = BundlerOptions {
            input: Some(vec![InputItem {
                name: Some("bundle".to_string()),
                import: entry.to_string_lossy().to_string(),
            }]),
            cwd: Some(cwd.to_path_buf()),
            format: Some(OutputFormat::Iife),
            external: Some(is_external),
            define: Some(define),
            ..Default::default()
        };
        let resolve_anchor = cwd.to_string_lossy().to_string();
        let plugins: Vec<SharedPluginable> = vec![Arc::new(HtMockResolver { wrappers, aliases, resolve_anchor })];
        let mut bundler = Bundler::with_plugins(options, plugins)
            .map_err(|e| format!("rolldown bundler construction: {e:?}"))?;
        let out = bundler
            .generate()
            .await
            .map_err(|e| format!("rolldown bundle:\n{}", e.into_vec().iter().map(|d| d.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n")))?;

        for asset in &out.assets {
            if let Output::Chunk(chunk) = asset {
                return Ok(chunk.code.clone());
            }
        }
        Err("rolldown produced no chunk".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hermes::Runtime;
    use std::fs;

    /// The mock-delivery primitive, end to end IN-REPO: a Rust resolve_id
    /// hook redirects an import to a wrapper, the IIFE output executes in
    /// real (V1) Hermes, and the wrapper's value comes out.
    #[test]
    fn rolldown_delivers_wrapper_and_runs_in_hermes() {
        let dir = std::env::temp_dir().join(format!(
            "ht-rolldown-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("entry.js"),
            "import { greet } from './analytics.js';\nglobalThis.result = greet('hermes');\n",
        )
        .unwrap();
        fs::write(
            dir.join("analytics.js"),
            "export function greet(n) { return 'REAL:' + n; }\n",
        )
        .unwrap();
        fs::write(
            dir.join("analytics.wrapper.js"),
            "export function greet(n) { return 'WRAPPED:' + n; }\n",
        )
        .unwrap();

        let mut wrappers = HashMap::new();
        wrappers.insert(
            "analytics.js".to_string(),
            dir.join("analytics.wrapper.js").to_string_lossy().to_string(),
        );

        let code = bundle_via_rolldown(&dir.join("entry.js"), &dir, wrappers, vec![], vec![]).unwrap();
        assert!(code.contains("WRAPPED"), "wrapper not delivered:\n{code}");
        assert!(
            !code.contains("REAL:"),
            "real module should be tree-shaken:\n{code}"
        );

        let rt = Runtime::new().expect("hermes runtime");
        let program = format!("{code}\nglobalThis.result;");
        let result = rt.eval(&program, "rolldown-smoke.js").expect("hermes eval");
        assert!(
            result.contains("WRAPPED:hermes"),
            "hermes execution: got {result}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// Phase-1 benchmark: bundle the REAL examples app through both pipelines
    /// on identical inputs. Run explicitly:
    ///   cargo test -p hermes-test-cli --release bench_rolldown -- --ignored --nocapture
    ///
    /// Honesty notes printed with the results: the esbuild leg is the
    /// production path (Node service + onResolve plugin, wrapper-carrying);
    /// the rolldown leg delivers the same FILE wrappers via the Rust hook but
    /// does NOT yet implement specifier-text (alias/package) mocks — those
    /// bundle their real modules instead, which if anything gives rolldown
    /// MORE work, not less.
    #[test]
    #[ignore]
    fn bench_rolldown_vs_esbuild_examples_app() {
        let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let root = repo_root.join("examples/expo-app");
        let cfg = crate::bundler::read_config(&root);
        let alias_names: Vec<String> = cfg.aliases.iter().map(|(a, _)| a.clone()).collect();
        let alias_pairs: Vec<(String, String)> = cfg.aliases.clone();
        let test_files = crate::bundler::find_test_files(&root);
        assert!(!test_files.is_empty(), "no example test files found");
        let mocks = crate::bundler::find_mock_modules_with_alias_pairs(
            &test_files,
            &alias_names,
            &alias_pairs,
        );

        let (shim_cfg, _wrapper_dir) = crate::bundler::create_wrapper_shims(&root, &cfg);
        let entry = crate::bundler::generate_entry_with_shallow(
            &test_files, None, &mocks, &shim_cfg, &[], Some(&root), &[],
        );
        let entry_path = root.join(".hermes-test-bench-entry.js");
        fs::write(&entry_path, &entry).unwrap();

        // esbuild leg: the real production path (min of 3)
        let mut es_times = Vec::new();
        let mut es_len = 0usize;
        for _ in 0..3 {
            let pm = crate::bundler::create_plugin_mock_wrappers(&test_files, &root, &shim_cfg, &mocks);
            let t = std::time::Instant::now();
            let code = crate::bundler::bundle_via_plugin_with_config(
                &entry_path, &root, &pm.external_mocks, &shim_cfg,
                &pm.file_wrappers, &pm.text_wrappers,
            )
            .expect("esbuild bundle");
            es_times.push(t.elapsed());
            es_len = code.len();
            let _ = fs::remove_dir_all(&pm.dir);
        }

        // rolldown leg: same entry, same file wrappers, same externals (min of 3)
        let mut rd_times = Vec::new();
        let mut rd_len = 0usize;
        let mut rd_err: Option<String> = None;
        for _ in 0..3 {
            let pm = crate::bundler::create_plugin_mock_wrappers(&test_files, &root, &shim_cfg, &mocks);
            let wrappers: HashMap<String, String> = pm
                .file_wrappers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let mut externals = pm.external_mocks.clone();
            externals.extend(shim_cfg.externals.iter().cloned());
            externals.push("hermes-test".to_string());
            let t = std::time::Instant::now();
            let aliases: Vec<(String, String)> = shim_cfg
                .aliases
                .iter()
                .map(|(a, t)| (a.clone(), t.clone()))
                .collect();
            match bundle_via_rolldown(&entry_path, &root, wrappers, externals, aliases) {
                Ok(code) => {
                    rd_times.push(t.elapsed());
                    rd_len = code.len();
                }
                Err(e) => {
                    rd_err = Some(e);
                    break;
                }
            }
            let _ = fs::remove_dir_all(&pm.dir);
        }

        let _ = fs::remove_file(&entry_path);

        let min = |v: &Vec<std::time::Duration>| v.iter().min().copied().unwrap_or_default();
        println!("==== phase-1 bundle benchmark: examples app ({} test files) ====", test_files.len());
        println!(
            "esbuild (prod path, node service + plugin): min {:?} of {:?}, bundle {} KB",
            min(&es_times), es_times, es_len / 1024
        );
        match rd_err {
            None => println!(
                "rolldown (in-process crate, file wrappers): min {:?} of {:?}, bundle {} KB",
                min(&rd_times), rd_times, rd_len / 1024
            ),
            Some(e) => println!("rolldown FAILED (phase-1 finding, not a verdict): {e}"),
        }
    }
}
