//! In-process bundling via the Rolldown crate — the only bundler.
//!
//! This replaced the esbuild pipeline (JS-API service driven through a spawned
//! Node process, plugin_build.cjs, JSON config round-trips, and four regex
//! patches over the emitted text). What each piece became here:
//!
//! | esbuild-era machinery                | now                                    |
//! |--------------------------------------|----------------------------------------|
//! | plugin_build.cjs + Node + JSON       | `HtMockResolver::resolve_id` (in-proc) |
//! | inject_mock_require_shim (regex)     | `EXTERNALS_PRELUDE` (static JS)        |
//! | patch 3 __toESM regex                | `transform` hook on rolldown's runtime |
//! | patches 1–2 (configurable getters)   | unnecessary (scope hoisting)           |
//! | hoist_mock_modules (+brace matcher)  | unnecessary (wrapper call-time checks) |
//! | --supported:async-await=false        | transform target "es2016"              |
//!
//! Benchmark record (examples app, 24 suites / 1211 tests, min-of-3,
//! identical inputs, 2026-07-27): esbuild production path 177.6ms / 1646 KB;
//! rolldown in-process 46.5ms / 1383 KB — 3.8x faster, 16% smaller, with
//! 100% test parity (1211/1211 on both).

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use rolldown::plugin::__inner::SharedPluginable;
use rolldown::plugin::{
    HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext,
    PluginContextResolveOptions,
};
use rolldown::{Bundler, BundlerOptions, BundlerTransformOptions, InputItem, OutputFormat};
use rolldown_common::{Output, SourceMapType};

/// Core of the runtime prelude. Rollup-dialect externals are GLOBALS, not
/// require() calls; every externalized module's global gets the forgiving
/// Proxy semantics the old require shim had: per-file mocks → global registry
/// (natives get their shims registered there by the entry) → chainable noop.
/// 'hermes-test' additionally falls back to the harness + store surfaces.
const PRELUDE_CORE: &str = r#"(function(){
  var noopFn = function(){};
  var noop = new Proxy(noopFn, {
    get: function(t,p){ if (p === Symbol.toPrimitive) return function(){ return ''; };
      if (p === 'then' || p === '$$typeof') return undefined;
      if (p === 'length' || p === 'size') return 0;
      if (typeof p === 'symbol') return undefined; return noop; },
    apply: function(){ return noop; },
    construct: function(){ return {}; }
  });
  globalThis.require = function(m) {
    if (m === 'hermes-test') return globalThis.hermes_test;
    throw new Error('require not available for: ' + m);
  };
  globalThis.__htExternalProxy = function(spec, isHermesTest) {
    return new Proxy({}, {
      get: function(t, p){
        if (p === '__esModule') return true;
        if (typeof p === 'symbol') return undefined;
        var fm = globalThis.__HT_file_mocks, cf = globalThis.__currentTestFile;
        var m = fm && cf && fm[cf] && fm[cf][spec];
        var reg = globalThis.__HT_mocks && globalThis.__HT_mocks[spec];
        var v = (m && m[p] !== undefined) ? m[p]
          : (reg && reg[p] !== undefined) ? reg[p]
          : undefined;
        if (v === undefined && isHermesTest) {
          v = (globalThis.__HT && globalThis.__HT[p] !== undefined) ? globalThis.__HT[p]
            : (globalThis.__HT_storeSurface ? globalThis.__HT_storeSurface[p] : undefined);
        }
        return v !== undefined ? v : noop;
      }
    });
  };
})();
"#;

