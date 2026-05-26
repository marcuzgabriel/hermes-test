//! Istanbul-compatible coverage instrumentation.
//!
//! Post-bundle instrumentation: the bundler produces a perfect bundle, then we add
//! coverage counters via string insertion. Uses Istanbul's function declaration pattern
//! so counters work in esbuild-hoisted function declarations.
//!
//! Only instruments user code — skips node_modules and hermes-test harness.

use std::collections::BTreeMap;

use oxc::allocator::Allocator;
use oxc::ast::ast::*;
use oxc::parser::Parser;
use oxc::span::{GetSpan, SourceType};

/// Instrument a JS bundle with Istanbul-compatible coverage counters.
/// Returns (instrumented_source, coverage_map_json).
pub fn instrument_bundle(source: &str, filename: &str) -> Option<(String, String)> {
    let allocator = Allocator::default();
    let source_type = SourceType::mjs();
    let ret = Parser::new(&allocator, source, source_type).parse();
    if ret.panicked {
        return None;
    }

    // Build user-code inclusion ranges from esbuild module comments.
    let user_ranges = build_user_ranges(source);

    let mut stmts: Vec<(u32, u32, bool)> = Vec::new();
    let mut fns: Vec<(u32, u32, String)> = Vec::new();
    let mut fn_body_starts: Vec<Option<u32>> = Vec::new();
    let mut branches: Vec<(u32, u32, String, u32)> = Vec::new();

    for stmt in &ret.program.body {
        walk_stmt(stmt, &mut stmts, &mut fns, &mut fn_body_starts, &mut branches, false);
    }

    // Filter to user code only
    let is_user = |offset: u32| -> bool {
        user_ranges.is_empty() || user_ranges.iter().any(|(s, e)| offset >= *s && offset < *e)
    };
    stmts.retain(|(s, _, _)| is_user(*s));
    // fns and fn_body_starts are parallel — filter together
    let keep: Vec<bool> = fns.iter().map(|(s, _, _)| is_user(*s)).collect();
    {
        let mut i = 0;
        fns.retain(|_| { let k = keep[i]; i += 1; k });
    }
    {
        let mut i = 0;
        fn_body_starts.retain(|_| { let k = keep[i]; i += 1; k });
    }
    branches.retain(|(s, _, _, _)| is_user(*s));

    if stmts.is_empty() && fns.is_empty() {
        return Some((source.to_string(), "{}".to_string()));
    }

    let file_id = filename.replace('\\', "/");
    let lt = LineTable::new(source);
    let stmt_spans: Vec<(u32, u32)> = stmts.iter().map(|(s, e, _)| (*s, *e)).collect();
    let stmt_map = build_loc_map(&stmt_spans, &lt);
    let fn_map = build_fn_map(&fns, &lt);
    let branch_map = build_branch_map(&branches, &lt);
    let coverage_map = format!(
        "{{\"{file_id}\":{{\"statementMap\":{stmt_map},\"fnMap\":{fn_map},\"branchMap\":{branch_map}}}}}"
    );
    let s_init = build_zero_map(stmts.len());
    let f_init = build_zero_map(fns.len());
    let b_init = build_branch_zero_map(&branches);

    // Istanbul pattern: hoisted function declaration with cache.
    // Works in esbuild-hoisted function declarations (unlike var which gets split).
    let var_name = "__cov";
    let preamble = format!(
        "function {var_name}(){{var c={var_name}.__c;if(c)return c;var g=globalThis;if(!g.__coverage__)g.__coverage__={{}};if(!g.__coverage__[\"{file_id}\"])g.__coverage__[\"{file_id}\"]={{path:\"{file_id}\",s:{s_init},f:{f_init},b:{b_init}}};{var_name}.__c=g.__coverage__[\"{file_id}\"];return {var_name}.__c}}\n"
    );

    // Find insertion point: after the opening of the IIFE `(() => {`
    let iife_body_start = find_iife_body_start(source);

    let mut insertions: Vec<(u32, String)> = Vec::new();
    // Insert preamble inside the IIFE (not before it)
    insertions.push((iife_body_start, preamble));

    for (i, (start, end, bare)) in stmts.iter().enumerate() {
        if *bare {
            insertions.push((*start, format!("{{{var_name}().s[{i}]++;")));
            insertions.push((*end, "}".to_string()));
        } else {
            insertions.push((*start, format!("{var_name}().s[{i}]++;")));
        }
    }
    for (i, offset) in fn_body_starts.iter().enumerate() {
        if let Some(pos) = offset {
            insertions.push((*pos, format!("{var_name}().f[{i}]++;")));
        }
    }

    insertions.sort_by_key(|i| i.0);
    let mut merged: Vec<(u32, String)> = Vec::new();
    for (offset, code) in insertions {
        if let Some(last) = merged.last_mut() {
            if last.0 == offset {
                last.1.push_str(&code);
                continue;
            }
        }
        merged.push((offset, code));
    }

    let mut result = String::with_capacity(source.len() + merged.len() * 20);
    let mut last = 0usize;
    for (offset, code) in &merged {
        let pos = *offset as usize;
        if pos >= last && pos <= source.len() {
            result.push_str(&source[last..pos]);
            result.push_str(code);
            last = pos;
        }
    }
    result.push_str(&source[last..]);

    Some((result, coverage_map))
}

