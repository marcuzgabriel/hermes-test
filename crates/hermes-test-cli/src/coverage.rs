//! Istanbul-compatible coverage instrumentation using OXC.
//!
//! Parses JS source, walks the full AST (including IIFEs, arrow functions,
//! class methods), collects statement/function/branch locations, injects
//! __cov.s[id]++ counters, outputs instrumented JS + lcov.

use std::collections::BTreeMap;
use oxc::allocator::Allocator;
use oxc::parser::Parser;
use oxc::span::{SourceType, GetSpan};
use oxc::ast::ast::*;

/// Instrument a JS bundle with Istanbul-compatible coverage counters.
pub fn instrument_bundle(source: &str, filename: &str) -> Option<String> {
    let allocator = Allocator::default();
    let source_type = SourceType::mjs();
    let ret = Parser::new(&allocator, source, source_type).parse();
    if ret.panicked { return None; }

    let mut stmts: Vec<(u32, u32)> = Vec::new();
    let mut fns: Vec<(u32, u32, String)> = Vec::new();
    let mut fn_body_offsets: Vec<Option<u32>> = Vec::new(); // byte after { of block body
    let mut branches: Vec<(u32, u32, String, u32)> = Vec::new();

    for stmt in &ret.program.body {
        walk_stmt(stmt, source, &mut stmts, &mut fns, &mut fn_body_offsets, &mut branches);
    }

    if stmts.is_empty() && fns.is_empty() {
        return Some(source.to_string());
    }

    let file_id = filename.replace('\\', "/");
    let lt = LineTable::new(source);
    let stmt_map = build_loc_map(&stmts, &lt);
    let s_init = build_zero_map(stmts.len());
    let fn_map = build_fn_map(&fns, &lt);
    let f_init = build_zero_map(fns.len());
    let branch_map = build_branch_map(&branches, &lt);
    let b_init = build_branch_zero_map(&branches);

    let preamble = format!(
        "var __cov=(function(){{var g=globalThis;if(!g.__coverage__)g.__coverage__={{}};if(!g.__coverage__[\"{file_id}\"])g.__coverage__[\"{file_id}\"]={{path:\"{file_id}\",statementMap:{stmt_map},fnMap:{fn_map},branchMap:{branch_map},s:{s_init},f:{f_init},b:{b_init}}};return g.__coverage__[\"{file_id}\"]}})()\n"
    );

    // Collect insertions: (byte_offset, code)
    // Only insert where it's syntactically safe (inside blocks, not bare control bodies)
    let mut insertions: Vec<(u32, String)> = Vec::new();
    for (i, (start, _)) in stmts.iter().enumerate() {
        if is_safe_insert_point(source, *start) && !is_bare_control_body(source, *start) {
            insertions.push((*start, format!("__cov.s[{i}]++;")));
        }
    }
    // fn_body_offsets: byte offset right after opening { of block body (set by walk_expr/walk_stmt)
    for (i, offset) in fn_body_offsets.iter().enumerate() {
        if let Some(pos) = offset {
            insertions.push((*pos, format!("__cov.f[{i}]++;")));
        }
    }

    // Sort ASCENDING by offset for single-pass output
    insertions.sort_by_key(|i| i.0);
    insertions.dedup_by_key(|i| i.0);

    // Build output in one pass — no insert_str reallocation
    let mut result = String::with_capacity(source.len() + insertions.len() * 20 + preamble.len());
    result.push_str(&preamble);
    let mut last = 0usize;
    for (offset, code) in &insertions {
        let pos = *offset as usize;
        if pos >= last && pos <= source.len() {
            result.push_str(&source[last..pos]);
            result.push_str(code);
            last = pos;
        }
    }
    result.push_str(&source[last..]);

    Some(result)
}

// --- AST walker: recurse into everything ---

