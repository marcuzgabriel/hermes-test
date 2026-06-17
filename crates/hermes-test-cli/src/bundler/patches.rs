/// Patch esbuild's __require to route externalized modules through __HT_mocks.
/// Simple approach: replace the "throw" line inside __require with a registry lookup.
pub fn inject_mock_require_shim(code: &str) -> String {
    // esbuild's __require contains this exact throw statement for unsupported externals:
    //   throw Error('Dynamic require of "' + x + '" is not supported');
    // Replace it with a __HT_mocks lookup.
    // Return a Proxy for externalized modules so that properties added later
    // via mock() are visible even though import destructuring already ran.
    // This solves the ESM import hoisting problem: `import {X} from 'mod'` runs
    // before `mock('mod', () => ({X: ...}))` but the Proxy delegates reads
    // to the live mock registry entry.
    // esbuild may use different variable names (x, x2, etc.) depending on version.
    let throw_re = regex::Regex::new(
        r#"throw Error\('Dynamic require of "' \+ (\w+) \+ '" is not supported'\)"#
    ).unwrap();

    if !throw_re.is_match(&code) {
        // No dynamic require in bundle — no externalized modules, shim not needed. Silent.
        return code.to_string();
    }
    // Hoist __noop outside __require so it's created once, not per call.
    // Proxy-based noop: any property access, function call, or `new` returns __HT_noop,
    // enabling infinite chains like `noop.foo().bar.baz()` without throwing.
    let code = code.replacen(
        "throw Error('Dynamic require",
        "var __HT_noop_fn = function(){}; var __HT_noop = typeof Proxy !== 'undefined' ? new Proxy(__HT_noop_fn, { get: function(t,p) { if (p === Symbol.toPrimitive) return function() { return ''; }; if (p === 'valueOf' || p === 'toJSON') return function() { return 0; }; if (p === 'toString' || p === 'toLocaleString') return function() { return ''; }; if (p === Symbol.iterator) return function() { return { next: function() { return { done: true }; } }; }; if (p === 'length' || p === 'size') return 0; if (p === 'then' || p === '$$typeof' || p === '_isAMomentObject' || p === '__esModule') return undefined; if (p === 'constructor') return Object; if (typeof p === 'symbol') return undefined; return __HT_noop; }, apply: function() { return __HT_noop; }, construct: function() { return {}; }, ownKeys: function(t) { return Object.getOwnPropertyNames(t); }, getOwnPropertyDescriptor: function(t, p) { return Object.getOwnPropertyDescriptor(t, p) || { configurable: true, enumerable: false, writable: true, value: undefined }; } }) : function() {}; throw Error('Dynamic require",
        1,
    );

    throw_re.replace(&code, |caps: &regex::Captures| {
        let v = &caps[1];
        // Proxy with get trap that checks per-file mocks first, then global mocks.
        // Per-file mocks: __HT_file_mocks[__currentTestFile][path] — set by mock()
        // Global mocks: __HT_mocks[path] — fallback for backward compat
        // The Proxy's get trap checks per-file mocks first (for mock isolation),
        // then global mocks. For aliased mocks, __require receives the resolved path
        // (e.g. "/abs/src/hooks") but mock() registers under the original path
        // (e.g. "@scope/pkg/hooks"). __HT_mock_aliases maps resolved → original.
        format!(
            r#"{{ var __r = globalThis.__HT_mocks || (globalThis.__HT_mocks = {{}}); var __k = {v}.replace(/^\.\//, ''); var __t = __r[{v}] || __r[__k] || __r['./' + __k] || {{}}; return typeof Proxy !== 'undefined' ? new Proxy(__t, {{ get: function(t,p) {{ if (p === Symbol.toPrimitive || p === 'then' || p === '$$typeof') return undefined; if (p === '__esModule') return true; var __fm = globalThis.__HT_file_mocks; var __cf = globalThis.__currentTestFile; var __pf = __fm && __cf && __fm[__cf]; var __al = globalThis.__HT_mock_aliases || {{}}; var __orig = __al[{v}] || __al[__k]; var __m = (__pf && (__pf[{v}] || __pf[__k] || __pf['./' + __k] || (__orig && __pf[__orig]))) || __r[{v}] || __r[__k] || __r['./' + __k]; if (p === 'default') {{ var __d = __m && __m['default']; return __d !== undefined ? __d : (__m || t); }} var val = __m ? __m[p] : t[p]; return val !== undefined ? val : __HT_noop; }}, apply: function() {{ return __HT_noop; }}, construct: function() {{ return {{}}; }}, ownKeys: function(t) {{ return Object.getOwnPropertyNames(t); }}, getOwnPropertyDescriptor: function(t, p) {{ return Object.getOwnPropertyDescriptor(t, p) || {{ configurable: true, enumerable: false, writable: true, value: undefined }}; }} }}) : __t }}"#,
        )
    }).to_string()
}

