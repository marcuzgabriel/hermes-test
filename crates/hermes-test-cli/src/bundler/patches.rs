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

/// Downlevel ALL `class Foo extends Expr { ... }` patterns to function-based constructors.
///
/// Hermes has two bugs with class-extends:
/// 1. TDZ bug: `class X extends Variable` crashes when Variable is a local/parameter
///    (e.g. zod v4: `class Definition extends Parent {}` where Parent = params?.Parent ?? Object)
/// 2. Native super bug: `super()` in `class X extends Array` discards the return value
///
/// Fix: replace every class-extends with Reflect.construct-based function.
/// Same pattern as Babel's `_wrapNativeSuper` and SWC's `_wrap_native_super`.
///
/// Handles three forms:
/// - Assignment with named class: `Name = class InternalName extends Expr {`
/// - Assignment with anonymous class: `var Name = class extends Expr {`
/// - Class declaration: `class Name extends Expr {` (inside function scope)
fn fix_all_class_extends(code: &str) -> String {
    // Collect all matches from three patterns, storing (start, end, var_name, internal_name, parent_expr)
    struct ClassMatch {
        start: usize,
        end: usize,       // end of the full class (including trailing ;)
        var_name: String,
        internal_name: String,
        parent_expr: String,
        body: String,      // everything between the opening { and closing }
    }

    let mut matches: Vec<ClassMatch> = Vec::new();

    // JS identifiers can contain $ — use [\w$] instead of \w
    // Pattern A: `Name = class InternalName extends Expr {`
    let re_a = regex::Regex::new(
        r"([\w$]+)\s*=\s*class\s+([\w$]+)\s+extends\s+([\w$][\w$.]*)\s*\{"
    ).unwrap();

    // Pattern B: `Name = class extends Expr {` (anonymous)
    let re_b = regex::Regex::new(
        r"([\w$]+)\s*=\s*class\s+extends\s+([\w$][\w$.]*)\s*\{"
    ).unwrap();

    // Pattern C: class declaration `class Name extends Expr {` (not preceded by `= `)
    let re_c = regex::Regex::new(
        r"(?:^|[\n;{}\s])class\s+([\w$]+)\s+extends\s+([\w$][\w$.]*)\s*\{"
    ).unwrap();

    // Helper: find matching close brace from position after opening {
    fn find_class_end(code: &str, body_start: usize) -> usize {
        let mut depth = 1;
        for (i, ch) in code[body_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return body_start + i + 1;
                    }
                }
                _ => {}
            }
        }
        code.len()
    }

    // Collect pattern A matches
    for cap in re_a.captures_iter(code) {
        let full = cap.get(0).unwrap();
        let body_start = full.end();
        let mut class_end = find_class_end(code, body_start);
        if class_end < code.len() && code.as_bytes()[class_end] == b';' {
            class_end += 1;
        }
        let body = code[body_start..class_end].to_string();
        matches.push(ClassMatch {
            start: full.start(),
            end: class_end,
            var_name: cap[1].to_string(),
            internal_name: cap[2].to_string(),
            parent_expr: cap[3].to_string(),
            body,

        });
    }

    // Collect pattern B matches (skip if overlaps with pattern A)
    for cap in re_b.captures_iter(code) {
        let full = cap.get(0).unwrap();
        if matches.iter().any(|m| m.start <= full.start() && full.start() < m.end) {
            continue; // Already matched by pattern A
        }
        let body_start = full.end();
        let mut class_end = find_class_end(code, body_start);
        if class_end < code.len() && code.as_bytes()[class_end] == b';' {
            class_end += 1;
        }
        let var_name = cap[1].to_string();
        let body = code[body_start..class_end].to_string();
        matches.push(ClassMatch {
            start: full.start(),
            end: class_end,
            var_name: var_name.clone(),
            internal_name: var_name,
            parent_expr: cap[2].to_string(),
            body,

        });
    }

    // Collect pattern C matches (class declarations, skip overlaps)
    for cap in re_c.captures_iter(code) {
        let full = cap.get(0).unwrap();
        // Adjust start to skip the leading whitespace/newline character
        let actual_start = code[full.start()..full.end()]
            .find("class")
            .map(|i| full.start() + i)
            .unwrap_or(full.start());
        if matches.iter().any(|m| m.start <= actual_start && actual_start < m.end) {
            continue;
        }
        // Also skip if preceded by `= ` (would be pattern A/B)
        if actual_start >= 2 && &code[actual_start - 2..actual_start] == "= " {
            continue;
        }
        let body_start = full.end();
        let mut class_end = find_class_end(code, body_start);
        if class_end < code.len() && code.as_bytes()[class_end] == b';' {
            class_end += 1;
        }
        let var_name = cap[1].to_string();
        let body = code[body_start..class_end].to_string();
        matches.push(ClassMatch {
            start: actual_start,
            end: class_end,
            var_name: var_name.clone(),
            internal_name: var_name,
            parent_expr: cap[2].to_string(),
            body,

        });
    }

    if matches.is_empty() {
        return code.to_string();
    }

    // Sort by start position descending so we can replace from end to start
    matches.sort_by(|a, b| b.start.cmp(&a.start));

    let mut result = code.to_string();

    for m in &matches {
        let var_name = &m.var_name;
        let parent = &m.parent_expr;
        let class_body = &m.body;

        // Parse class body for constructor and methods
        let mut methods = Vec::new();
        let mut constructor_body: Option<(String, String)> = None; // (params, body)
        let mut pos = 0;

        while pos < class_body.len() {
            let remaining = &class_body[pos..];

            // Skip whitespace and closing braces
            if remaining.starts_with('}') || remaining.starts_with('\n') || remaining.starts_with(' ') || remaining.starts_with(';') {
                pos += 1;
                continue;
            }

            // Constructor
            if remaining.starts_with("constructor") {
                let after_kw = &remaining[11..];
                if let Some(paren_start) = after_kw.find('(') {
                    let params_start = 11 + paren_start + 1;
                    // Find matching close paren
                    let mut pd = 1;
                    let mut params_end = pos + params_start;
                    for (i, ch) in class_body[pos + params_start..].char_indices() {
                        match ch {
                            '(' => pd += 1,
                            ')' => { pd -= 1; if pd == 0 { params_end = pos + params_start + i; break; } }
                            _ => {}
                        }
                    }
                    let params = class_body[pos + params_start..params_end].to_string();

                    // Find constructor body
                    if let Some(brace) = class_body[params_end..].find('{') {
                        let body_start = params_end + brace + 1;
                        let mut d = 1;
                        let mut body_end = body_start;
                        for (i, ch) in class_body[body_start..].char_indices() {
                            match ch {
                                '{' => d += 1,
                                '}' => { d -= 1; if d == 0 { body_end = body_start + i; break; } }
                                _ => {}
                            }
                        }
                        let body = class_body[body_start..body_end].to_string();
                        constructor_body = Some((params, body));
                        pos = body_end + 1;
                    } else {
                        pos += 1;
                    }
                } else {
                    pos += 1;
                }
                continue;
            }

            // Static or instance methods
            let is_static = remaining.starts_with("static ");
            let method_remaining = if is_static { &remaining[7..] } else { remaining };

            // Skip Symbol.species getter — we add our own for Array subclasses
            if is_static && method_remaining.starts_with("get [Symbol.species]") {
                if let Some(brace) = remaining.find('{') {
                    let mut d = 1;
                    let start = pos + brace + 1;
                    for (i, ch) in class_body[start..].char_indices() {
                        match ch {
                            '{' => d += 1,
                            '}' => { d -= 1; if d == 0 { pos = start + i + 1; break; } }
                            _ => {}
                        }
                    }
                } else {
                    pos += 1;
                }
                continue;
            }

            // Match: `methodName(` — use paren-matching for params (handles nested parens/braces)
            let method_start_re = regex::Regex::new(r"^([\w$]+)\s*\(").unwrap();
            if let Some(cap) = method_start_re.captures(method_remaining) {
                let name = cap.get(1).unwrap().as_str();
                // Skip JS keywords — these are statements, not method definitions
                if matches!(name, "if" | "for" | "while" | "switch" | "catch" | "with" | "do"
                    | "return" | "throw" | "try" | "new" | "delete" | "typeof" | "void"
                    | "function" | "var" | "let" | "const" | "class") {
                    pos += 1;
                    continue;
                }
                let paren_start = pos + (if is_static { 7 } else { 0 }) + cap.get(0).unwrap().end();
                // Find matching close paren with balanced matching
                let mut pd = 1;
                let mut paren_end = paren_start;
                for (i, ch) in class_body[paren_start..].char_indices() {
                    match ch {
                        '(' => pd += 1,
                        ')' => { pd -= 1; if pd == 0 { paren_end = paren_start + i; break; } }
                        _ => {}
                    }
                }
                let params = &class_body[paren_start..paren_end];
                // After `)`, expect whitespace then `{`
                let after_paren = &class_body[paren_end + 1..];
                let trimmed = after_paren.trim_start();
                if !trimmed.starts_with('{') {
                    pos += 1;
                    continue;
                }
                let brace_offset = paren_end + 1 + (after_paren.len() - trimmed.len()) + 1;
                let abs_body_start = brace_offset;

                let mut d = 1;
                let mut body_end = abs_body_start;
                for (i, ch) in class_body[abs_body_start..].char_indices() {
                    match ch {
                        '{' => d += 1,
                        '}' => { d -= 1; if d == 0 { body_end = abs_body_start + i; break; } }
                        _ => {}
                    }
                }

                let body = &class_body[abs_body_start..body_end];
                let body = body.replace("super.", &format!("{parent}.prototype."));
                // Replace ALL references to internal class name with var_name
                // e.g. _I18n.createInstance → I18n.createInstance, new _Tuple → new Tuple
                let body = if m.internal_name != *var_name {
                    body.replace(&m.internal_name, var_name)
                } else {
                    body
                };

                let target = if is_static { var_name.to_string() } else { format!("{var_name}.prototype") };
                methods.push(format!("{target}.{name} = function({params}) {{{body}}};"));

                pos = body_end + 1;
                continue;
            }

            pos += 1;
        }

        let methods_str = methods.join("\n");

        // Build constructor function
        let constructor_fn = if let Some((params, body)) = &constructor_body {
            // Replace internal class name references with var_name
            // e.g. _Tuple.prototype → Tuple.prototype, new _Tuple → new Tuple
            let body = if m.internal_name != *var_name {
                body.replace(&m.internal_name, var_name)
            } else {
                body.clone()
            };
            // Transform super(...args) → Reflect.construct(Parent, [args], ClassName)
            // and this → _this for property assignments after super()
            // Use paren-matching to handle nested parens in super() args
            let transformed = if let Some(super_pos) = body.find("super(") {
                let args_start = super_pos + 6; // after "super("
                // Find matching close paren
                let mut depth = 1;
                let mut args_end = args_start;
                for (i, ch) in body[args_start..].char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => { depth -= 1; if depth == 0 { args_end = args_start + i; break; } }
                        _ => {}
                    }
                }
                let args = body[args_start..args_end].trim();
                let super_call_end = args_end + 1; // include closing paren
                let reflect_call = if args.is_empty() {
                    format!("var _this = Reflect.construct({parent}, [], new.target || {var_name})")
                } else {
                    format!("var _this = Reflect.construct({parent}, [{args}], new.target || {var_name})")
                };
                format!("{}{}{}", &body[..super_pos], reflect_call, &body[super_call_end..])
            } else {
                // No super call — just wrap body
                body.clone()
            };
            let has_super = body.contains("super(");
            if has_super {
                // Replace `this` as whole word with `_this` (handles this.x, this, etc.)
                let this_re = regex::Regex::new(r"\bthis\b").unwrap();
                let transformed = this_re.replace_all(&transformed, "_this").to_string();
                format!("function {var_name}({params}) {{{transformed}\n    return _this;\n  }}")
            } else {
                // No super call — just use Reflect.construct
                format!("function {var_name}({params}) {{\n    return Reflect.construct({parent}, [], {var_name});\n  }}")
            }
        } else {
            // No constructor — default: forward all args to parent via Reflect.construct
            format!("function {var_name}() {{\n    return Reflect.construct({parent}, Array.prototype.slice.call(arguments), new.target || {var_name});\n  }}")
        };

        // For Array subclasses, add Symbol.species
        let species = if parent == "Array" {
            format!("\n  Object.defineProperty({var_name}, Symbol.species, {{ get: function() {{ return {var_name}; }} }});")
        } else {
            String::new()
        };

        // For class declarations (Pattern C), need `var` since there's no existing assignment
        let is_class_decl = m.start < code.len()
            && code[m.start..].starts_with("class ");
        let decl_prefix = if is_class_decl { "var " } else { "" };

        let replacement = format!(
            r#"{decl_prefix}{var_name} = (function() {{
  {constructor_fn}
  {var_name}.prototype = Object.create({parent}.prototype, {{
    constructor: {{ value: {var_name}, writable: true, configurable: true }}
  }});
  Object.setPrototypeOf({var_name}, {parent});{species}
  {methods_str}
  return {var_name};
}})();"#
        );

        result = format!("{}{}{}", &result[..m.start], replacement, &result[m.end..]);
    }

    result
}

