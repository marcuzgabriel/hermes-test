//! Istanbul-compatible coverage instrumentation.
//!
//! Per-file instrumentation BEFORE bundling — each source file gets its own
//! small coverage preamble + counters. Uses the bundler's transform mechanism
//! (same as shadow wrappers) to redirect esbuild to instrumented copies.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxc::allocator::Allocator;
use oxc::ast::ast::*;
use oxc::parser::Parser;
use oxc::span::{GetSpan, SourceType};

/// Instrument a single source file for coverage.
/// Returns the instrumented source with a per-file preamble + counters.
pub fn instrument_file(source: &str, filename: &str, var_name: &str) -> Option<String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(filename).unwrap_or_else(|_| SourceType::mjs());
    let ret = Parser::new(&allocator, source, source_type).parse();
    if ret.panicked {
        return None;
    }

    let mut stmts: Vec<(u32, u32, bool)> = Vec::new();
    let mut fns: Vec<(u32, u32, String)> = Vec::new();
    let mut fn_body_starts: Vec<Option<u32>> = Vec::new();
    let mut branches: Vec<(u32, u32, String, u32)> = Vec::new();

    for stmt in &ret.program.body {
        walk_stmt(stmt, &mut stmts, &mut fns, &mut fn_body_starts, &mut branches, false);
    }

    if stmts.is_empty() && fns.is_empty() {
        return Some(source.to_string());
    }

    let file_id = filename.replace('\\', "/");
    let lt = LineTable::new(source);
    let stmt_spans: Vec<(u32, u32)> = stmts.iter().map(|(s, e, _)| (*s, *e)).collect();
    let stmt_map = build_loc_map(&stmt_spans, &lt);
    let fn_map = build_fn_map(&fns, &lt);
    let branch_map = build_branch_map(&branches, &lt);
    let s_init = build_zero_map(stmts.len());
    let f_init = build_zero_map(fns.len());
    let b_init = build_branch_zero_map(&branches);

    // Per-file preamble — small (only this file's counters + maps)
    let preamble = format!(
        "var {var_name}=(function(){{var g=globalThis;if(!g.__coverage__)g.__coverage__={{}};if(!g.__coverage__[\"{file_id}\"])g.__coverage__[\"{file_id}\"]={{path:\"{file_id}\",statementMap:{stmt_map},fnMap:{fn_map},branchMap:{branch_map},s:{s_init},f:{f_init},b:{b_init}}};return g.__coverage__[\"{file_id}\"]}})();\n"
    );

    // Build insertions
    let mut insertions: Vec<(u32, String)> = Vec::new();
    for (i, (start, end, bare)) in stmts.iter().enumerate() {
        if *bare {
            insertions.push((*start, format!("{{{var_name}.s[{i}]++;")));
            insertions.push((*end, "}".to_string()));
        } else {
            insertions.push((*start, format!("{var_name}.s[{i}]++;")));
        }
    }
    for (i, offset) in fn_body_starts.iter().enumerate() {
        if let Some(pos) = offset {
            insertions.push((*pos, format!("{var_name}.f[{i}]++;")));
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

    let mut result = String::with_capacity(source.len() + merged.len() * 20 + preamble.len());
    result.push_str(&preamble);
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

    Some(result)
}

/// Create instrumented copies of test files for coverage.
/// Uses the same transforms mechanism as shadow wrappers.
/// Returns Vec<(original_path, temp_path)> for the bundler's transform map.
pub fn create_coverage_transforms(
    test_files: &[PathBuf],
    project_root: &Path,
) -> (Vec<(PathBuf, PathBuf)>, PathBuf) {
    let cov_dir = project_root.join(".hermes-test-cov");
    let _ = std::fs::remove_dir_all(&cov_dir);
    let _ = std::fs::create_dir_all(&cov_dir);

    let mut transforms: Vec<(PathBuf, PathBuf)> = Vec::new();

    for (i, file) in test_files.iter().enumerate() {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel = file.strip_prefix(project_root)
            .map(|r| r.to_string_lossy().to_string())
            .unwrap_or_else(|_| file.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("file{i}")));

        let var_name = format!("__cov{i}");
        let instrumented = instrument_file(&content, &rel, &var_name)
            .unwrap_or(content);

        // Write instrumented copy next to original with .cov suffix
        let temp_path = cov_dir.join(format!("cov{i}.tsx"));
        if std::fs::write(&temp_path, &instrumented).is_ok() {
            transforms.push((file.clone(), temp_path));
        }
    }

    (transforms, cov_dir)
}