fn walk_stmt(stmt: &Statement, src: &str, stmts: &mut Vec<(u32, u32)>, fns: &mut Vec<(u32, u32, String)>, fbo: &mut Vec<Option<u32>>, br: &mut Vec<(u32, u32, String, u32)>) {
    let span = stmt.span();
    if span.start == span.end { return; }

    match stmt {
        Statement::BlockStatement(b) => { for s in &b.body { walk_stmt(s, src, stmts, fns, fbo, br); } }
        Statement::FunctionDeclaration(f) => {
            let name = f.id.as_ref().map(|id| id.name.to_string()).unwrap_or("anonymous".into());
            fns.push((span.start, span.end, name));
            if let Some(body) = &f.body {
                fbo.push(Some(body.span.start + 1)); // after {
                for s in &body.statements { walk_stmt(s, src, stmts, fns, fbo, br); }
            } else {
                fbo.push(None);
            }
        }
        Statement::ClassDeclaration(cls) => {
            stmts.push((span.start, span.end));
            walk_class(&cls.body, src, stmts, fns, fbo, br);
        }
        Statement::IfStatement(i) => {
            stmts.push((span.start, span.end));
            br.push((span.start, span.end, "if".into(), 2));
            walk_stmt(&i.consequent, src, stmts, fns, fbo, br);
            if let Some(alt) = &i.alternate { walk_stmt(alt, src, stmts, fns, fbo, br); }
        }
        Statement::ForStatement(f) => { stmts.push((span.start, span.end)); walk_stmt(&f.body, src, stmts, fns, fbo, br); }
        Statement::ForInStatement(f) => { stmts.push((span.start, span.end)); walk_stmt(&f.body, src, stmts, fns, fbo, br); }
        Statement::ForOfStatement(f) => { stmts.push((span.start, span.end)); walk_stmt(&f.body, src, stmts, fns, fbo, br); }
        Statement::WhileStatement(w) => { stmts.push((span.start, span.end)); walk_stmt(&w.body, src, stmts, fns, fbo, br); }
        Statement::DoWhileStatement(d) => { stmts.push((span.start, span.end)); walk_stmt(&d.body, src, stmts, fns, fbo, br); }
        Statement::SwitchStatement(sw) => {
            stmts.push((span.start, span.end));
            br.push((span.start, span.end, "switch".into(), sw.cases.len() as u32));
            for case in &sw.cases { for s in &case.consequent { walk_stmt(s, src, stmts, fns, fbo, br); } }
        }
        Statement::TryStatement(t) => {
            stmts.push((span.start, span.end));
            for s in &t.block.body { walk_stmt(s, src, stmts, fns, fbo, br); }
            if let Some(h) = &t.handler { for s in &h.body.body { walk_stmt(s, src, stmts, fns, fbo, br); } }
            if let Some(f) = &t.finalizer { for s in &f.body { walk_stmt(s, src, stmts, fns, fbo, br); } }
        }
        Statement::ExpressionStatement(e) => {
            stmts.push((span.start, span.end));
            walk_expr(&e.expression, src, stmts, fns, fbo, br);
        }
        Statement::VariableDeclaration(v) => {
            stmts.push((span.start, span.end));
            for d in &v.declarations { if let Some(init) = &d.init { walk_expr(init, src, stmts, fns, fbo, br); } }
        }
        Statement::ReturnStatement(r) => {
            stmts.push((span.start, span.end));
            if let Some(arg) = &r.argument { walk_expr(arg, src, stmts, fns, fbo, br); }
        }
        Statement::ThrowStatement(_) | Statement::BreakStatement(_) |
        Statement::ContinueStatement(_) | Statement::LabeledStatement(_) => {
            stmts.push((span.start, span.end));
        }
        _ => {}
    }
}

