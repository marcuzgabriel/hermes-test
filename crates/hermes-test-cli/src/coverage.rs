//! Istanbul-compatible coverage instrumentation using OXC.
//!
//! Parses JS source, collects statement/function/branch locations,
//! injects `__coverage__[fileId].s[id]++` counters, outputs instrumented JS.
//! After test execution, __coverage__ is read from Hermes and written as lcov.

use std::collections::BTreeMap;

/// A source location range (1-based line/col).
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct Loc {
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
}

/// Coverage map for a single file (Istanbul-compatible).
#[allow(dead_code)]
struct FileCoverage {
    statements: Vec<Loc>,       // statementMap
    functions: Vec<(Loc, String)>, // fnMap: (loc, name)
    branches: Vec<(Loc, String, u32)>, // branchMap: (loc, type, num_paths)
}

/// Instrument a JS bundle string with Istanbul-compatible coverage counters.
/// Returns (instrumented_code, coverage_map_json) or None on error.
pub fn instrument_bundle(source: &str, filename: &str) -> Option<String> {
    let allocator = oxc::allocator::Allocator::default();
    let source_type = oxc::span::SourceType::mjs();
    let ret = oxc::parser::Parser::new(&allocator, source, source_type).parse();

    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }

    let program = ret.program;

    // Collect all statement spans
    let mut stmts: Vec<(u32, u32)> = Vec::new(); // (start_offset, end_offset)
    let mut fns: Vec<(u32, u32, String)> = Vec::new(); // (start, end, name)
    let mut branches: Vec<(u32, u32, String, u32)> = Vec::new(); // (start, end, type, paths)

    collect_coverage_points(&program, source, &mut stmts, &mut fns, &mut branches);

    if stmts.is_empty() && fns.is_empty() {
        return Some(source.to_string());
    }

    // Build the coverage object initialization + counter injections
    let file_id = filename.replace('\\', "/");

    // Build statementMap, fnMap, branchMap as JSON
    let mut stmt_map = String::from("{");
    let mut s_init = String::from("{");
    for (i, (start, end)) in stmts.iter().enumerate() {
        let (sl, sc) = offset_to_line_col(source, *start);
        let (el, ec) = offset_to_line_col(source, *end);
        if i > 0 { stmt_map.push(','); s_init.push(','); }
        stmt_map.push_str(&format!(
            "\"{i}\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}"
        ));
        s_init.push_str(&format!("\"{i}\":0"));
    }
    stmt_map.push('}');
    s_init.push('}');

    let mut fn_map = String::from("{");
    let mut f_init = String::from("{");
    for (i, (start, end, name)) in fns.iter().enumerate() {
        let (sl, sc) = offset_to_line_col(source, *start);
        let (el, ec) = offset_to_line_col(source, *end);
        if i > 0 { fn_map.push(','); f_init.push(','); }
        fn_map.push_str(&format!(
            "\"{i}\":{{\"name\":\"{name}\",\"decl\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}}}"
        ));
        f_init.push_str(&format!("\"{i}\":0"));
    }
    fn_map.push('}');
    f_init.push('}');

    let mut branch_map = String::from("{");
    let mut b_init = String::from("{");
    for (i, (start, end, btype, paths)) in branches.iter().enumerate() {
        let (sl, sc) = offset_to_line_col(source, *start);
        let (el, ec) = offset_to_line_col(source, *end);
        if i > 0 { branch_map.push(','); b_init.push(','); }
        branch_map.push_str(&format!(
            "\"{i}\":{{\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"type\":\"{btype}\",\"locations\":[]}}"
        ));
        let zeros: Vec<&str> = (0..*paths).map(|_| "0").collect();
        b_init.push_str(&format!("\"{i}\":[{}]", zeros.join(",")));
    }
    branch_map.push('}');
    b_init.push('}');

    // Build the preamble: initialize __coverage__ for this file
    let preamble = format!(
        r#"var __cov = (function() {{
  var g = globalThis;
  if (!g.__coverage__) g.__coverage__ = {{}};
  if (!g.__coverage__["{file_id}"]) g.__coverage__["{file_id}"] = {{
    path: "{file_id}",
    statementMap: {stmt_map},
    fnMap: {fn_map},
    branchMap: {branch_map},
    s: {s_init},
    f: {f_init},
    b: {b_init}
  }};
  return g.__coverage__["{file_id}"];
}})();
"#
    );

    // Now inject counter increments into the source.
    // We insert `__cov.s[id]++,` before each statement and `__cov.f[id]++,` at function entry.
    // Strategy: collect all insertion points, sort by offset descending, insert from end to start.
    let mut insertions: Vec<(u32, String)> = Vec::new();

    for (i, (start, _end)) in stmts.iter().enumerate() {
        insertions.push((*start, format!("__cov.s[{i}]++;")));
    }
    for (i, (start, _end, _name)) in fns.iter().enumerate() {
        // Insert after the opening brace of the function body
        // We approximate: find the first '{' after start
        if let Some(brace_pos) = source[*start as usize..].find('{') {
            let insert_pos = *start + brace_pos as u32 + 1;
            insertions.push((insert_pos, format!("__cov.f[{i}]++;")));
        }
    }
    for (i, (start, _end, btype, _paths)) in branches.iter().enumerate() {
        if btype == "if" {
            // For if branches, we insert counters at the consequent and alternate
            // This is approximate — full AST would be better
            insertions.push((*start, format!("__cov.b[{i}][0]++;")));
        }
    }

    // Sort insertions by offset descending (insert from end to preserve earlier offsets)
    insertions.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = source.to_string();
    for (offset, code) in &insertions {
        let pos = *offset as usize;
        if pos <= result.len() {
            result.insert_str(pos, code);
        }
    }

    Some(format!("{preamble}{result}"))
}

