/// V8 JavaScript engine backend via deno_core.
///
/// Uses Deno's V8 runtime which includes full ICU data for Intl support
/// (DateTimeFormat, NumberFormat, etc.) on all platforms.
///
/// Refs:
/// - https://docs.rs/deno_core/latest/deno_core/
/// - https://deno.com/blog/roll-your-own-javascript-runtime

use deno_core::JsRuntime;
use deno_core::RuntimeOptions;

pub struct V8Runtime {
    runtime: std::cell::UnsafeCell<JsRuntime>,
}

// Safety: V8Runtime is used single-threaded within a given test run.
unsafe impl Send for V8Runtime {}

impl V8Runtime {
    pub fn new() -> Result<Self, String> {
        let runtime = JsRuntime::new(RuntimeOptions::default());
        Ok(V8Runtime {
            runtime: std::cell::UnsafeCell::new(runtime),
        })
    }
}

impl super::Engine for V8Runtime {
    fn eval(&self, source: &str, url: &str) -> Result<String, String> {
        let runtime = unsafe { &mut *self.runtime.get() };

        // Install print() global — the harness and tests use print() for output.
        // Deno.core.print is the low-level V8 print function.
        let print_setup = deno_core::FastString::from(r#"
            globalThis.print = function(...args) {
                const msg = args.map(a => typeof a === 'string' ? a : JSON.stringify(a)).join(' ');
                Deno.core.print(msg + '\n', false);
            };
        "#.to_string());
        let setup_name: &'static str = "[print-setup]";
        let _ = runtime.execute_script(setup_name, print_setup);

        let source_owned = deno_core::FastString::from(source.to_string());
        let url_static: &'static str = Box::leak(url.to_string().into_boxed_str());
        match runtime.execute_script(url_static, source_owned) {
            Ok(_) => Ok("null".into()),
            Err(e) => Err(format!("{e}")),
        }
    }
}
