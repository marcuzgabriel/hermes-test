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
            // Couldn't process, copy up to and past this match (`.test.ts"(` is 10 bytes;
            // `abs_pos` already includes `pos`).
            let skip_to = (abs_pos + 10).min(len);
            result.push_str(&code[i..skip_to]);
            i = skip_to;
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
/// Returns the position of the closing `}`; if the body is unterminated (or the scanner
/// loses sync) it returns `code.len()` — never past the end, so callers can slice safely.
///
/// Skips string literals, line/block comments AND regex literals. A regex literal such as
/// `/[<>:"\/]/` contains a quote that must not open a phantom string — before this was
/// handled, one such literal in a test body derailed the scan to the end of the bundle and
/// the `j += 1` after the string loop produced `len + 1` → slice panic (`patches.rs:76`).
pub fn find_matching_brace(code: &str, start: usize) -> usize {
    let bytes = code.as_bytes();
    let len = bytes.len();
    let mut depth = 1;
    let mut j = start;
    // Last significant (non-whitespace) byte seen, for the regex-vs-division decision.
    let mut prev_sig: u8 = b'{';
    // Bytes of the last identifier seen, so `return /re/` and `typeof /re/` read as regex.
    let mut last_ident_start: usize = usize::MAX;
    let mut last_ident_end: usize = 0;

    while j < len && depth > 0 {
        let c = bytes[j];
        match c {
            b'{' => {
                depth += 1;
                prev_sig = c;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return j;
                }
                prev_sig = c;
            }
            b'"' | b'\'' | b'`' => {
                // Skip string literals
                let quote = c;
                j += 1;
                while j < len {
                    if bytes[j] == b'\\' {
                        j += 1; // skip escaped char
                    } else if bytes[j] == quote {
                        break;
                    }
                    j += 1;
                }
                prev_sig = quote;
            }
            b'/' => {
                if j + 1 < len && bytes[j + 1] == b'/' {
                    // Line comment
                    while j < len && bytes[j] != b'\n' {
                        j += 1;
                    }
                    continue;
                } else if j + 1 < len && bytes[j + 1] == b'*' {
                    // Block comment
                    j += 2;
                    while j + 1 < len {
                        if bytes[j] == b'*' && bytes[j + 1] == b'/' {
                            j += 1;
                            break;
                        }
                        j += 1;
                    }
                } else if regex_can_start_here(prev_sig, code, last_ident_start, last_ident_end) {
                    // Regex literal: skip to the unescaped closing `/` that is not inside a
                    // character class, then past the flags.
                    j += 1;
                    let mut in_class = false;
                    while j < len {
                        let r = bytes[j];
                        if r == b'\\' {
                            j += 1;
                        } else if r == b'[' {
                            in_class = true;
                        } else if r == b']' {
                            in_class = false;
                        } else if r == b'/' && !in_class {
                            break;
                        } else if r == b'\n' {
                            // Unterminated regex on this line — give up on it, it was division.
                            break;
                        }
                        j += 1;
                    }
                    while j + 1 < len && bytes[j + 1].is_ascii_alphabetic() {
                        j += 1;
                    }
                    prev_sig = b')'; // a regex literal is a value, like a closed expression
                } else {
                    prev_sig = c; // division operator
                }
            }
            b' ' | b'\t' | b'\n' | b'\r' => {}
            _ => {
                if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
                    if last_ident_end != j {
                        last_ident_start = j;
                    }
                    last_ident_end = j + 1;
                }
                prev_sig = c;
            }
        }
        j += 1;
    }
    j.min(len)
}