/// Find byte offset right after esbuild's IIFE opening `(() => {`
fn find_iife_body_start(source: &str) -> u32 {
    // esbuild format=iife starts with `(() => {\n` or `(function() {\n`
    if let Some(pos) = source.find("(() => {") {
        return (pos + 8) as u32; // after `{`
    }
    if let Some(pos) = source.find("(function() {") {
        return (pos + 13) as u32;
    }
    0 // fallback: beginning of file
}

/// Build byte ranges for user code (non-vendor) sections.
fn build_user_ranges(source: &str) -> Vec<(u32, u32)> {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    let mut current_user_start: Option<u32> = None;
    let mut offset = 0u32;

    for line in source.split('\n') {
        if line.starts_with("  // ") && !line.starts_with("  // @") {
            let path = line[5..].trim();
            let is_module_marker = (path.starts_with("../") || path.starts_with("src/") || path.starts_with("./"))
                && (path.ends_with(".js") || path.ends_with(".ts") || path.ends_with(".tsx")
                    || path.ends_with(".mjs") || path.ends_with(".cjs") || path.ends_with(".jsx"));
            if is_module_marker {
                let is_user = !path.contains("node_modules") && !path.contains("hermes-test/src/")
                    && !path.contains(".hermes-test-entry");
                if let Some(start) = current_user_start {
                    ranges.push((start, offset));
                    current_user_start = None;
                }
                if is_user {
                    current_user_start = Some(offset);
                }
            }
        }
        offset += line.len() as u32 + 1;
    }
    if let Some(start) = current_user_start {
        ranges.push((start, source.len() as u32));
    }
    ranges
}

// --- AST walker ---

fn is_bare(stmt: &Statement) -> bool {
    !matches!(stmt, Statement::BlockStatement(_))
}

