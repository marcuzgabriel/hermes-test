/// V8 JavaScript engine backend via rusty_v8.
///
/// V8 provides full ECMAScript compliance including Intl (DateTimeFormat,
/// NumberFormat, etc.) on all platforms — no platform-specific stubs.
/// V8 isolates are lightweight and can be parallelized across threads.
///
/// Refs:
/// - https://docs.rs/v8/latest/v8/
/// - https://github.com/denoland/rusty_v8

use std::sync::Once;

static V8_INIT: Once = Once::new();

// ICU data for V8 Intl support. Built by build.rs, embedded at compile time.
// Without this, toLocaleDateString/toLocaleString crash with OOM.
#[cfg(feature = "v8-engine")]
static ICU_DATA: &[u8] = include_bytes!(env!("V8_ICU_DATA_PATH"));

fn ensure_v8_initialized() {
    V8_INIT.call_once(|| {
        // Load ICU data for full Intl support (DateTimeFormat, NumberFormat, etc.)
        v8::icu::set_common_data_74(ICU_DATA)
            .expect("Failed to load V8 ICU data");

        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });
}

pub struct V8Runtime {
    isolate: std::cell::UnsafeCell<v8::OwnedIsolate>,
}

// Safety: V8Runtime is used single-threaded within a given test run.
unsafe impl Send for V8Runtime {}

impl V8Runtime {
    pub fn new() -> Result<Self, String> {
        ensure_v8_initialized();
        let params = v8::CreateParams::default()
            .heap_limits(0, 1024 * 1024 * 1024); // 1GB max heap
        let isolate = v8::Isolate::new(params);
        Ok(V8Runtime { isolate: std::cell::UnsafeCell::new(isolate) })
    }
}

impl super::Engine for V8Runtime {
    fn eval(&self, source: &str, url: &str) -> Result<String, String> {
        let isolate = unsafe { &mut *self.isolate.get() };

        let handle_scope = &mut v8::HandleScope::new(isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let scope = &mut v8::ContextScope::new(handle_scope, context);

        // Install print() global — matches Hermes behavior
        let global = context.global(scope);
        let print_key = v8::String::new(scope, "print").unwrap();
        let print_fn = v8::Function::new(scope, |scope: &mut v8::HandleScope,
                                           args: v8::FunctionCallbackArguments,
                                           mut rv: v8::ReturnValue| {
            let mut parts = Vec::new();
            for i in 0..args.length() {
                let arg = args.get(i);
                if let Some(s) = arg.to_string(scope) {
                    parts.push(s.to_rust_string_lossy(scope));
                }
            }
            println!("{}", parts.join(" "));
            rv.set_null();
        }).unwrap();
        global.set(scope, print_key.into(), print_fn.into());

        // Install globalThis
        let global_this_key = v8::String::new(scope, "globalThis").unwrap();
        global.set(scope, global_this_key.into(), global.into());

        // Compile and run
        let v8_source = v8::String::new(scope, source)
            .ok_or_else(|| "Failed to create V8 source string".to_string())?;

        let origin = {
            let name = v8::String::new(scope, url).unwrap();
            v8::ScriptOrigin::new(scope, name.into(), 0, 0, false, -1,
                                  None, false, false, false, None)
        };

        let try_catch = &mut v8::TryCatch::new(scope);

        let script = match v8::Script::compile(try_catch, v8_source, Some(&origin)) {
            Some(s) => s,
            None => {
                let msg = try_catch.exception()
                    .and_then(|e| e.to_string(try_catch))
                    .map(|s| s.to_rust_string_lossy(try_catch))
                    .unwrap_or_else(|| "Script compilation failed".into());
                return Err(msg);
            }
        };

        let result = script.run(try_catch);

        match result {
            Some(value) => {
                if value.is_null_or_undefined() {
                    Ok("null".into())
                } else {
                    let s = value.to_string(try_catch)
                        .map(|s| s.to_rust_string_lossy(try_catch))
                        .unwrap_or_else(|| "undefined".into());
                    Ok(s)
                }
            }
            None => {
                // Exception was thrown
                let exception = try_catch.exception().unwrap();
                let msg = exception.to_string(try_catch)
                    .map(|s| s.to_rust_string_lossy(try_catch))
                    .unwrap_or_else(|| "Unknown JS error".into());

                // Try to get stack trace
                if let Some(stack) = try_catch.stack_trace() {
                    let stack_str = stack.to_string(try_catch)
                        .map(|s| s.to_rust_string_lossy(try_catch))
                        .unwrap_or_default();
                    Err(format!("{msg}\n{stack_str}"))
                } else {
                    Err(msg)
                }
            }
        }
    }
}