fn walk_expr(expr: &Expression, src: &str, stmts: &mut Vec<(u32, u32)>, fns: &mut Vec<(u32, u32, String)>, fbo: &mut Vec<Option<u32>>, br: &mut Vec<(u32, u32, String, u32)>) {
    match expr {
        Expression::CallExpression(call) => {
            walk_expr(&call.callee, src, stmts, fns, fbo, br);
            for arg in &call.arguments {
                if let Argument::SpreadElement(s) = arg { walk_expr(&s.argument, src, stmts, fns, fbo, br); }
                else if let Some(e) = arg.as_expression() { walk_expr(e, src, stmts, fns, fbo, br); }
            }
        }
        Expression::ArrowFunctionExpression(arrow) => {
            let span = arrow.span;
            fns.push((span.start, span.end, "(arrow)".into()));
            if arrow.expression {
                // Concise arrow: () => expr — no block body, can't insert
                fbo.push(None);
            } else {
                fbo.push(Some(arrow.body.span.start + 1)); // after {
            }
            for s in &arrow.body.statements { walk_stmt(s, src, stmts, fns, fbo, br); }
        }
        Expression::FunctionExpression(func) => {
            let span = func.span;
            let name = func.id.as_ref().map(|id| id.name.to_string()).unwrap_or("(anonymous)".into());
            fns.push((span.start, span.end, name));
            if let Some(body) = &func.body {
                fbo.push(Some(body.span.start + 1));
                for s in &body.statements { walk_stmt(s, src, stmts, fns, fbo, br); }
            } else {
                fbo.push(None);
            }
        }
        Expression::ConditionalExpression(c) => {
            br.push((c.span.start, c.span.end, "cond-expr".into(), 2));
            walk_expr(&c.test, src, stmts, fns, fbo, br);
            walk_expr(&c.consequent, src, stmts, fns, fbo, br);
            walk_expr(&c.alternate, src, stmts, fns, fbo, br);
        }
        Expression::LogicalExpression(l) => {
            walk_expr(&l.left, src, stmts, fns, fbo, br);
            walk_expr(&l.right, src, stmts, fns, fbo, br);
        }
        Expression::AssignmentExpression(a) => { walk_expr(&a.right, src, stmts, fns, fbo, br); }
        Expression::SequenceExpression(s) => { for e in &s.expressions { walk_expr(e, src, stmts, fns, fbo, br); } }
        Expression::ObjectExpression(o) => {
            for prop in &o.properties {
                if let ObjectPropertyKind::ObjectProperty(p) = prop { walk_expr(&p.value, src, stmts, fns, fbo, br); }
            }
        }
        Expression::ArrayExpression(a) => {
            for el in &a.elements {
                if let ArrayExpressionElement::SpreadElement(s) = el { walk_expr(&s.argument, src, stmts, fns, fbo, br); }
                else if let Some(e) = el.as_expression() { walk_expr(e, src, stmts, fns, fbo, br); }
            }
        }
        Expression::ClassExpression(cls) => { walk_class(&cls.body, src, stmts, fns, fbo, br); }
        Expression::ParenthesizedExpression(p) => { walk_expr(&p.expression, src, stmts, fns, fbo, br); }
        Expression::NewExpression(n) => {
            walk_expr(&n.callee, src, stmts, fns, fbo, br);
            for arg in &n.arguments {
                if let Argument::SpreadElement(s) = arg { walk_expr(&s.argument, src, stmts, fns, fbo, br); }
                else if let Some(e) = arg.as_expression() { walk_expr(e, src, stmts, fns, fbo, br); }
            }
        }
        Expression::TemplateLiteral(t) => {
            for e in &t.expressions { walk_expr(e, src, stmts, fns, fbo, br); }
        }
        Expression::TaggedTemplateExpression(t) => {
            walk_expr(&t.tag, src, stmts, fns, fbo, br);
        }
        Expression::UnaryExpression(u) => { walk_expr(&u.argument, src, stmts, fns, fbo, br); }
        Expression::BinaryExpression(b) => { walk_expr(&b.left, src, stmts, fns, fbo, br); walk_expr(&b.right, src, stmts, fns, fbo, br); }
        _ => {}
    }
}

fn walk_class(body: &ClassBody, src: &str, stmts: &mut Vec<(u32, u32)>, fns: &mut Vec<(u32, u32, String)>, fbo: &mut Vec<Option<u32>>, br: &mut Vec<(u32, u32, String, u32)>) {
    for el in &body.body {
        if let ClassElement::MethodDefinition(m) = el {
            let name = if let PropertyKey::StaticIdentifier(id) = &m.key { id.name.to_string() } else { "(method)".into() };
            fns.push((m.span.start, m.span.end, name));
            if let Some(body) = &m.value.body {
                fbo.push(Some(body.span.start + 1));
                for s in &body.statements { walk_stmt(s, src, stmts, fns, fbo, br); }
            } else {
                fbo.push(None);
            }
        }
        if let ClassElement::PropertyDefinition(p) = el {
            if let Some(val) = &p.value { walk_expr(val, src, stmts, fns, fbo, br); }
        }
    }
}

// --- Helpers ---