/// Decide whether a `/` at the current position starts a regex literal (vs. division),
/// from the previous significant byte and the previous identifier.
fn regex_can_start_here(prev_sig: u8, code: &str, ident_start: usize, ident_end: usize) -> bool {
    // After an identifier / number / closing bracket / string, `/` is division —
    // unless the identifier is a keyword that takes an expression.
    let after_value = prev_sig.is_ascii_alphanumeric()
        || prev_sig == b'_'
        || prev_sig == b'$'
        || prev_sig == b')'
        || prev_sig == b']'
        || prev_sig == b'"'
        || prev_sig == b'\''
        || prev_sig == b'`';
    if !after_value {
        return true;
    }
    if ident_start != usize::MAX && ident_end <= code.len() && ident_start < ident_end {
        // The identifier must be the thing immediately before `/` (prev_sig was its last byte).
        let ident = &code[ident_start..ident_end];
        if prev_sig == code.as_bytes()[ident_end - 1] {
            return matches!(
                ident,
                "return" | "typeof" | "instanceof" | "in" | "of" | "new" | "delete" | "void"
                    | "throw" | "case" | "do" | "else" | "yield" | "await"
            );
        }
    }
    false
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


/// Find the index of the `)` matching the `(` at `open`.
/// Skips string literals and comments (same discipline as find_matching_brace).
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
            b'/' => {
                if j + 1 < bytes.len() {
                    if bytes[j + 1] == b'/' {
                        while j < bytes.len() && bytes[j] != b'\n' {
                            j += 1;
                        }
                        continue;
                    } else if bytes[j + 1] == b'*' {
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

/// Rewrite esbuild's runtime helpers for mockability.
///
/// Since the V1 engine (static_h line, RN 0.84+), these are NOT engine-bug
/// workarounds anymore: native classes, for-let-of closures, TDZ, and native
/// super all work (probed July 2026; the class downleveler — old patch 4 —
/// was deleted then). What remains is the mocking feature set: esbuild emits
/// non-configurable export getters and a Proxy-destroying __toESM copy, both
/// of which would make mock() impossible regardless of engine.
///
/// Real bundles contain MULTIPLE copies of the helpers: dependencies shipped
/// as pre-bundled CJS (react-redux et al.) inline their own set, renamed with
/// a numeric suffix (__copyProps2, ...) and indented deeper. Every patch must
/// match all occurrences (fixture 04-duplicated-nested-helpers).
pub fn patch_esbuild_for_hermes(code: &str) -> String {
    // Patch 1: add configurable:true to __copyProps getters so mock() can
    // redefine re-exported bindings. The loop itself is untouched — the
    // engine's for-let-of semantics are correct now.
    let copyprops_re = regex::Regex::new(
        r"(__defProp\d*)\(to, key, \{ get: \(\) => from\[key\], enumerable: !\(desc = (__getOwnPropDesc\d*)\(from, key\)\) \|\| desc\.enumerable \}\);"
    ).unwrap();
    let code = copyprops_re.replace_all(code, |c: &regex::Captures| {
        let (dp, gopd) = (&c[1], &c[2]);
        format!(
            "{dp}(to, key, {{ get: () => from[key], enumerable: !(desc = {gopd}(from, key)) || desc.enumerable, configurable: true }});"
        )
    }).to_string();

    // Patch 2: Make __export configurable for useMock (all copies).
    let code = code.replace(
        "{ get: all[name], enumerable: true }",
        "{ get: all[name], enumerable: true, configurable: true }",
    );

    // Patch 3: Make __toESM return mock Proxies directly (skip copy).
    // Our __require returns Proxies with __esModule=true for externalized modules.
    // __toESM normally copies properties into a new object, which destroys Proxy behavior.
    // Fix: insert early return at the start of every __toESM variant.
    // Note: esbuild may rename `mod` to `mod2`, `mod3` etc. to avoid conflicts.
    let code = {
        let toesm_re = regex::Regex::new(r"var __toESM\d* = \((\w+), isNodeMode, target\) => \(").unwrap();
        let headers: Vec<(usize, String)> = toesm_re
            .captures_iter(&code)
            .map(|c| (c.get(0).unwrap().end(), c[1].to_string()))
            .collect();
        let mut code = code;
        // Patch from last to first so earlier byte offsets stay valid.
        for (hdr_end, mod_var) in headers.iter().rev() {
            let open = hdr_end - 1; // the `(` opening the arrow body
            let close = find_paren_close(&code, open);
            code.insert(close, ')');
            code.insert_str(open + 1, &format!("{mod_var} && {mod_var}.__esModule ? {mod_var} : ("));
        }
        code
    };

    // (Old patch 4 — downleveling every `class extends` to Reflect.construct
    // functions, ~400 lines of regex transpiler — was DELETED July 2026: the
    // V1 engine handles classes natively. See fixtures/class-extends/, which
    // now guard the ENGINE's class semantics instead of a transform.)

    code
}