// --- AST walker (read-only) ---

fn is_bare(stmt: &Statement) -> bool {
    !matches!(stmt, Statement::BlockStatement(_))
}

fn walk_stmt(
    stmt: &Statement,
    stmts: &mut Vec<(u32, u32, bool)>,
    fns: &mut Vec<(u32, u32, String)>,
    fbo: &mut Vec<Option<u32>>,
    br: &mut Vec<(u32, u32, String, u32)>,
    bare: bool,
) {
    let span = stmt.span();
    if span.start == span.end { return; }

    match stmt {
        Statement::BlockStatement(b) => {
            for s in &b.body { walk_stmt(s, stmts, fns, fbo, br, false); }
        }
        Statement::FunctionDeclaration(f) => {
            let name = f.id.as_ref().map(|id| id.name.to_string()).unwrap_or("anonymous".into());
            fns.push((span.start, span.end, name));
            if let Some(body) = &f.body {
                fbo.push(Some(body.span.start + 1));
                for s in &body.statements { walk_stmt(s, stmts, fns, fbo, br, false); }
            } else {
                fbo.push(None);
            }
        }
        Statement::ClassDeclaration(cls) => {
            stmts.push((span.start, span.end, bare));
            walk_class(&cls.body, stmts, fns, fbo, br);
        }
        Statement::IfStatement(i) => {
            stmts.push((span.start, span.end, bare));
            br.push((span.start, span.end, "if".into(), 2));
            walk_stmt(&i.consequent, stmts, fns, fbo, br, is_bare(&i.consequent));
            if let Some(alt) = &i.alternate { walk_stmt(alt, stmts, fns, fbo, br, is_bare(alt)); }
        }
        Statement::ForStatement(f) => {
            stmts.push((span.start, span.end, bare));
            walk_stmt(&f.body, stmts, fns, fbo, br, is_bare(&f.body));
        }
        Statement::ForInStatement(f) => {
            stmts.push((span.start, span.end, bare));
            walk_stmt(&f.body, stmts, fns, fbo, br, is_bare(&f.body));
        }
        Statement::ForOfStatement(f) => {
            stmts.push((span.start, span.end, bare));
            walk_stmt(&f.body, stmts, fns, fbo, br, is_bare(&f.body));
        }
        Statement::WhileStatement(w) => {
            stmts.push((span.start, span.end, bare));
            walk_stmt(&w.body, stmts, fns, fbo, br, is_bare(&w.body));
        }
        Statement::DoWhileStatement(d) => {
            stmts.push((span.start, span.end, bare));
            walk_stmt(&d.body, stmts, fns, fbo, br, is_bare(&d.body));
        }
        Statement::SwitchStatement(sw) => {
            stmts.push((span.start, span.end, bare));
            br.push((span.start, span.end, "switch".into(), sw.cases.len() as u32));
            for case in &sw.cases {
                for s in &case.consequent { walk_stmt(s, stmts, fns, fbo, br, false); }
            }
        }
        Statement::TryStatement(t) => {
            stmts.push((span.start, span.end, bare));
            for s in &t.block.body { walk_stmt(s, stmts, fns, fbo, br, false); }
            if let Some(h) = &t.handler { for s in &h.body.body { walk_stmt(s, stmts, fns, fbo, br, false); } }
            if let Some(f) = &t.finalizer { for s in &f.body { walk_stmt(s, stmts, fns, fbo, br, false); } }
        }
        Statement::ExpressionStatement(e) => {
            stmts.push((span.start, span.end, bare));
            walk_expr(&e.expression, stmts, fns, fbo, br);
        }
        Statement::VariableDeclaration(v) => {
            stmts.push((span.start, span.end, bare));
            for d in &v.declarations {
                if let Some(init) = &d.init { walk_expr(init, stmts, fns, fbo, br); }
            }
        }
        Statement::ReturnStatement(r) => {
            stmts.push((span.start, span.end, bare));
            if let Some(arg) = &r.argument { walk_expr(arg, stmts, fns, fbo, br); }
        }
        Statement::ThrowStatement(_) | Statement::BreakStatement(_)
        | Statement::ContinueStatement(_) | Statement::LabeledStatement(_) => {
            stmts.push((span.start, span.end, bare));
        }
        _ => {}
    }
}