fn walk_stmt(stmt: &Statement, stmts: &mut Vec<(u32, u32, bool)>, fns: &mut Vec<(u32, u32, String)>, fbo: &mut Vec<Option<u32>>, br: &mut Vec<(u32, u32, String, u32)>, bare: bool) {
    let span = stmt.span();
    if span.start == span.end { return; }
    match stmt {
        Statement::BlockStatement(b) => { for s in &b.body { walk_stmt(s, stmts, fns, fbo, br, false); } }
        Statement::FunctionDeclaration(f) => {
            let name = f.id.as_ref().map(|id| id.name.to_string()).unwrap_or("anonymous".into());
            fns.push((span.start, span.end, name));
            if let Some(body) = &f.body { fbo.push(Some(body.span.start + 1)); for s in &body.statements { walk_stmt(s, stmts, fns, fbo, br, false); } } else { fbo.push(None); }
        }
        Statement::ClassDeclaration(cls) => { stmts.push((span.start, span.end, bare)); walk_class(&cls.body, stmts, fns, fbo, br); }
        Statement::IfStatement(i) => { stmts.push((span.start, span.end, bare)); br.push((span.start, span.end, "if".into(), 2)); walk_stmt(&i.consequent, stmts, fns, fbo, br, is_bare(&i.consequent)); if let Some(alt) = &i.alternate { walk_stmt(alt, stmts, fns, fbo, br, is_bare(alt)); } }
        Statement::ForStatement(f) => { stmts.push((span.start, span.end, bare)); walk_stmt(&f.body, stmts, fns, fbo, br, is_bare(&f.body)); }
        Statement::ForInStatement(f) => { stmts.push((span.start, span.end, bare)); walk_stmt(&f.body, stmts, fns, fbo, br, is_bare(&f.body)); }
        Statement::ForOfStatement(f) => { stmts.push((span.start, span.end, bare)); walk_stmt(&f.body, stmts, fns, fbo, br, is_bare(&f.body)); }
        Statement::WhileStatement(w) => { stmts.push((span.start, span.end, bare)); walk_stmt(&w.body, stmts, fns, fbo, br, is_bare(&w.body)); }
        Statement::DoWhileStatement(d) => { stmts.push((span.start, span.end, bare)); walk_stmt(&d.body, stmts, fns, fbo, br, is_bare(&d.body)); }
        Statement::SwitchStatement(sw) => { stmts.push((span.start, span.end, bare)); br.push((span.start, span.end, "switch".into(), sw.cases.len() as u32)); for case in &sw.cases { for s in &case.consequent { walk_stmt(s, stmts, fns, fbo, br, false); } } }
        Statement::TryStatement(t) => { stmts.push((span.start, span.end, bare)); for s in &t.block.body { walk_stmt(s, stmts, fns, fbo, br, false); } if let Some(h) = &t.handler { for s in &h.body.body { walk_stmt(s, stmts, fns, fbo, br, false); } } if let Some(f) = &t.finalizer { for s in &f.body { walk_stmt(s, stmts, fns, fbo, br, false); } } }
        Statement::ExpressionStatement(e) => { stmts.push((span.start, span.end, bare)); walk_expr(&e.expression, stmts, fns, fbo, br); }
        Statement::VariableDeclaration(v) => { stmts.push((span.start, span.end, bare)); for d in &v.declarations { if let Some(init) = &d.init { walk_expr(init, stmts, fns, fbo, br); } } }
        Statement::ReturnStatement(r) => { stmts.push((span.start, span.end, bare)); if let Some(arg) = &r.argument { walk_expr(arg, stmts, fns, fbo, br); } }
        Statement::ThrowStatement(_) | Statement::BreakStatement(_) | Statement::ContinueStatement(_) | Statement::LabeledStatement(_) => { stmts.push((span.start, span.end, bare)); }
        _ => {}
    }
}

fn walk_expr(expr: &Expression, stmts: &mut Vec<(u32, u32, bool)>, fns: &mut Vec<(u32, u32, String)>, fbo: &mut Vec<Option<u32>>, br: &mut Vec<(u32, u32, String, u32)>) {
    match expr {
        Expression::CallExpression(call) => { walk_expr(&call.callee, stmts, fns, fbo, br); for arg in &call.arguments { if let Argument::SpreadElement(s) = arg { walk_expr(&s.argument, stmts, fns, fbo, br); } else if let Some(e) = arg.as_expression() { walk_expr(e, stmts, fns, fbo, br); } } }
        Expression::ArrowFunctionExpression(arrow) => { let span = arrow.span; fns.push((span.start, span.end, "(arrow)".into())); if arrow.expression { fbo.push(None); for s in &arrow.body.statements { if let Statement::ExpressionStatement(e) = s { walk_expr(&e.expression, stmts, fns, fbo, br); } } } else { fbo.push(Some(arrow.body.span.start + 1)); for s in &arrow.body.statements { walk_stmt(s, stmts, fns, fbo, br, false); } } }
        Expression::FunctionExpression(func) => { let span = func.span; let name = func.id.as_ref().map(|id| id.name.to_string()).unwrap_or("(anonymous)".into()); fns.push((span.start, span.end, name)); if let Some(body) = &func.body { fbo.push(Some(body.span.start + 1)); for s in &body.statements { walk_stmt(s, stmts, fns, fbo, br, false); } } else { fbo.push(None); } }
        Expression::ConditionalExpression(c) => { br.push((c.span.start, c.span.end, "cond-expr".into(), 2)); walk_expr(&c.test, stmts, fns, fbo, br); walk_expr(&c.consequent, stmts, fns, fbo, br); walk_expr(&c.alternate, stmts, fns, fbo, br); }
        Expression::LogicalExpression(l) => { walk_expr(&l.left, stmts, fns, fbo, br); walk_expr(&l.right, stmts, fns, fbo, br); }
        Expression::AssignmentExpression(a) => { walk_expr(&a.right, stmts, fns, fbo, br); }
        Expression::SequenceExpression(s) => { for e in &s.expressions { walk_expr(e, stmts, fns, fbo, br); } }
        Expression::ObjectExpression(o) => { for prop in &o.properties { if let ObjectPropertyKind::ObjectProperty(p) = prop { walk_expr(&p.value, stmts, fns, fbo, br); } } }
        Expression::ArrayExpression(a) => { for el in &a.elements { if let ArrayExpressionElement::SpreadElement(s) = el { walk_expr(&s.argument, stmts, fns, fbo, br); } else if let Some(e) = el.as_expression() { walk_expr(e, stmts, fns, fbo, br); } } }
        Expression::ClassExpression(cls) => { walk_class(&cls.body, stmts, fns, fbo, br); }
        Expression::ParenthesizedExpression(p) => { walk_expr(&p.expression, stmts, fns, fbo, br); }
        Expression::NewExpression(n) => { walk_expr(&n.callee, stmts, fns, fbo, br); for arg in &n.arguments { if let Argument::SpreadElement(s) = arg { walk_expr(&s.argument, stmts, fns, fbo, br); } else if let Some(e) = arg.as_expression() { walk_expr(e, stmts, fns, fbo, br); } } }
        Expression::TemplateLiteral(t) => { for e in &t.expressions { walk_expr(e, stmts, fns, fbo, br); } }
        Expression::TaggedTemplateExpression(t) => { walk_expr(&t.tag, stmts, fns, fbo, br); }
        Expression::UnaryExpression(u) => { walk_expr(&u.argument, stmts, fns, fbo, br); }
        Expression::BinaryExpression(b) => { walk_expr(&b.left, stmts, fns, fbo, br); walk_expr(&b.right, stmts, fns, fbo, br); }
        _ => {}
    }
}