/// Hoist mock() calls before init_*() calls in esbuild's bundled output.
/// Hoist mock() calls before init_*() / require() calls so that when a module's
/// initializer runs (e.g. `const { dispatch, getState } = store`), the mock is already
/// registered in __HT_file_mocks and the shadow-wrapper Proxy returns the mock value.
pub fn hoist_mock_modules(code: &str) -> String {
    // Pattern: (0, import_hermes_test.mock)("path", () => ({ ... }));
    // or: (0, import_hermes_test2.mock)("path", () => ({ ... }));
    // We need to find these, extract them, and move them before init_*() calls.

    let mut result = String::with_capacity(code.len());
    let bytes = code.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // Process each __commonJS or __esm block that contains test files
    // Look for the function body pattern: `"filename.test.ts"(exports) {` or `"filename.test.ts"() {`
    while i < len {
        // Find test file function bodies inside __commonJS/__esm
        // Pattern: "something.test.ts"(exports) { or "something.test.ts"() {
        if let Some(pos) = code[i..].find(".test.ts\"(") {
            let abs_pos = i + pos;
            // Find the opening brace of this function body
            if let Some(brace_offset) = code[abs_pos..].find('{') {
                let body_start = abs_pos + brace_offset + 1;
                // Find the end of this function body by counting braces
                let body_end = find_matching_brace(code, body_start);
                if body_end > body_start {
                    // Copy everything up to body_start
                    result.push_str(&code[i..body_start]);

                    // Process this function body: extract mock() calls and hoist them
                    let body = &code[body_start..body_end];
                    let hoisted = hoist_mocks_in_body(body);
                    if std::env::var("HT_DEBUG_BUNDLE").is_ok() && hoisted != body {
                        eprintln!("[HT_HOIST] Modified body at offset {abs_pos} (body len: {})", body.len());
                    }
                    result.push_str(&hoisted);

                    i = body_end;
                    continue;
                }
            }
            // Couldn't process, copy up to and past this match
            result.push_str(&code[i..abs_pos + pos + 10]);
            i = abs_pos + pos + 10;
        } else {
            // No more test file blocks
            result.push_str(&code[i..]);
            break;
        }
    }

    result
}