fn walk_expr(
    expr: &Expression,
    stmts: &mut Vec<(u32, u32, bool)>,
    fns: &mut Vec<(u32, u32, String)>,
    fbo: &mut Vec<Option<u32>>,
    br: &mut Vec<(u32, u32, String, u32)>,
) {
    match expr {
        Expression::CallExpression(call) => {
            walk_expr(&call.callee, stmts, fns, fbo, br);
            for arg in &call.arguments {
                if let Argument::SpreadElement(s) = arg { walk_expr(&s.argument, stmts, fns, fbo, br); }
                else if let Some(e) = arg.as_expression() { walk_expr(e, stmts, fns, fbo, br); }
            }
        }
        Expression::ArrowFunctionExpression(arrow) => {
            let span = arrow.span;
            fns.push((span.start, span.end, "(arrow)".into()));
            if arrow.expression {
                fbo.push(None);
                for s in &arrow.body.statements {
                    if let Statement::ExpressionStatement(e) = s {
                        walk_expr(&e.expression, stmts, fns, fbo, br);
                    }
                }
            } else {
                fbo.push(Some(arrow.body.span.start + 1));
                for s in &arrow.body.statements { walk_stmt(s, stmts, fns, fbo, br, false); }
            }
        }
        Expression::FunctionExpression(func) => {
            let span = func.span;
            let name = func.id.as_ref().map(|id| id.name.to_string()).unwrap_or("(anonymous)".into());
            fns.push((span.start, span.end, name));
            if let Some(body) = &func.body {
                fbo.push(Some(body.span.start + 1));
                for s in &body.statements { walk_stmt(s, stmts, fns, fbo, br, false); }
            } else {
                fbo.push(None);
            }
        }
        Expression::ConditionalExpression(c) => {
            br.push((c.span.start, c.span.end, "cond-expr".into(), 2));
            walk_expr(&c.test, stmts, fns, fbo, br);
            walk_expr(&c.consequent, stmts, fns, fbo, br);
            walk_expr(&c.alternate, stmts, fns, fbo, br);
        }
        Expression::LogicalExpression(l) => {
            walk_expr(&l.left, stmts, fns, fbo, br);
            walk_expr(&l.right, stmts, fns, fbo, br);
        }
        Expression::AssignmentExpression(a) => { walk_expr(&a.right, stmts, fns, fbo, br); }
        Expression::SequenceExpression(s) => { for e in &s.expressions { walk_expr(e, stmts, fns, fbo, br); } }
        Expression::ObjectExpression(o) => {
            for prop in &o.properties {
                if let ObjectPropertyKind::ObjectProperty(p) = prop { walk_expr(&p.value, stmts, fns, fbo, br); }
            }
        }
        Expression::ArrayExpression(a) => {
            for el in &a.elements {
                if let ArrayExpressionElement::SpreadElement(s) = el { walk_expr(&s.argument, stmts, fns, fbo, br); }
                else if let Some(e) = el.as_expression() { walk_expr(e, stmts, fns, fbo, br); }
            }
        }
        Expression::ClassExpression(cls) => { walk_class(&cls.body, stmts, fns, fbo, br); }
        Expression::ParenthesizedExpression(p) => { walk_expr(&p.expression, stmts, fns, fbo, br); }
        Expression::NewExpression(n) => {
            walk_expr(&n.callee, stmts, fns, fbo, br);
            for arg in &n.arguments {
                if let Argument::SpreadElement(s) = arg { walk_expr(&s.argument, stmts, fns, fbo, br); }
                else if let Some(e) = arg.as_expression() { walk_expr(e, stmts, fns, fbo, br); }
            }
        }
        Expression::TemplateLiteral(t) => { for e in &t.expressions { walk_expr(e, stmts, fns, fbo, br); } }
        Expression::TaggedTemplateExpression(t) => { walk_expr(&t.tag, stmts, fns, fbo, br); }
        Expression::UnaryExpression(u) => { walk_expr(&u.argument, stmts, fns, fbo, br); }
        Expression::BinaryExpression(b) => { walk_expr(&b.left, stmts, fns, fbo, br); walk_expr(&b.right, stmts, fns, fbo, br); }
        _ => {}
    }
}