fn walk_class(body: &ClassBody, stmts: &mut Vec<(u32, u32, bool)>, fns: &mut Vec<(u32, u32, String)>, fbo: &mut Vec<Option<u32>>, br: &mut Vec<(u32, u32, String, u32)>) {
    for el in &body.body {
        if let ClassElement::MethodDefinition(m) = el { let name = if let PropertyKey::StaticIdentifier(id) = &m.key { id.name.to_string() } else { "(method)".into() }; fns.push((m.span.start, m.span.end, name)); if let Some(body) = &m.value.body { fbo.push(Some(body.span.start + 1)); for s in &body.statements { walk_stmt(s, stmts, fns, fbo, br, false); } } else { fbo.push(None); } }
        if let ClassElement::PropertyDefinition(p) = el { if let Some(val) = &p.value { walk_expr(val, stmts, fns, fbo, br); } }
    }
}

// --- Helpers ---

struct LineTable { line_starts: Vec<u32> }
impl LineTable {
    fn new(source: &str) -> Self { let mut starts = vec![0u32]; for (i, ch) in source.bytes().enumerate() { if ch == b'\n' { starts.push(i as u32 + 1); } } Self { line_starts: starts } }
    fn lookup(&self, offset: u32) -> (u32, u32) { let line = match self.line_starts.binary_search(&offset) { Ok(i) => i, Err(i) => i.saturating_sub(1) }; (line as u32 + 1, offset.saturating_sub(self.line_starts[line])) }
}