/// Check if inserting a statement at this offset is syntactically safe.
/// Safe = the statement starts at the beginning of a line (only whitespace before it)
/// or right after { or ; followed by optional whitespace.
/// This prevents inserting inside object literals, call arguments, etc.
fn is_safe_insert_point(source: &str, offset: u32) -> bool {
    if offset == 0 { return true; }
    let bytes = source.as_bytes();
    let mut pos = offset as usize;
    while pos > 0 {
        pos -= 1;
        match bytes[pos] {
            b' ' | b'\t' | b'\r' => continue,
            b'\n' => return true,
            b'{' | b';' | b'}' => {
                // Make sure {  is a block brace, not an object literal.
                // Simple heuristic: if { is preceded by ), =>, or keyword, it's a block.
                if bytes[pos] == b'{' {
                    let mut p2 = pos;
                    while p2 > 0 { p2 -= 1; if bytes[p2] != b' ' && bytes[p2] != b'\t' { break; } }
                    match bytes[p2] {
                        b')' | b'>' | b'e' | b'{' | b';' => return true, // ) => else { ;
                        _ => return false, // likely object literal
                    }
                }
                return true;
            }
            _ => return false,
        }
    }
    true
}

/// Check if this statement is a bare body of a control structure (no braces).
/// e.g. `if (x) stmt;` or `else stmt;` — prepending __cov.s++ would steal the body.
fn is_bare_control_body(source: &str, offset: u32) -> bool {
    if offset == 0 { return false; }
    let bytes = source.as_bytes();
    let mut pos = offset as usize;
    // Walk back past whitespace to find what precedes this statement
    while pos > 0 {
        pos -= 1;
        match bytes[pos] {
            b' ' | b'\t' | b'\r' | b'\n' => continue,
            b')' => return true,  // if (...) stmt / for (...) stmt / while (...) stmt
            b'{' | b';' | b'}' => return false,  // inside a block — safe
            b'e' => {
                // Check for "else" keyword
                if pos >= 3 && &source[pos - 3..=pos] == "else" {
                    return true;
                }
                return false;
            }
            _ => return false,
        }
    }
    false
}

fn find_function_body_start(source: &str, start: u32, end: u32) -> Option<u32> {
    let end = (end as usize).min(source.len());
    let slice = &source[start as usize..end];
    let bytes = source.as_bytes();
    let mut paren_depth = 0i32;
    let mut found_arrow = false;
    for (i, ch) in slice.char_indices() {
        match ch {
            '(' => paren_depth += 1,
            ')' => { paren_depth -= 1; found_arrow = false; }
            '=' if i + 1 < slice.len() && slice.as_bytes()[i + 1] == b'>' => {
                found_arrow = true;
            }
            '{' if paren_depth <= 0 => {
                let abs = start as usize + i;
                // Skip ${ (template literal)
                if abs > 0 && bytes[abs - 1] == b'$' { continue; }
                // For arrows: only accept { that directly follows =>
                if found_arrow {
                    // Check: the only thing between => and { should be whitespace
                    let before = &slice[..i];
                    if let Some(arrow_pos) = before.rfind("=>") {
                        let between = &before[arrow_pos + 2..];
                        if between.trim().is_empty() {
                            return Some(start + i as u32 + 1);
                        }
                    }
                    // { is not right after => — it's an object literal in the expression
                    return None;
                }
                // Regular function: first { after ) is the body
                return Some(start + i as u32 + 1);
            }
            _ => {}
        }
    }
    None
}

/// Pre-computed line offset table for O(1) line/col lookups.
struct LineTable { line_starts: Vec<u32> }

impl LineTable {
    fn new(source: &str) -> Self {
        let mut starts = vec![0u32];
        for (i, ch) in source.bytes().enumerate() {
            if ch == b'\n' { starts.push(i as u32 + 1); }
        }
        Self { line_starts: starts }
    }

    fn lookup(&self, offset: u32) -> (u32, u32) {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let col = offset.saturating_sub(self.line_starts[line]);
        (line as u32 + 1, col)
    }
}

fn build_loc_map(locs: &[(u32, u32)], lt: &LineTable) -> String {
    let mut o = String::from("{");
    for (i, (s, e)) in locs.iter().enumerate() {
        let (sl, sc) = lt.lookup(*s);
        let (el, ec) = lt.lookup(*e);
        if i > 0 { o.push(','); }
        o.push_str(&format!("\"{i}\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}"));
    }
    o.push('}'); o
}