fn walk_class(body: &ClassBody, stmts: &mut Vec<(u32, u32, bool)>, fns: &mut Vec<(u32, u32, String)>, fbo: &mut Vec<Option<u32>>, br: &mut Vec<(u32, u32, String, u32)>) {
    for el in &body.body {
        if let ClassElement::MethodDefinition(m) = el {
            let name = if let PropertyKey::StaticIdentifier(id) = &m.key { id.name.to_string() } else { "(method)".into() };
            fns.push((m.span.start, m.span.end, name));
            if let Some(body) = &m.value.body {
                fbo.push(Some(body.span.start + 1));
                for s in &body.statements { walk_stmt(s, stmts, fns, fbo, br, false); }
            } else { fbo.push(None); }
        }
        if let ClassElement::PropertyDefinition(p) = el {
            if let Some(val) = &p.value { walk_expr(val, stmts, fns, fbo, br); }
        }
    }
}

// --- Helpers ---

struct LineTable { line_starts: Vec<u32> }
impl LineTable {
    fn new(source: &str) -> Self {
        let mut starts = vec![0u32];
        for (i, ch) in source.bytes().enumerate() { if ch == b'\n' { starts.push(i as u32 + 1); } }
        Self { line_starts: starts }
    }
    fn lookup(&self, offset: u32) -> (u32, u32) {
        let line = match self.line_starts.binary_search(&offset) { Ok(i) => i, Err(i) => i.saturating_sub(1) };
        (line as u32 + 1, offset.saturating_sub(self.line_starts[line]))
    }
}

fn build_loc_map(locs: &[(u32, u32)], lt: &LineTable) -> String {
    let mut o = String::from("{");
    for (i, (s, e)) in locs.iter().enumerate() {
        let (sl, sc) = lt.lookup(*s); let (el, ec) = lt.lookup(*e);
        if i > 0 { o.push(','); }
        o.push_str(&format!("\"{i}\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}"));
    }
    o.push('}'); o
}

fn build_fn_map(fns: &[(u32, u32, String)], lt: &LineTable) -> String {
    let mut o = String::from("{");
    for (i, (s, e, name)) in fns.iter().enumerate() {
        let (sl, sc) = lt.lookup(*s); let (el, ec) = lt.lookup(*e);
        if i > 0 { o.push(','); }
        let n = name.replace('"', "\\\"");
        o.push_str(&format!("\"{i}\":{{\"name\":\"{n}\",\"decl\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}}}"));
    }
    o.push('}'); o
}

fn build_branch_map(branches: &[(u32, u32, String, u32)], lt: &LineTable) -> String {
    let mut o = String::from("{");
    for (i, (s, e, btype, _)) in branches.iter().enumerate() {
        let (sl, sc) = lt.lookup(*s); let (el, ec) = lt.lookup(*e);
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
