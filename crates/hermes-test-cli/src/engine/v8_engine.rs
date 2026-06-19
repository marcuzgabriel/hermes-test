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

fn ensure_v8_initialized() {
    V8_INIT.call_once(|| {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });
}

pub struct V8Runtime {
    isolate: v8::OwnedIsolate,
}

impl V8Runtime {
    pub fn new() -> Result<Self, String> {
        ensure_v8_initialized();
        let isolate = v8::Isolate::new(v8::CreateParams::default());
        Ok(V8Runtime { isolate })
    }
}

impl super::Engine for V8Runtime {
    fn eval(&self, source: &str, url: &str) -> Result<String, String> {
        // V8 requires mutable access to the isolate for scope creation.
        // We use unsafe to cast away the shared ref since we know we're
        // single-threaded within a given runtime instance.
        let isolate = unsafe {
            &mut *(&self.isolate as *const v8::OwnedIsolate as *mut v8::OwnedIsolate)
        };

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

        let script = v8::Script::compile(scope, v8_source, Some(&origin))
            .ok_or_else(|| {
                // Try to get the exception message
                if let Some(exception) = scope.exception() {
                    let msg = exception.to_string(scope)
                        .map(|s| s.to_rust_string_lossy(scope))
                        .unwrap_or_else(|| "Compilation failed".into());
                    return msg;
                }
                "Script compilation failed".to_string()
            })?;

        let try_catch = &mut v8::TryCatch::new(scope);
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
