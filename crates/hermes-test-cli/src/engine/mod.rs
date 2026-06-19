/// Engine abstraction for hermes-test.
///
/// Supports multiple JavaScript engines behind a common trait.
/// V8 is the default (full Intl, cross-platform). Hermes is available
/// via --engine hermes for engine parity with React Native production.

#[cfg(feature = "hermes-engine")]
pub mod hermes;
pub mod v8_engine;

/// A JavaScript engine that can evaluate source code.
pub trait Engine {
    /// Evaluate JavaScript source code. Returns the result as a string,
    /// or an error message if evaluation failed.
    fn eval(&self, source: &str, url: &str) -> Result<String, String>;

    /// Evaluate raw bytes as JavaScript source.
    fn eval_bytes(&self, source: &[u8], url: &str) -> Result<String, String> {
        let source_str = std::str::from_utf8(source)
            .map_err(|e| format!("Invalid UTF-8 source: {e}"))?;
        self.eval(source_str, url)
    }
}

/// Which engine to use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineKind {
    V8,
    #[cfg(feature = "hermes-engine")]
    Hermes,
}

impl std::fmt::Display for EngineKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineKind::V8 => write!(f, "v8"),
            #[cfg(feature = "hermes-engine")]
            EngineKind::Hermes => write!(f, "hermes"),
        }
    }
}

impl std::str::FromStr for EngineKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "v8" => Ok(EngineKind::V8),
            #[cfg(feature = "hermes-engine")]
            "hermes" => Ok(EngineKind::Hermes),
            #[cfg(not(feature = "hermes-engine"))]
            "hermes" => Err("Hermes engine not available — rebuild with --features hermes-engine".into()),
            other => Err(format!("Unknown engine: {other}. Available: v8, hermes")),
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
            EngineKind::V8 => Ok(Box::new(v8_engine::V8Runtime::new()?) as Box<dyn Engine>),
            #[cfg(feature = "hermes-engine")]
            EngineKind::Hermes => Ok(Box::new(hermes::HermesRuntime::new()?) as Box<dyn Engine>),
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
        EngineKind::V8 => false,
        #[cfg(feature = "hermes-engine")]
        EngineKind::Hermes => true,
    }
}
