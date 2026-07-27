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
        Ok(None)
    }
}

/// Bundle `entry` into a single IIFE chunk, redirecting mocked imports to
/// wrapper files. Synchronous facade over rolldown's async API (the CLI is
/// synchronous throughout).
pub fn bundle_via_rolldown(
    entry: &Path,
    cwd: &Path,
    wrappers: HashMap<String, String>,
) -> Result<String, String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime: {e}"))?;

    rt.block_on(async move {
        let options = BundlerOptions {
            input: Some(vec![InputItem {
                name: Some("bundle".to_string()),
                import: entry.to_string_lossy().to_string(),
            }]),
            cwd: Some(cwd.to_path_buf()),
            format: Some(OutputFormat::Iife),
            ..Default::default()
        };
        let plugins: Vec<SharedPluginable> = vec![Arc::new(HtMockResolver { wrappers })];
        let mut bundler = Bundler::with_plugins(options, plugins)
            .map_err(|e| format!("rolldown bundler construction: {e:?}"))?;
        let out = bundler
            .generate()
            .await
            .map_err(|e| format!("rolldown bundle: {e:?}"))?;

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

        let code = bundle_via_rolldown(&dir.join("entry.js"), &dir, wrappers).unwrap();
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
}