fn collect_coverage_points(
    program: &oxc::ast::ast::Program,
    _source: &str,
    stmts: &mut Vec<(u32, u32)>,
    fns: &mut Vec<(u32, u32, String)>,
    branches: &mut Vec<(u32, u32, String, u32)>,
) {
    

    for stmt in &program.body {
        collect_stmt(stmt, stmts, fns, branches);
    }
}

fn collect_stmt(
    stmt: &oxc::ast::ast::Statement,
    stmts: &mut Vec<(u32, u32)>,
    fns: &mut Vec<(u32, u32, String)>,
    branches: &mut Vec<(u32, u32, String, u32)>,
) {
    use oxc::ast::ast::*;
    use oxc::span::GetSpan;

    let span = stmt.span();
    // Skip empty or tiny spans
    if span.start == span.end { return; }

    match stmt {
        Statement::FunctionDeclaration(f) => {
            let name = f.id.as_ref().map(|id| id.name.to_string()).unwrap_or_else(|| "anonymous".to_string());
            fns.push((span.start, span.end, name));
            if let Some(body) = &f.body {
                for s in &body.statements {
                    collect_stmt(s, stmts, fns, branches);
                }
            }
        }
        Statement::IfStatement(if_stmt) => {
            stmts.push((span.start, span.end));
            branches.push((span.start, span.end, "if".to_string(), 2));
            collect_stmt(&if_stmt.consequent, stmts, fns, branches);
            if let Some(alt) = &if_stmt.alternate {
                collect_stmt(alt, stmts, fns, branches);
            }
        }
        Statement::BlockStatement(block) => {
            for s in &block.body {
                collect_stmt(s, stmts, fns, branches);
            }
        }
        Statement::ForStatement(f) => {
            stmts.push((span.start, span.end));
            collect_stmt(&f.body, stmts, fns, branches);
        }
        Statement::ForInStatement(f) => {
            stmts.push((span.start, span.end));
            collect_stmt(&f.body, stmts, fns, branches);
        }
        Statement::ForOfStatement(f) => {
            stmts.push((span.start, span.end));
            collect_stmt(&f.body, stmts, fns, branches);
        }
        Statement::WhileStatement(w) => {
            stmts.push((span.start, span.end));
            collect_stmt(&w.body, stmts, fns, branches);
        }
        Statement::DoWhileStatement(d) => {
            stmts.push((span.start, span.end));
            collect_stmt(&d.body, stmts, fns, branches);
        }
        Statement::SwitchStatement(sw) => {
            stmts.push((span.start, span.end));
            let paths = sw.cases.len() as u32;
            branches.push((span.start, span.end, "switch".to_string(), paths));
            for case in &sw.cases {
                for s in &case.consequent {
                    collect_stmt(s, stmts, fns, branches);
                }
            }
        }
        Statement::TryStatement(t) => {
            stmts.push((span.start, span.end));
            for s in &t.block.body {
                collect_stmt(s, stmts, fns, branches);
            }
            if let Some(handler) = &t.handler {
                for s in &handler.body.body {
                    collect_stmt(s, stmts, fns, branches);
                }
            }
            if let Some(finalizer) = &t.finalizer {
                for s in &finalizer.body {
                    collect_stmt(s, stmts, fns, branches);
                }
            }
        }
        Statement::ReturnStatement(_) |
        Statement::ExpressionStatement(_) |
        Statement::VariableDeclaration(_) |
        Statement::ThrowStatement(_) |
        Statement::BreakStatement(_) |
        Statement::ContinueStatement(_) => {
            stmts.push((span.start, span.end));
        }
        _ => {
            // Other statements: still count as statements
            if span.end > span.start + 1 {
                stmts.push((span.start, span.end));
            }
        }
    }
}

