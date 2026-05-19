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
}

pub struct Runtime {
    ptr: *mut std::ffi::c_void,
}

impl Runtime {
    pub fn new() -> Result<Self, String> {
        let ptr = unsafe { hermes_create_runtime() };
        if ptr.is_null() {
            return Err("Failed to create Hermes runtime".into());
        }
        Ok(Runtime { ptr })
    }

    pub fn eval(&self, source: &str, url: &str) -> Result<String, String> {
        let c_source = CString::new(source).map_err(|e| format!("Source contains null byte: {e}"))?;
        let c_url = CString::new(url).map_err(|e| format!("URL contains null byte: {e}"))?;
        let mut error_out: *mut c_char = ptr::null_mut();

        let result = unsafe {
            hermes_eval(
                self.ptr,
                c_source.as_ptr(),
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

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe { hermes_destroy_runtime(self.ptr) };
    }
}