fn build_fn_map(fns: &[(u32, u32, String)], lt: &LineTable) -> String {
    let mut o = String::from("{");
    for (i, (s, e, name)) in fns.iter().enumerate() {
        let (sl, sc) = lt.lookup(*s);
        let (el, ec) = lt.lookup(*e);
        if i > 0 { o.push(','); }
        let n = name.replace('"', "\\\"");
        o.push_str(&format!("\"{i}\":{{\"name\":\"{n}\",\"decl\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}}}"));
    }
    o.push('}'); o
}

fn build_branch_map(branches: &[(u32, u32, String, u32)], lt: &LineTable) -> String {
    let mut o = String::from("{");
    for (i, (s, e, btype, _)) in branches.iter().enumerate() {
        let (sl, sc) = lt.lookup(*s);
        let (el, ec) = lt.lookup(*e);
        if i > 0 { o.push(','); }
        o.push_str(&format!("\"{i}\":{{\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"type\":\"{btype}\"}}"));
    }
    o.push('}'); o
}

fn build_zero_map(count: usize) -> String {
    let mut o = String::from("{");
    for i in 0..count { if i > 0 { o.push(','); } o.push_str(&format!("\"{i}\":0")); }
    o.push('}'); o
}

fn build_branch_zero_map(branches: &[(u32, u32, String, u32)]) -> String {
    let mut o = String::from("{");
    for (i, (_, _, _, paths)) in branches.iter().enumerate() {
        if i > 0 { o.push(','); }
        let z: Vec<&str> = (0..*paths).map(|_| "0").collect();
        o.push_str(&format!("\"{i}\":[{}]", z.join(",")));
    }
    o.push('}'); o
}

// --- Coverage collection ---

pub fn collect_coverage(rt: &crate::hermes::Runtime) -> Option<String> {
    let js = r#"(function(){var c=globalThis.__coverage__;if(!c)return'null';return JSON.stringify(c)})()"#;
    let raw = rt.eval(js, "coverage-collect").ok()?;
    let json_str: String = serde_json::from_str(&raw).unwrap_or(raw.clone());
    if json_str == "null" || json_str.is_empty() { return None; }
    let coverage: serde_json::Value = serde_json::from_str(&json_str).ok()?;
    Some(coverage_to_lcov(&coverage))
}

fn coverage_to_lcov(coverage: &serde_json::Value) -> String {
    let mut lcov = String::new();
    let obj = match coverage.as_object() { Some(o) => o, None => return lcov };
    for (filename, fc) in obj {
        lcov.push_str(&format!("SF:{filename}\n"));
        if let Some(fm) = fc["fnMap"].as_object() {
            for (_, fd) in fm { lcov.push_str(&format!("FN:{},{}\n", fd["decl"]["start"]["line"].as_u64().unwrap_or(0), fd["name"].as_str().unwrap_or("?"))); }
            if let Some(f) = fc["f"].as_object() {
                for (id, c) in f { if let Some(fd) = fm.get(id) { lcov.push_str(&format!("FNDA:{},{}\n", c.as_u64().unwrap_or(0), fd["name"].as_str().unwrap_or("?"))); } }
            }
        }
        if let Some(sm) = fc["statementMap"].as_object() {
            if let Some(s) = fc["s"].as_object() {
                let mut lines: BTreeMap<u64, u64> = BTreeMap::new();
                for (id, st) in sm { let l = st["start"]["line"].as_u64().unwrap_or(0); let c = s.get(id).and_then(|v| v.as_u64()).unwrap_or(0); let e = lines.entry(l).or_insert(0); *e = (*e).max(c); }
                for (l, c) in &lines { lcov.push_str(&format!("DA:{l},{c}\n")); }
            }
        }
        if let Some(bm) = fc["branchMap"].as_object() {
            if let Some(b) = fc["b"].as_object() {
                for (id, bd) in bm { if let Some(cs) = b.get(id).and_then(|v| v.as_array()) { let l = bd["loc"]["start"]["line"].as_u64().unwrap_or(0); for (j, c) in cs.iter().enumerate() { lcov.push_str(&format!("BRDA:{l},{id},{j},{}\n", c.as_u64().unwrap_or(0))); } } }
            }
        }
        lcov.push_str("end_of_record\n");
    }
    lcov
}