fn offset_to_line_col(source: &str, offset: u32) -> (u32, u32) {
    let offset = offset as usize;
    let mut line = 1u32;
    let mut col = 0u32;
    for (i, ch) in source.char_indices() {
        if i >= offset { break; }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Read __coverage__ from Hermes runtime and format as lcov.
pub fn collect_coverage(rt: &crate::hermes::Runtime) -> Option<String> {
    let js = r#"(function() {
        var cov = globalThis.__coverage__;
        if (!cov) return 'null';
        return JSON.stringify(cov);
    })()"#;

    let raw = rt.eval(js, "coverage-collect").ok()?;
    let json_str: String = serde_json::from_str(&raw).unwrap_or(raw.clone());
    if json_str == "null" || json_str.is_empty() { return None; }

    let coverage: serde_json::Value = serde_json::from_str(&json_str).ok()?;
    Some(coverage_to_lcov(&coverage))
}

/// Convert Istanbul __coverage__ JSON to lcov format.
fn coverage_to_lcov(coverage: &serde_json::Value) -> String {
    let mut lcov = String::new();
    let obj = match coverage.as_object() {
        Some(o) => o,
        None => return lcov,
    };

    for (filename, file_cov) in obj {
        lcov.push_str(&format!("SF:{filename}\n"));

        // Function coverage
        if let Some(fn_map) = file_cov["fnMap"].as_object() {
            for (_id, fn_def) in fn_map {
                let name = fn_def["name"].as_str().unwrap_or("anonymous");
                let line = fn_def["decl"]["start"]["line"].as_u64().unwrap_or(0);
                lcov.push_str(&format!("FN:{line},{name}\n"));
            }
            if let Some(f) = file_cov["f"].as_object() {
                for (id, count) in f {
                    if let Some(fn_def) = fn_map.get(id) {
                        let name = fn_def["name"].as_str().unwrap_or("anonymous");
                        let c = count.as_u64().unwrap_or(0);
                        lcov.push_str(&format!("FNDA:{c},{name}\n"));
                    }
                }
            }
        }

        // Line coverage from statement map
        if let Some(s_map) = file_cov["statementMap"].as_object() {
            if let Some(s) = file_cov["s"].as_object() {
                let mut lines: BTreeMap<u64, u64> = BTreeMap::new();
                for (id, stmt) in s_map {
                    let line = stmt["start"]["line"].as_u64().unwrap_or(0);
                    let count = s.get(id).and_then(|v| v.as_u64()).unwrap_or(0);
                    let entry = lines.entry(line).or_insert(0);
                    *entry = (*entry).max(count);
                }
                for (line, count) in &lines {
                    lcov.push_str(&format!("DA:{line},{count}\n"));
                }
            }
        }

        // Branch coverage
        if let Some(b_map) = file_cov["branchMap"].as_object() {
            if let Some(b) = file_cov["b"].as_object() {
                for (id, branch_def) in b_map {
                    if let Some(counts) = b.get(id).and_then(|v| v.as_array()) {
                        let line = branch_def["loc"]["start"]["line"].as_u64().unwrap_or(0);
                        for (i, c) in counts.iter().enumerate() {
                            let count = c.as_u64().unwrap_or(0);
                            lcov.push_str(&format!("BRDA:{line},{id},{i},{count}\n"));
                        }
                    }
                }
            }
        }

        lcov.push_str("end_of_record\n");
    }

    lcov
}
