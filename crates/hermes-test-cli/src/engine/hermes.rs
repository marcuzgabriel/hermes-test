/// Hermes JavaScript engine backend via C++ FFI.
///
/// Provides engine parity with React Native production — tests run on
/// the same engine that ships in your app. Requires cmake + ninja +
/// vendor/hermes to be built.
///
/// Limitations:
/// - Intl support varies by platform (full on macOS, stub on Linux)
/// - Requires 4 esbuild patches for Hermes-specific JS quirks
/// - macOS recommended for CI/CD
///
/// Refs:
/// - https://github.com/facebook/hermes
/// - https://github.com/facebook/hermes/blob/main/doc/IntlAPIs.md

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

extern "C" {
    fn hermes_create_runtime() -> *mut std::ffi::c_void;
    fn hermes_eval(
        rt: *mut std::ffi::c_void,
        source: *const c_char,
        source_len: usize,
        source_url: *const c_char,
        error_out: *mut *mut c_char,
    ) -> *mut c_char;
    fn hermes_free_string(s: *mut c_char);
    fn hermes_destroy_runtime(rt: *mut std::ffi::c_void);
    fn hermes_compile(
        source: *const c_char,
        source_len: usize,
        source_url: *const c_char,
        out_bytecode: *mut *mut u8,
        out_size: *mut usize,
        error_out: *mut *mut c_char,
    ) -> i32;
}

pub struct HermesRuntime {
    ptr: *mut std::ffi::c_void,
}

impl HermesRuntime {
    pub fn new() -> Result<Self, String> {
        let ptr = unsafe { hermes_create_runtime() };
        if ptr.is_null() {
            return Err("Failed to create Hermes runtime".into());
        }
        Ok(HermesRuntime { ptr })
    }
}

impl super::Engine for HermesRuntime {
    fn eval(&self, source: &str, url: &str) -> Result<String, String> {
        self.eval_bytes(source.as_bytes(), url)
    }

    fn eval_bytes(&self, source: &[u8], url: &str) -> Result<String, String> {
        let c_url = CString::new(url).map_err(|e| format!("URL contains null byte: {e}"))?;
        let mut error_out: *mut c_char = ptr::null_mut();

        let result = unsafe {
            hermes_eval(
                self.ptr,
                source.as_ptr() as *const c_char,
                source.len(),
                c_url.as_ptr(),
                &mut error_out,
            )
        };

        if result.is_null() {
            let error_msg = if !error_out.is_null() {
                let msg = unsafe { CStr::from_ptr(error_out) }
                    .to_string_lossy()
                    .into_owned();
                unsafe { hermes_free_string(error_out) };
                msg
            } else {
                "Unknown error".into()
            };
            return Err(error_msg);
        }

        let json = unsafe { CStr::from_ptr(result) }
            .to_string_lossy()
            .into_owned();
        unsafe { hermes_free_string(result) };
        Ok(json)
    }
}

/// Compile JS source to Hermes bytecode in-process (no subprocess).
pub fn compile_bytecode(source: &str, url: &str) -> Result<Vec<u8>, String> {
    let c_url = CString::new(url).map_err(|e| format!("URL null byte: {e}"))?;
    let mut out_bytecode: *mut u8 = ptr::null_mut();
    let mut out_size: usize = 0;
    let mut error_out: *mut c_char = ptr::null_mut();

    let rc = unsafe {
        hermes_compile(
            source.as_ptr() as *const c_char,
            source.len(),
            c_url.as_ptr(),
            &mut out_bytecode,
            &mut out_size,
            &mut error_out,
        )
    };

    if rc != 0 {
        let msg = if !error_out.is_null() {
            let s = unsafe { CStr::from_ptr(error_out) }.to_string_lossy().into_owned();
            unsafe { libc::free(error_out as *mut _) };
            s
        } else {
            "Compilation failed".into()
        };
        return Err(msg);
    }

    if out_bytecode.is_null() || out_size == 0 {
        return Err("Empty bytecode".into());
    }

    let bytecode = unsafe { std::slice::from_raw_parts(out_bytecode, out_size) }.to_vec();
    unsafe { libc::free(out_bytecode as *mut _) };
    Ok(bytecode)
}

impl Drop for HermesRuntime {
    fn drop(&mut self) {
        unsafe { hermes_destroy_runtime(self.ptr) };
    }
}
