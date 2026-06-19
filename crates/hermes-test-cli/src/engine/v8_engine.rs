/// V8 JavaScript engine backend via deno_core.
///
/// Uses Deno's V8 runtime which includes full ICU data for Intl support
/// (DateTimeFormat, NumberFormat, etc.) on all platforms.

use deno_core::JsRuntime;
use deno_core::RuntimeOptions;

pub struct V8Runtime {
    runtime: std::cell::UnsafeCell<JsRuntime>,
}

unsafe impl Send for V8Runtime {}

impl V8Runtime {
    pub fn new() -> Result<Self, String> {
        let mut runtime = JsRuntime::new(RuntimeOptions::default());

        let setup = deno_core::FastString::from(r#"
            globalThis.print = function(...args) {
                const msg = args.map(a => typeof a === 'string' ? a : JSON.stringify(a)).join(' ');
                Deno.core.print(msg + '\n', false);
            };
            // __HT_drain flushes the microtask/promise queue synchronously.
            // This is critical for React's act() and async test patterns.
            // Deno.core.runMicrotasks() is the V8 equivalent of Hermes's drainMicrotasks().
            // Recursion guard prevents infinite loop: drain → promise callback → act → drain.
            var __HT_draining = false;
            globalThis.__HT_drain = function() {
                if (__HT_draining) return;
                __HT_draining = true;
                try {
                    Deno.core.runMicrotasks();
                } finally {
                    __HT_draining = false;
                }
            };
        "#.to_string());
        runtime.execute_script("[v8-setup]", setup)
            .map_err(|e| format!("Failed to setup V8: {e}"))?;

        Ok(V8Runtime {
            runtime: std::cell::UnsafeCell::new(runtime),
        })
    }
}

impl super::Engine for V8Runtime {
    fn eval(&self, source: &str, url: &str) -> Result<String, String> {
        let runtime = unsafe { &mut *self.runtime.get() };
        let src = deno_core::FastString::from(source.to_string());
        let url_static: &'static str = Box::leak(url.to_string().into_boxed_str());

        // Execute the source in global scope (no wrapping — important for harness)
        if let Err(e) = runtime.execute_script(url_static, src) {
            return Err(format!("{e}"));
        }

        // Flush V8 microtask queue — essential for React's scheduler and act().
        // Multiple checkpoints needed: promise callbacks may queue more microtasks.
        for _ in 0..10 {
            runtime.v8_isolate().perform_microtask_checkpoint();
        }

        // For result extraction (e.g. "globalThis.__HT_results"), the source
        // is a simple expression. Read its value via stderr capture.
        // Check if this looks like a result-reading expression (short, no semicolons)
        let is_expression = source.len() < 200 && !source.contains(';')
            && !source.contains("function") && !source.contains("var ");
        if is_expression {
            let read_code = format!(
                "Deno.core.print((function() {{ \
                    var __r = {source}; \
                    if (__r === undefined || __r === null) return 'null'; \
                    if (typeof __r === 'string') return __r; \
                    return JSON.stringify(__r); \
                }})(), true)"
            );
            let read_src = deno_core::FastString::from(read_code);
            let captured = capture_stderr(|| {
                let _ = runtime.execute_script("[v8-read]", read_src);
            });
            if !captured.is_empty() {
                return Ok(captured);
            }
        }

        Ok("null".into())
    }
}

/// Capture stderr output during closure execution.
fn capture_stderr<F: FnOnce()>(f: F) -> String {
    use std::os::unix::io::AsRawFd;

    let (read_fd, write_fd) = unsafe {
        let mut fds = [0i32; 2];
        libc::pipe(fds.as_mut_ptr());
        (fds[0], fds[1])
    };

    let old_stderr = unsafe { libc::dup(2) };
    unsafe { libc::dup2(write_fd, 2) };

    f();

    // Restore stderr and close write end
    unsafe {
        libc::dup2(old_stderr, 2);
        libc::close(old_stderr);
        libc::close(write_fd);
    }

    // Read captured output (non-blocking)
    unsafe { libc::fcntl(read_fd, libc::F_SETFL, libc::O_NONBLOCK) };
    let mut buf = vec![0u8; 65536];
    let n = unsafe { libc::read(read_fd, buf.as_mut_ptr() as *mut _, buf.len()) };
    unsafe { libc::close(read_fd) };

    if n > 0 {
        String::from_utf8_lossy(&buf[..n as usize]).to_string()
    } else {
        String::new()
    }
}