fn build_loc_map(locs: &[(u32, u32)], lt: &LineTable) -> String { let mut o = String::from("{"); for (i, (s, e)) in locs.iter().enumerate() { let (sl, sc) = lt.lookup(*s); let (el, ec) = lt.lookup(*e); if i > 0 { o.push(','); } o.push_str(&format!("\"{i}\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}")); } o.push('}'); o }
fn build_fn_map(fns: &[(u32, u32, String)], lt: &LineTable) -> String { let mut o = String::from("{"); for (i, (s, e, name)) in fns.iter().enumerate() { let (sl, sc) = lt.lookup(*s); let (el, ec) = lt.lookup(*e); if i > 0 { o.push(','); } let n = name.replace('"', "\\\""); o.push_str(&format!("\"{i}\":{{\"name\":\"{n}\",\"decl\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}}}")); } o.push('}'); o }
fn build_branch_map(branches: &[(u32, u32, String, u32)], lt: &LineTable) -> String { let mut o = String::from("{"); for (i, (s, e, btype, _)) in branches.iter().enumerate() { let (sl, sc) = lt.lookup(*s); let (el, ec) = lt.lookup(*e); if i > 0 { o.push(','); } o.push_str(&format!("\"{i}\":{{\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"type\":\"{btype}\"}}")); } o.push('}'); o }
fn build_zero_map(count: usize) -> String { let mut o = String::from("{"); for i in 0..count { if i > 0 { o.push(','); } o.push_str(&format!("\"{i}\":0")); } o.push('}'); o }
fn build_branch_zero_map(branches: &[(u32, u32, String, u32)]) -> String { let mut o = String::from("{"); for (i, (_, _, _, paths)) in branches.iter().enumerate() { if i > 0 { o.push(','); } let z: Vec<&str> = (0..*paths).map(|_| "0").collect(); o.push_str(&format!("\"{i}\":[{}]", z.join(","))); } o.push('}'); o }

// --- Coverage collection ---

pub fn collect_coverage(rt: &crate::hermes::Runtime, map_path: Option<&std::path::Path>) -> Option<String> {
    let js = r#"(function(){var c=globalThis.__coverage__;if(!c)return'null';return JSON.stringify(c)})()"#;
    let raw = rt.eval(js, "coverage-collect").ok()?;
    let json_str: String = serde_json::from_str(&raw).unwrap_or(raw.clone());
    if json_str == "null" || json_str.is_empty() { return None; }
    let mut coverage: serde_json::Value = serde_json::from_str(&json_str).ok()?;
    if let Some(path) = map_path {
        if let Ok(map_str) = std::fs::read_to_string(path) {
            if let Ok(map) = serde_json::from_str::<serde_json::Value>(&map_str) {
                if let (Some(cov_obj), Some(map_obj)) = (coverage.as_object_mut(), map.as_object()) {
                    for (filename, map_data) in map_obj {
                        if let Some(fc) = cov_obj.get_mut(filename) {
                            if let (Some(fc_obj), Some(md)) = (fc.as_object_mut(), map_data.as_object()) {
                                for (key, val) in md { fc_obj.insert(key.clone(), val.clone()); }
                            }
                        }
                    }
                }
            }
        }
    }
    Some(coverage_to_lcov(&coverage))
}

fn coverage_to_lcov(coverage: &serde_json::Value) -> String {
    let mut lcov = String::new();
    let obj = match coverage.as_object() { Some(o) => o, None => return lcov };
    for (filename, fc) in obj {
        lcov.push_str(&format!("SF:{filename}\n"));
        if let Some(fm) = fc["fnMap"].as_object() { for (_, fd) in fm { lcov.push_str(&format!("FN:{},{}\n", fd["decl"]["start"]["line"].as_u64().unwrap_or(0), fd["name"].as_str().unwrap_or("?"))); } if let Some(f) = fc["f"].as_object() { for (id, c) in f { if let Some(fd) = fm.get(id) { lcov.push_str(&format!("FNDA:{},{}\n", c.as_u64().unwrap_or(0), fd["name"].as_str().unwrap_or("?"))); } } } }
        if let Some(sm) = fc["statementMap"].as_object() { if let Some(s) = fc["s"].as_object() { let mut lines: BTreeMap<u64, u64> = BTreeMap::new(); for (id, st) in sm { let l = st["start"]["line"].as_u64().unwrap_or(0); let c = s.get(id).and_then(|v| v.as_u64()).unwrap_or(0); let e = lines.entry(l).or_insert(0); *e = (*e).max(c); } for (l, c) in &lines { lcov.push_str(&format!("DA:{l},{c}\n")); } } }
        if let Some(bm) = fc["branchMap"].as_object() { if let Some(b) = fc["b"].as_object() { for (id, bd) in bm { if let Some(cs) = b.get(id).and_then(|v| v.as_array()) { let l = bd["loc"]["start"]["line"].as_u64().unwrap_or(0); for (j, c) in cs.iter().enumerate() { lcov.push_str(&format!("BRDA:{l},{id},{j},{}\n", c.as_u64().unwrap_or(0))); } } } } }
        lcov.push_str("end_of_record\n");
    }
    lcov
}