/// Build the full prelude: core + one global per external, mapped by zipping
/// the IIFE's parameter names (positional) with the chunk's external imports.
fn build_prelude(code: &str, imports: &[String]) -> String {
    let mut prelude = String::from(PRELUDE_CORE);
    // Parse `(function(a, b, c) {` from the chunk head.
    let params: Vec<String> = code
        .find("(function(")
        .and_then(|p| {
            let rest = &code[p + 10..code.len().min(p + 2000)];
            rest.find(')').map(|end| {
                rest[..end]
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
        })
        .unwrap_or_default();
    for (i, param) in params.iter().enumerate() {
        if let Some(spec) = imports.get(i) {
            let is_ht = spec == "hermes-test";
            prelude.push_str(&format!(
                "globalThis.{param} = globalThis.__htExternalProxy('{spec}', {is_ht});
"
            ));
        }
    }
    prelude
}

/// Find the index of the `)` matching the `(` at `open` (string-aware).
fn find_paren_close(code: &str, open: usize) -> usize {
    let bytes = code.as_bytes();
    let mut depth = 0;
    let mut j = open;
    while j < bytes.len() {
        match bytes[j] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
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

/// The receptionist (see website/docs/architecture/mock-resolution.md) as an
/// in-process resolver hook. Directions only, never answers: which FILE sits
/// at this import site. The brain (wrapper get(), run time) is unchanged JS.
#[derive(Debug)]
struct HtMockResolver {
    /// FILE-IDENTITY wrappers (relative mocks): resolved absolute path of the
    /// mocked file → wrapper path. Matched after resolving against importer.
    file_wrappers: HashMap<String, String>,
    /// SPECIFIER-TEXT wrappers (alias + package mocks incl. barrel ancestors):
    /// exact import text → wrapper path — the legacy boundary, faithfully
    /// kept (production-internal relative imports never see text mocks).
    text_wrappers: HashMap<String, String>,
    /// esbuild --alias parity: (name, target) for config shims + tsconfig
    /// aliases + @__ht_real_pkg re-entries.
    aliases: Vec<(String, String)>,
    /// Bare specifiers must re-resolve from the PROJECT (wrapper/shim temp
    /// dirs have no node_modules ancestry).
    resolve_anchor: String,
    /// Project root for test-file ids in the context prologue.
    project_root: String,
}

impl HtMockResolver {
    fn skip_self() -> PluginContextResolveOptions {
        PluginContextResolveOptions { skip_self: true, ..Default::default() }
    }
}

impl Plugin for HtMockResolver {
    fn name(&self) -> Cow<'static, str> {
        "ht-mock-resolver".into()
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId | HookUsage::Load | HookUsage::Transform
    }

    /// Binary assets (images/fonts/media) resolve to empty modules — the
    /// esbuild --loader:.png=empty parity, served from the load hook so the
    /// file bytes are never read as UTF-8 source.
    async fn load(
        &self,
        _ctx: rolldown::plugin::SharedLoadPluginContext,
        args: &rolldown::plugin::HookLoadArgs<'_>,
    ) -> rolldown::plugin::HookLoadReturn {
        const EMPTY_EXTS: [&str; 10] =
            ["png", "jpg", "jpeg", "gif", "svg", "webp", "ttf", "otf", "mp4", "riv"];
        let path = args.id.split('?').next().unwrap_or(args.id);
        if let Some(ext) = std::path::Path::new(path).extension().and_then(|e| e.to_str()) {
            if EMPTY_EXTS.contains(&ext.to_ascii_lowercase().as_str()) {
                return Ok(Some(rolldown::plugin::HookLoadOutput {
                    code: "export default {};".into(),
                    module_type: Some(rolldown_common::ModuleType::Js),
                    ..Default::default()
                }));
            }
        }
        Ok(None)
    }

    async fn resolve_id(
        &self,
        ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> HookResolveIdReturn {
        // 1. Wrapper re-entry to the real module: {abs}?ht-real (relative
        //    mocks) or {package}?ht-real (package mocks → re-resolve, anchored).
        if let Some(real) = args.specifier.strip_suffix("?ht-real") {
            if Path::new(real).is_absolute() {
                return Ok(Some(HookResolveIdOutput::from_id(real)));
            }
            let anchor = format!("{}/__ht_resolve_anchor__.js", self.resolve_anchor);
            if let Ok(Ok(r)) =
                ctx.resolve(real, Some(anchor.as_str()), Some(Self::skip_self())).await
            {
                return Ok(Some(HookResolveIdOutput::from_id(r.id.as_str())));
            }
            return Ok(Some(HookResolveIdOutput::from_id(real)));
        }
        // 2. Specifier-text mocks — exact import text.
        if args.importer.is_some() {
            if let Some(w) = self.text_wrappers.get(args.specifier) {
                return Ok(Some(HookResolveIdOutput::from_id(w.as_str())));
            }
        }
        // 3. File-identity mocks — resolve against the importer, then match.
        if args.importer.is_some()
            && (args.specifier.starts_with("./") || args.specifier.starts_with("../"))
            && !self.file_wrappers.is_empty()
        {
            if let Ok(Ok(r)) =
                ctx.resolve(args.specifier, args.importer, Some(Self::skip_self())).await
            {
                if let Some(w) = self.file_wrappers.get(r.id.as_str()) {
                    return Ok(Some(HookResolveIdOutput::from_id(w.as_str())));
                }
                return Ok(Some(HookResolveIdOutput::from_id(r.id.as_str())));
            }
        }
        // 4. Alias/shim redirection (rollup-style: re-resolve non-file targets).
        for (name, target) in &self.aliases {
            let rewritten = if args.specifier == name.as_str() {
                Some(target.clone())
            } else {
                args.specifier
                    .strip_prefix(&format!("{name}/"))
                    .map(|rest| format!("{target}/{rest}"))
            };
            if let Some(new_spec) = rewritten {
                if Path::new(&new_spec).is_file() {
                    return Ok(Some(HookResolveIdOutput::from_id(new_spec.as_str())));
                }
                let anchor;
                let importer = if Path::new(&new_spec).is_absolute() {
                    args.importer
                } else {
                    anchor = format!("{}/__ht_resolve_anchor__.js", self.resolve_anchor);
                    Some(anchor.as_str())
                };
                if let Ok(Ok(r)) = ctx.resolve(&new_spec, importer, None).await {
                    return Ok(Some(HookResolveIdOutput::from_id(r.id.as_str())));
                }
                return Ok(Some(HookResolveIdOutput::from_id(new_spec.as_str())));
            }
        }
        Ok(None)
    }

    async fn transform(
        &self,
        _ctx: rolldown::plugin::SharedTransformPluginContext,
        args: &rolldown::plugin::HookTransformArgs<'_>,
    ) -> rolldown::plugin::HookTransformReturn {
        // Interop fix on rolldown's OWN runtime module: __toESM must return
        // __esModule modules (our wrapper Proxies claim it) unchanged instead
        // of copying them into a dead namespace. A supported hook on a stable
        // module — not regex over the final bundle.
        if args.id.contains("rolldown/runtime") {
            let mut code = args.code.to_string();
            let header = "__toESM = (mod, isNodeMode, target) => (";
            if let Some(pos) = code.find(header) {
                let open = pos + header.len() - 1;
                let close = find_paren_close(&code, open);
                code.insert(close, ')');
                code.insert_str(open + 1, "mod && mod.__esModule ? mod : (");
            }
            return Ok(Some(rolldown::plugin::HookTransformOutput {
                code: Some(code),
                ..Default::default()
            }));
        }
        // Per-file context prologue: rollup evaluates scope-hoisted ESM
        // EAGERLY, so each test-file module sets its own registration context
        // (the entry's per-file protocol runs too late for that).
        if args.id.contains(".test.t") {
            let file_id = args
                .id
                .strip_prefix(&format!("{}/", self.project_root))
                .unwrap_or(args.id);
            let prologue = format!(
                "globalThis.__currentTestFile = '{file_id}';\nglobalThis.__currentTestFilePath = '{}';\n",
                args.id
            );
            return Ok(Some(rolldown::plugin::HookTransformOutput {
                code: Some(format!("{prologue}{}", args.code)),
                ..Default::default()
            }));
        }
        Ok(None)
    }
}

/// Bundle result with optional parsed source map (coverage mode).
pub struct RolldownBundle {
    /// Prelude-prefixed bundle, ready to eval (and cache).
    pub code: String,
    /// The chunk WITHOUT the prelude — coverage instruments this (the __cov
    /// header must precede all counters) and prepends `prelude` afterwards.
    pub chunk: String,
    /// The generated externals prelude (core + per-external globals).
    pub prelude: String,
    pub source_map: Option<sourcemap::SourceMap>,
}

fn build_options(entry: &Path, cwd: &Path, externals: Vec<String>, sourcemap: bool) -> BundlerOptions {
    let ext = Arc::new(externals);
    let is_external = rolldown_common::IsExternal::Fn(Some(Arc::new(move |spec, _imp, _res| {
        let ext = Arc::clone(&ext);
        let spec = spec.to_string();
        Box::pin(async move {
            Ok(ext.iter().any(|e| {
                if let Some(prefix) = e.strip_suffix('*') {
                    spec.starts_with(prefix)
                } else if e == "hermes-test" {
                    // exact only: hermes-test/store is aliased and BUNDLED
                    spec == *e
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

    // Async downleveled (es2016): the harness settles promises via synchronous
    // drains; RTK Query chains require lowered async — probed on both
    // bundlers, this is the twin of esbuild's --supported:async-await=false.
    let transform = BundlerTransformOptions {
        target: Some(rolldown_common::Either::Left("es2016".to_string())),
        ..Default::default()
    };

    // esbuild loader parity: images resolve to empty modules; RN-ecosystem
    // .js files may contain JSX.
    let mut mt: rustc_hash::FxHashMap<String, rolldown_common::ModuleType> = Default::default();
    for ext in ["png", "jpg", "jpeg", "gif", "svg", "webp", "ttf", "otf", "mp4", "riv"] {
        mt.insert(ext.to_string(), rolldown_common::ModuleType::Empty);
        mt.insert(format!(".{ext}"), rolldown_common::ModuleType::Empty);
    }
    mt.insert("js".to_string(), rolldown_common::ModuleType::Jsx);

    BundlerOptions {
        module_types: Some(mt),
        input: Some(vec![InputItem {
            name: Some("bundle".to_string()),
            import: entry.to_string_lossy().to_string(),
        }]),
        cwd: Some(cwd.to_path_buf()),
        format: Some(OutputFormat::Iife),
        external: Some(is_external),
        define: Some(define),
        transform: Some(transform),
        // esbuild-looseness parity: TS codebases routinely value-import types
        // (esbuild silently drops unknown named imports). Shim them to
        // undefined instead of failing the build.
        shim_missing_exports: Some(true),
        sourcemap: sourcemap.then_some(SourceMapType::Inline),
        ..Default::default()
    }
}

/// Bundle `entry` into a single prelude-prefixed IIFE chunk.
pub fn bundle_via_rolldown(
    entry: &Path,
    cwd: &Path,
    file_wrappers: HashMap<String, String>,
    text_wrappers: HashMap<String, String>,
    externals: Vec<String>,
    aliases: Vec<(String, String)>,
    sourcemap: bool,
) -> Result<RolldownBundle, String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime: {e}"))?;

    rt.block_on(async move {
        let options = build_options(entry, cwd, externals, sourcemap);
        let resolve_anchor = cwd.to_string_lossy().to_string();
        let project_root = resolve_anchor.clone();
        let plugins: Vec<SharedPluginable> = vec![Arc::new(HtMockResolver {
            file_wrappers,
            text_wrappers,
            aliases,
            resolve_anchor,
            project_root,
        })];
        let mut bundler = Bundler::with_plugins(options, plugins)
            .map_err(|e| format!("rolldown bundler construction: {e:?}"))?;
        let out = bundler.generate().await.map_err(|e| {
            format!(
                "rolldown bundle:\n{}",
                e.into_vec()
                    .iter()
                    .map(|d| d.to_diagnostic().to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })?;

        for asset in &out.assets {
            if let Output::Chunk(chunk) = asset {
                let mut source_map = None;
                let mut code = chunk.code.clone();
                if sourcemap {
                    let marker = "//# sourceMappingURL=data:application/json;base64,";
                    if let Some(pos) = code.rfind(marker) {
                        let b64 = code[pos + marker.len()..].trim().to_string();
                        code.truncate(pos);
                        if let Ok(decoded) = base64_decode(&b64) {
                            source_map = sourcemap::SourceMap::from_reader(&decoded[..]).ok();
                        }
                    }
                }
                let imports: Vec<String> =
                    chunk.imports.iter().map(|i| i.to_string()).collect();
                let prelude = build_prelude(&code, &imports);
                return Ok(RolldownBundle {
                    code: format!("{prelude}{code}"),
                    chunk: code,
                    prelude,
                    source_map,
                });
            }
        }
        Err("rolldown produced no chunk".to_string())
    })
}

fn base64_decode(input: &str) -> Result<Vec<u8>, ()> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in input.as_bytes() {
        if b == b'=' || b == b'\n' || b == b'\r' {
            continue;
        }
        let val = TABLE.iter().position(|&c| c == b).ok_or(())? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

/// High-level bundling for a test run: assembles wrapper maps, externals
/// (config + natives, hermes-test exact-only), and aliases (config shims +
/// tsconfig + the hermes-test/store bundling alias) from the mock set.
pub fn bundle_tests(
    entry_path: &Path,
    root: &Path,
    shim_cfg: &super::BundleConfig,
    pm: &super::PluginMockSet,
    sourcemap: bool,
) -> Result<RolldownBundle, String> {
    let file_w: HashMap<String, String> =
        pm.file_wrappers.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let text_w: HashMap<String, String> =
        pm.text_wrappers.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let mut externals = pm.external_mocks.clone();
    externals.extend(shim_cfg.externals.iter().cloned());
    externals.push("hermes-test".to_string());
    let mut aliases: Vec<(String, String)> =
        shim_cfg.aliases.iter().map(|(a, t)| (a.clone(), t.clone())).collect();
    let store = root.join("node_modules/hermes-test/src/store.ts");
    if store.exists() {
        aliases.insert(0, ("hermes-test/store".to_string(), store.to_string_lossy().to_string()));
    }
    bundle_via_rolldown(entry_path, root, file_w, text_w, externals, aliases, sourcemap)
}

/// Prepend the store-surface import to a generated entry: hermes-test/store
/// is aliased to the real source file and BUNDLED; its namespace extends the
/// 'hermes-test' import surface (setupApiStore, withStore, ...).
pub fn entry_with_store_surface(entry: String) -> String {
    format!(
        "import * as __htStoreNS from 'hermes-test/store';\nglobalThis.__HT_storeSurface = __htStoreNS;\n{entry}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hermes::Runtime;
    use std::fs;

    /// The mock-delivery primitive end to end: a Rust resolve_id hook
    /// redirects an import to a wrapper, the IIFE output executes in real
    /// (V1) Hermes, and the wrapper's value comes out.
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
        fs::write(dir.join("analytics.js"), "export function greet(n) { return 'REAL:' + n; }\n")
            .unwrap();
        fs::write(
            dir.join("analytics.wrapper.js"),
            "export function greet(n) { return 'WRAPPED:' + n; }\n",
        )
        .unwrap();

        let mut text = HashMap::new();
        text.insert(
            "./analytics.js".to_string(),
            dir.join("analytics.wrapper.js").to_string_lossy().to_string(),
        );

        let b = bundle_via_rolldown(
            &dir.join("entry.js"),
            &dir,
            HashMap::new(),
            text,
            vec![],
            vec![],
            false,
        )
        .unwrap();
        assert!(b.code.contains("WRAPPED"), "wrapper not delivered:\n{}", b.code);
        assert!(!b.code.contains("REAL:"), "real module should be tree-shaken");

        let rt = Runtime::new().expect("hermes runtime");
        let program = format!("{}\nglobalThis.result;", b.code);
        let result = rt.eval(&program, "rolldown-smoke.js").expect("hermes eval");
        assert!(result.contains("WRAPPED:hermes"), "hermes execution: got {result}");

        let _ = fs::remove_dir_all(&dir);
    }
}
