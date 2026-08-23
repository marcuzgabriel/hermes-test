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

/// Hoist `ht.mock()` calls above `init_*()` / `require_*()` calls inside every test-file module
/// body in esbuild's bundled output.
///
/// esbuild initializes imported modules (`init_x()`) where the `import` statements were — i.e.
/// BEFORE the `ht.mock(...)` calls written below them ever run. A module that captures values at
/// init time (`const { dispatch } = store`) would grab the real thing. So, per test-file body,
/// every `init_*()` statement (except hermes-test's own `init_hermes*`) that appears before the
/// last `ht.mock(...)` statement is moved to just after it.
///
/// Implemented on the OXC AST (already a dependency, already parsing the bundle for coverage):
/// the parser handles strings, template literals, comments and regex literals — the hand-written
/// brace/quote scanner this replaced mis-tokenized `"` inside `/[<>:"\/]/` and could run to the
/// end of the bundle (slice panic at the old `patches.rs:76`).
pub fn hoist_mock_modules(code: &str) -> String {
    use oxc::allocator::Allocator;
    use oxc::ast::ast::*;
    use oxc::ast_visit::{walk, Visit};
    use oxc::parser::Parser;
    use oxc::span::{GetSpan, SourceType};

    // Cheap pre-check: nothing to hoist without a mock call anywhere.
    if !code.contains("ht.mock(") {
        return code.to_string();
    }

    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, code, SourceType::mjs()).parse();
    if !ret.errors.is_empty() {
        eprintln!(
            "Warning: hermes-test could not parse the bundle to hoist ht.mock() calls ({} error(s)); \
             mocks may register after the modules they target initialize. First: {}",
            ret.errors.len(),
            ret.errors[0]
        );
        return code.to_string();
    }

    /// Does this object-literal key name a test file? (`"src/x.test.ts"(exports) {` …)
    fn is_test_file_key(key: &str) -> bool {
        let stem = key.rsplit('/').next().unwrap_or(key);
        let mut parts = stem.rsplitn(3, '.');
        let ext = parts.next().unwrap_or("");
        let kind = parts.next().unwrap_or("");
        matches!(ext, "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs") && matches!(kind, "test" | "spec")
    }

    // (start, end, replacement) edits for each test-file function body (span inside the braces).
    struct Collector<'s> {
        source: &'s str,
        edits: Vec<(usize, usize, String)>,
    }

    impl<'a, 's> Visit<'a> for Collector<'s> {
        fn visit_object_property(&mut self, prop: &ObjectProperty<'a>) {
            let key = match &prop.key {
                PropertyKey::StringLiteral(lit) => Some(lit.value.as_str()),
                PropertyKey::StaticIdentifier(id) => Some(id.name.as_str()),
                _ => None,
            };
            if let (Some(key), Expression::FunctionExpression(func)) = (key, &prop.value) {
                if is_test_file_key(key) {
                    if let Some(body) = &func.body {
                        if let Some(edit) = hoist_in_body(self.source, body) {
                            self.edits.push(edit);
                        }
                        // Test-file bodies don't nest; no need to descend.
                        return;
                    }
                }
            }
            walk::walk_object_property(self, prop);
        }
    }

    /// Compute the rewritten text for one function body, or None when nothing moves.
    fn hoist_in_body(source: &str, body: &FunctionBody<'_>) -> Option<(usize, usize, String)> {
        let body_start = body.span.start as usize + 1; // after `{`
        let body_end = body.span.end as usize - 1; // before `}`
        let text = &source[body_start..body_end];

        let mut last_mock_end: Option<usize> = None; // relative to body_start
        let mut inits: Vec<(usize, usize)> = Vec::new(); // (start, end) relative to body_start

        for stmt in &body.statements {
            let Statement::ExpressionStatement(es) = stmt else { continue };
            let Expression::CallExpression(call) = &es.expression else { continue };
            let span = es.span();
            let (s, e) = ((span.start as usize) - body_start, (span.end as usize) - body_start);
            match &call.callee {
                // ht.mock(...)
                Expression::StaticMemberExpression(m)
                    if m.property.name == "mock"
                        && matches!(&m.object, Expression::Identifier(id) if id.name == "ht") =>
                {
                    last_mock_end = Some(last_mock_end.map_or(e, |p| p.max(e)));
                }
                // init_foo()  (esbuild ESM initializer) — but never hermes-test's own.
                Expression::Identifier(id)
                    if id.name.starts_with("init_")
                        && !id.name.starts_with("init_hermes")
                        && call.arguments.is_empty() =>
                {
                    inits.push((s, e));
                }
                _ => {}
            }
        }

        let last_mock_end = last_mock_end?;
        // Extend the insertion point past a trailing newline so hoisted inits land on their own
        // lines (mirrors the historical text transform).
        let mut insert_at = last_mock_end;
        if text.as_bytes().get(insert_at) == Some(&b'\n') {
            insert_at += 1;
        }
        let moving: Vec<(usize, usize)> = inits.into_iter().filter(|&(s, _)| s < last_mock_end).collect();
        if moving.is_empty() {
            return None;
        }

        // Each moved statement takes its leading indentation and trailing newline with it.
        let ranges: Vec<(usize, usize, &str)> = moving
            .iter()
            .map(|&(s, e)| {
                let bytes = text.as_bytes();
                let mut rs = s;
                while rs > 0 && (bytes[rs - 1] == b' ' || bytes[rs - 1] == b'\t') {
                    rs -= 1;
                }
                let mut re = e;
                if bytes.get(re) == Some(&b'\n') {
                    re += 1;
                }
                (rs, re, &text[rs..re])
            })
            .collect();

        let mut out = String::with_capacity(text.len() + 16);
        let mut collected = String::new();
        let mut pos = 0;
        for &(rs, re, t) in &ranges {
            out.push_str(&text[pos..rs]);
            collected.push_str(t);
            if !t.ends_with('\n') {
                collected.push('\n');
            }
            pos = re;
        }
        out.push_str(&text[pos..insert_at.max(pos)]);
        out.push_str(&collected);
        out.push_str(&text[insert_at.max(pos)..]);
        Some((body_start, body_end, out))
    }

    let mut collector = Collector { source: code, edits: Vec::new() };
    collector.visit_program(&ret.program);
    if collector.edits.is_empty() {
        return code.to_string();
    }
    if std::env::var("HT_DEBUG_BUNDLE").is_ok() {
        eprintln!("[HT_HOIST] rewrote {} test-file bodies", collector.edits.len());
    }

    // Apply edits back-to-front so earlier offsets stay valid.
    let mut edits = collector.edits;
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out = code.to_string();
    for (s, e, replacement) in edits {
        out.replace_range(s..e, &replacement);
    }
    out
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