/// Find the position of the matching closing brace for an opening brace at `start`.
/// `start` should be the position right after the opening `{`.
/// Returns the position of the closing `}`.
pub fn find_matching_brace(code: &str, start: usize) -> usize {
    let bytes = code.as_bytes();
    let mut depth = 1;
    let mut j = start;
    while j < bytes.len() && depth > 0 {
        match bytes[j] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return j;
                }
            }
            b'"' | b'\'' | b'`' => {
                // Skip string literals
                let quote = bytes[j];
                j += 1;
                while j < bytes.len() {
                    if bytes[j] == b'\\' {
                        j += 1; // skip escaped char
                    } else if bytes[j] == quote {
                        break;
                    }
                    j += 1;
                }
            }
            b'/' => {
                // Skip comments
                if j + 1 < bytes.len() {
                    if bytes[j + 1] == b'/' {
                        // Line comment
                        while j < bytes.len() && bytes[j] != b'\n' {
                            j += 1;
                        }
                        continue;
                    } else if bytes[j + 1] == b'*' {
                        // Block comment
                        j += 2;
                        while j + 1 < bytes.len() {
                            if bytes[j] == b'*' && bytes[j + 1] == b'/' {
                                j += 1;
                                break;
                            }
                            j += 1;
                        }
                    }
                }
            }
            _ => {}
        }
        j += 1;
    }
    j
}

/// Extract a balanced parenthesized expression starting at `start` (position of opening paren).
/// Returns the position after the closing paren (including trailing semicolon/newline).
fn extract_call_end(code: &str, start: usize) -> usize {
    let bytes = code.as_bytes();
    let mut depth = 0;
    let mut j = start;
    while j < bytes.len() {
        match bytes[j] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    j += 1;
                    // Skip trailing semicolon and newline
                    while j < bytes.len() && (bytes[j] == b';' || bytes[j] == b'\n' || bytes[j] == b'\r' || bytes[j] == b' ') {
                        j += 1;
                    }
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

/// Within a single function body, move init_*() calls for non-hermes modules to AFTER
/// the last mock() call. This ensures that:
/// 1. Variable declarations (mockDispatch, mockGetState etc.) execute before mock() factories
/// 2. mock() registers its mock values before modules initialize (init_*() runs)
/// 3. When a module's initializer captures values like `const { dispatch } = store`, the mock is live
///
/// Strategy: "push init_* calls down" rather than "pull mock() calls up".
/// This preserves the relative order of variable declarations and mock() calls.
pub fn hoist_mocks_in_body(body: &str) -> String {
    // Find all ht.mock() calls to determine if hoisting is needed
    // After bundling, ht.mock("path", ...) stays as-is (ht is a global)
    let mock_pattern = "ht.mock(";
    if !body.contains(mock_pattern) {
        if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
            eprintln!("[HOIST_BODY] no mock calls found in body (len={})", body.len());
        }
        return body.to_string();
    }
    if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
        eprintln!("[HOIST_BODY] found mock calls, body len={}", body.len());
    }

    // Find the last ht.mock() call's end position
    let mut last_mock_end = 0;
    let mut search_start = 0;
    while let Some(pos) = body[search_start..].find(mock_pattern) {
        let abs_pos = search_start + pos;
        // "ht.mock(" — the opening paren is at the end of the pattern
        let outer_call_start = abs_pos + mock_pattern.len() - 1;
        let outer_end = extract_call_end(body, outer_call_start);
        // Extend to include trailing semicolon and newline
        let mut end = outer_end;
        let bytes = body.as_bytes();
        if end < bytes.len() && bytes[end] == b';' { end += 1; }
        if end < bytes.len() && bytes[end] == b'\n' { end += 1; }
        if end > last_mock_end { last_mock_end = end; }
        search_start = outer_end;
    }

    if last_mock_end == 0 {
        if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
            eprintln!("[HOIST_BODY] last_mock_end=0, no mocks found");
        }
        return body.to_string();
    }

    if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
        eprintln!("[HOIST_BODY] last_mock_end={}", last_mock_end);
    }

    // Find init_*() calls that appear BEFORE last_mock_end and are not init_hermes*
    // These need to be moved to AFTER last_mock_end.
    // Note: Rust regex crate doesn't support lookahead, so we filter out hermes* manually.
    // Pattern: `      init_SomeName();\n` (with leading whitespace)
    let init_re = match regex::Regex::new(r"(?m)^([ \t]*)(init_\w+)\(\);?\n?") {
        Ok(re) => re,
        Err(_) => return body.to_string(),
    };

    // Collect init_* ranges that are before last_mock_end and are not hermes-test internals
    let mut init_ranges: Vec<(usize, usize, &str)> = Vec::new();
    for m in init_re.find_iter(body) {
        // Skip hermes-test internal inits like init_hermes_test
        let text = m.as_str().trim_start();
        if text.starts_with("init_hermes") { continue; }
        if m.start() < last_mock_end {
            init_ranges.push((m.start(), m.end(), m.as_str()));
        }
    }

    if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
        eprintln!("[HOIST_BODY] init_ranges count={}", init_ranges.len());
        for (s, e, t) in &init_ranges {
            eprintln!("[HOIST_BODY]   init at {}..{}: {:?}", s, e, t.trim());
        }
    }

    if init_ranges.is_empty() {
        return body.to_string();
    }

    // Rebuild body: copy everything, skipping init_* calls before last_mock_end,
    // then insert the collected init_* calls right after last_mock_end.
    let mut result = String::with_capacity(body.len() + 64);
    let mut pos = 0;
    let mut collected_inits = String::new();

    for &(start, end, text) in &init_ranges {
        result.push_str(&body[pos..start]);
        collected_inits.push_str(text);
        if !text.ends_with('\n') { collected_inits.push('\n'); }
        pos = end;
    }

    // Copy up to last_mock_end (might include some content after the last init_* we skipped)
    // We need to handle the case where last_mock_end > pos
    result.push_str(&body[pos..last_mock_end]);

    // Insert collected init_* calls after all mock() calls
    result.push_str(&collected_inits);

    // Copy the rest
    result.push_str(&body[last_mock_end..]);

    result
}