/// Patch esbuild's runtime helpers for Hermes compatibility + mockability.
///
/// Two issues:
/// 1. Hermes bug: `for (let key of arr)` with arrow closures captures by reference,
///    not by value. All getters end up returning the last key's value.
///    Fix: rewrite __copyProps to use var + bind.
///
/// 2. esbuild creates non-configurable ESM namespace getters, making useMock impossible.
///    Fix: add configurable:true to __copyProps and __export.
pub fn patch_esbuild_for_hermes(code: &str) -> String {
    let _original_len = code.len();
    let code_kb = code.len() / 1024;

    // Patch 1: Fix Hermes for-let-of closure bug in __copyProps
    let code = code.replacen(
        "for (let key of __getOwnPropNames(from))\n        if (!__hasOwnProp.call(to, key) && key !== except)\n          __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });",
        "var keys = __getOwnPropNames(from);\n      for (var i = 0; i < keys.length; i++) {\n        var key = keys[i];\n        if (!__hasOwnProp.call(to, key) && key !== except)\n          __defProp(to, key, { get: ((k) => from[k]).bind(null, key), enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable, configurable: true });\n      }",
        1,
    );

    // Patch 2: Make __export configurable for useMock
    let code = code.replacen(
        "{ get: all[name], enumerable: true }",
        "{ get: all[name], enumerable: true, configurable: true }",
        1,
    );

    // Patch 3: Make __toESM return mock Proxies directly (skip copy).
    // Our __require returns Proxies with __esModule=true for externalized modules.
    // __toESM normally copies properties into a new object, which destroys Proxy behavior.
    // Fix: insert early return at the start of __toESM.
    // Note: esbuild may rename `mod` to `mod2`, `mod3` etc. to avoid conflicts.
    let code = {
        let toesm_re = regex::Regex::new(r"var __toESM = \((\w+), isNodeMode, target\) => \(").unwrap();
        if let Some(caps) = toesm_re.captures(&code) {
            let mod_var = caps[1].to_string();
            let patched = toesm_re.replace(&code, |_caps: &regex::Captures| {
                format!("var __toESM = ({mod_var}, isNodeMode, target) => ({mod_var} && {mod_var}.__esModule ? {mod_var} : (")
            }).to_string();
            // Add closing paren for the ternary — right before the final ");".
            patched.replacen(
                &format!("{mod_var}\n  ));"),
                &format!("{mod_var}\n  )));"),
                1,
            )
        } else {
            code
        }
    };

    // Patch 4: Downlevel ALL `class extends Expr` patterns to function-based constructors.
    // Hermes bugs: (1) TDZ crash with `class X extends Variable` when Variable is local,
    // (2) super() in Array subclasses discards return value. Fix both by converting all
    // class-extends to Reflect.construct-based functions.
    let code = fix_all_class_extends(&code);

    let code_str = code;

    // Only warn if the unpatched for-let-of pattern is still present
    if code_str.contains("for (let key of __getOwnPropNames(from))") {
        eprintln!("WARNING: esbuild for-let-of patch did not match ({code_kb}KB bundle) — Hermes closure bug may cause failures");
    }

    code_str
}
