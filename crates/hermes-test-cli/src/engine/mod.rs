/// Engine abstraction for hermes-test.
///
/// Supports multiple JavaScript engines behind a common trait.

#[cfg(feature = "hermes-engine")]
pub mod hermes;
#[cfg(feature = "v8-engine")]
pub mod v8_engine;

use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag: whether the current engine needs Hermes-specific esbuild patches.
/// Set once at startup based on --engine flag. Checked by bundler/esbuild.rs.
static USE_HERMES_PATCHES: AtomicBool = AtomicBool::new(false);

pub fn set_hermes_patches_enabled(enabled: bool) {
    USE_HERMES_PATCHES.store(enabled, Ordering::Relaxed);
}

pub fn hermes_patches_enabled() -> bool {
    USE_HERMES_PATCHES.load(Ordering::Relaxed)
}

/// A JavaScript engine that can evaluate source code.
pub trait Engine {
    /// Evaluate JavaScript source code. Returns the result as a string,
    /// or an error message if evaluation failed.
    fn eval(&self, source: &str, url: &str) -> Result<String, String>;

    /// Evaluate raw bytes as JavaScript source.
    fn eval_bytes(&self, source: &[u8], url: &str) -> Result<String, String> {
        let source_str =
            std::str::from_utf8(source).map_err(|e| format!("Invalid UTF-8 source: {e}"))?;
        self.eval(source_str, url)
    }
}

/// Which engine to use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineKind {
    #[cfg(feature = "v8-engine")]
    V8,
    #[cfg(feature = "hermes-engine")]
    Hermes,
}

impl std::fmt::Display for EngineKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "v8-engine")]
            EngineKind::V8 => write!(f, "v8"),
            #[cfg(feature = "hermes-engine")]
            EngineKind::Hermes => write!(f, "hermes"),
        }
    }
}

pub fn default_engine_name() -> &'static str {
    #[cfg(feature = "hermes-engine")]
    {
        return "hermes";
    }
    #[cfg(all(not(feature = "hermes-engine"), feature = "v8-engine"))]
    {
        return "v8";
    }
    #[allow(unreachable_code)]
    "unknown"
}

pub fn available_engines() -> &'static str {
    #[cfg(all(feature = "hermes-engine", feature = "v8-engine"))]
    {
        return "hermes, v8";
    }
    #[cfg(all(feature = "hermes-engine", not(feature = "v8-engine")))]
    {
        return "hermes";
    }
    #[cfg(all(not(feature = "hermes-engine"), feature = "v8-engine"))]
    {
        return "v8";
    }
    #[allow(unreachable_code)]
    "none"
}

impl std::str::FromStr for EngineKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            #[cfg(feature = "v8-engine")]
            "v8" => Ok(EngineKind::V8),
            #[cfg(not(feature = "v8-engine"))]
            "v8" => Err("V8 engine not available — install the V8 platform package".into()),
            #[cfg(feature = "hermes-engine")]
            "hermes" => Ok(EngineKind::Hermes),
            #[cfg(not(feature = "hermes-engine"))]
            "hermes" => {
                Err("Hermes engine not available — install the default platform package".into())
            }
            other => Err(format!(
                "Unknown engine: {other}. Available: {}",
                available_engines()
            )),
        }
    }
}

/// Unified runtime wrapper that handles engine-specific behavior
/// (bytecode compilation for Hermes, direct eval for V8).
pub struct Runtime {
    engine: Box<dyn Engine>,
    kind: EngineKind,
}

impl Runtime {
    pub fn new(kind: EngineKind) -> Result<Self, String> {
        let engine: Box<dyn Engine> = match kind {
            #[cfg(feature = "v8-engine")]
            EngineKind::V8 => {
                let rt = v8_engine::V8Runtime::new()?;
                Ok::<Box<dyn Engine>, String>(Box::new(rt))
            }
            #[cfg(feature = "hermes-engine")]
            EngineKind::Hermes => {
                let rt = hermes::HermesRuntime::new()?;
                Ok::<Box<dyn Engine>, String>(Box::new(rt) as Box<dyn Engine>)
            }
        }?;
        Ok(Runtime { engine, kind })
    }

    pub fn eval(&self, source: &str, url: &str) -> Result<String, String> {
        self.engine.eval(source, url)
    }

    pub fn eval_bytes(&self, source: &[u8], url: &str) -> Result<String, String> {
        self.engine.eval_bytes(source, url)
    }

    /// Compile JS to bytecode (Hermes only). Returns None for V8.
    pub fn compile_bytecode(&self, source: &str, url: &str) -> Option<Result<Vec<u8>, String>> {
        match self.kind {
            #[cfg(feature = "hermes-engine")]
            EngineKind::Hermes => Some(hermes::compile_bytecode(source, url)),
            #[cfg(feature = "v8-engine")]
            _ => None,
        }
    }

    /// Evaluate JS with optional bytecode compilation.
    /// For Hermes: compiles to bytecode first, falls back to source eval.
    /// For V8: evaluates source directly.
    pub fn eval_smart(&self, source: &str, url: &str) -> Result<String, String> {
        if let Some(Ok(bytecode)) = self.compile_bytecode(source, url) {
            self.eval_bytes(&bytecode, &url.replace(".js", ".hbc"))
        } else {
            self.eval(source, url)
        }
    }

    pub fn kind(&self) -> EngineKind {
        self.kind
    }
}

/// Compile JS to Hermes bytecode if available, otherwise return None.
pub fn compile_bytecode_if_hermes(source: &str, url: &str) -> Option<Vec<u8>> {
    #[cfg(feature = "hermes-engine")]
    {
        match hermes::compile_bytecode(source, url) {
            Ok(bc) => Some(bc),
            Err(e) => {
                eprintln!("WARNING: hermesc bytecode compilation failed: {e}");
                None
            }
        }
    }
    #[cfg(not(feature = "hermes-engine"))]
    {
        let _ = (source, url);
        None
    }
}

/// Whether the engine requires Hermes-specific esbuild patches.
pub fn needs_hermes_patches(kind: EngineKind) -> bool {
    match kind {
        #[cfg(feature = "v8-engine")]
        EngineKind::V8 => false,
        #[cfg(feature = "hermes-engine")]
        EngineKind::Hermes => true,
    }
}