/// Find the insertion point for hoisted mocks in a function body.
/// Mocks must go AFTER the hermes-test require (so mockModule is defined)
/// but BEFORE any init_*() calls (so mocks are registered before modules load).
#[allow(dead_code)]
pub fn find_mock_insert_point(body: &str) -> usize {
    // Strategy: find the end of the hermes-test require line, insert after it.
    let hermes_patterns = [
        r#"__require("hermes-test")"#,
    ];
    let mut after_require = 0;
    for pat in &hermes_patterns {
        if let Some(pos) = body.find(pat) {
            // Find end of this statement (next semicolon + newline)
            let rest = &body[pos..];
            if let Some(semi) = rest.find(";\n") {
                let candidate = pos + semi + 2; // after ";\n"
                if candidate > after_require {
                    after_require = candidate;
                }
            } else if let Some(nl) = rest.find('\n') {
                let candidate = pos + nl + 1;
                if candidate > after_require {
                    after_require = candidate;
                }
            }
        }
    }

    // If we found a hermes require, also skip past any init_hermes* calls
    if after_require > 0 {
        let init_re = regex::Regex::new(r"(?m)^[ \t]*init_hermes\w*\(\);?\n?").unwrap();
        let mut pos = after_require;
        while let Some(m) = init_re.find(&body[pos..]) {
            if m.start() == 0 || body[pos..pos + m.start()].trim().is_empty() {
                pos += m.end();
            } else {
                break;
            }
        }
        return pos;
    }

    // Fallback: find first non-hermes init_*() call
    let init_re = regex::Regex::new(r"(?m)^[ \t]*(init_\w+)\(\)").unwrap();
    for m in init_re.find_iter(body) {
        let matched = m.as_str().trim();
        if !matched.starts_with("init_hermes") {
            return m.start();
        }
    }

    0
}

/// Patch esbuild's runtime helpers for Hermes compatibility + mockability.
///
/// Previously contained 4 patches for Hermes bugs and esbuild compatibility.
/// All removed — modern Hermes (RN 0.85+) has fixed the underlying issues,
/// and the shadow wrapper Proxy system handles mocking without esbuild patches.
pub fn patch_esbuild_for_hermes(code: &str) -> String {
    code.to_string()
}
