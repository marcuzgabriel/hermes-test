//! Post-bundle coverage instrumentation.
//! Adds Istanbul-style counters to the bundler's perfect output.

use std::collections::BTreeMap;
use oxc::allocator::Allocator;
use oxc::ast::ast::*;
use oxc::parser::Parser;
use oxc::span::{GetSpan, SourceType};

/// Source map info for accurate line mapping.
pub struct SourceMapInfo {
    pub source_map: sourcemap::SourceMap,
    /// Line delta: (patched_lines - unpatched_lines). Patches add lines before module bodies.
    pub line_delta: u32,
}

pub fn instrument_bundle(source: &str, filename: &str) -> Option<(String, String)> {
    instrument_bundle_inner(source, filename, None)
}

pub fn instrument_bundle_with_sourcemap(source: &str, filename: &str, sm_info: &SourceMapInfo) -> Option<(String, String)> {
    instrument_bundle_inner(source, filename, Some(sm_info))
}

fn instrument_bundle_inner(source: &str, filename: &str, sm_info: Option<&SourceMapInfo>) -> Option<(String, String)> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, SourceType::mjs()).parse();
    if ret.panicked { return None; }

    let mut stmts: Vec<(u32, u32, bool)> = Vec::new();
    let mut fns: Vec<(u32, u32, String)> = Vec::new();
    let mut fbo: Vec<Option<u32>> = Vec::new();
    let mut br: Vec<(u32, u32, String, u32)> = Vec::new();

    for s in &ret.program.body {
        if let Statement::ExpressionStatement(e) = s {
            if let Expression::CallExpression(call) = &e.expression {
                walk_iife(&call.callee, &mut stmts, &mut fns, &mut fbo, &mut br);
                continue;
            }
        }
        walk_stmt(s, &mut stmts, &mut fns, &mut fbo, &mut br, false);
    }

    if stmts.is_empty() && fns.is_empty() {
        return Some((source.to_string(), "{}".to_string()));
    }

    let lt = LineTable::new(source);

    // Resolve file + source line for a byte offset in the patched bundle.
    // With source map: look up (unpatched_line, col) → original (file, line).
    // Without: fall back to module boundary scanning.
    let modules = scan_module_boundaries(source);
    let find_file_and_line = |offset: u32| -> Option<(String, u32, u32)> {
        let (line, col) = lt.lookup(offset);
        if let Some(info) = sm_info {
            // Compute the unpatched line (patches add lines before modules)
            let unpatched_line = line.saturating_sub(info.line_delta);
            // Source map uses 0-based line/col
            if let Some(token) = info.source_map.lookup_token(unpatched_line.saturating_sub(1), col) {
                if let Some(src) = token.get_source() {
                    let src_line = token.get_src_line(); // 0-based
                    let src_col = token.get_src_col();
                    // Filter out unwanted files
                    if src.contains("node_modules") || is_test_file(src)
                        || src.starts_with("..") || src.starts_with(".hermes-test-") {
                        return None;
                    }
                    // Skip import/export-from lines (boilerplate, not logic)
                    if let Some(contents) = info.source_map.get_source_contents(token.get_src_id()) {
                        if let Some(line_text) = contents.lines().nth(src_line as usize) {
                            let trimmed = line_text.trim();
                            if trimmed.starts_with("import ") || trimmed.starts_with("import{")
                                || (trimmed.starts_with("export ") && trimmed.contains(" from ")) {
                                return None;
                            }
                        }
                    }
                    return Some((src.to_string(), src_line + 1, src_col)); // back to 1-based
                }
            }
            None // no source map match = skip this position
        } else {
            // Fallback: module boundary approach
            for (path, start, end) in modules.iter().rev() {
                if offset >= *start && offset < *end {
                    let base = lt.lookup(*start).0;
                    return Some((path.clone(), line.saturating_sub(base), col));
                }
            }
            None // fallback to bundle name → skip
        }
    };

    // Build per-file coverage maps grouped by source file
    let mut file_stmts: BTreeMap<String, Vec<(usize, u32, u32)>> = BTreeMap::new();
    let mut file_fns: BTreeMap<String, Vec<(usize, u32, u32, String)>> = BTreeMap::new();
    let mut file_br: BTreeMap<String, Vec<(usize, u32, u32, String, u32)>> = BTreeMap::new();
    // Store resolved (source_line, source_col) per global ID for each type
    let mut stmt_locs: BTreeMap<usize, (u32, u32, u32, u32)> = BTreeMap::new(); // gid → (start_line, start_col, end_line, end_col)
    let mut fn_locs: BTreeMap<usize, (u32, u32, u32, u32)> = BTreeMap::new();
    let mut br_locs: BTreeMap<usize, (u32, u32, u32, u32)> = BTreeMap::new();

    for (i, (s, e, _)) in stmts.iter().enumerate() {
        if let Some((file, src_line, src_col)) = find_file_and_line(*s) {
            let end_loc = find_file_and_line(*e);
            let (el, ec) = end_loc.map(|(_, l, c)| (l, c)).unwrap_or((src_line, src_col + (e - s)));
            file_stmts.entry(file).or_default().push((i, *s, *e));
            stmt_locs.insert(i, (src_line, src_col, el, ec));
        }
    }
    for (i, (s, e, n)) in fns.iter().enumerate() {
        if let Some((file, src_line, src_col)) = find_file_and_line(*s) {
            let end_loc = find_file_and_line(*e);
            let (el, ec) = end_loc.map(|(_, l, c)| (l, c)).unwrap_or((src_line, src_col + (e - s)));
            file_fns.entry(file).or_default().push((i, *s, *e, n.clone()));
            fn_locs.insert(i, (src_line, src_col, el, ec));
        }
    }
    for (i, (s, e, t, p)) in br.iter().enumerate() {
        if let Some((file, src_line, src_col)) = find_file_and_line(*s) {
            let end_loc = find_file_and_line(*e);
            let (el, ec) = end_loc.map(|(_, l, c)| (l, c)).unwrap_or((src_line, src_col + (e - s)));
            file_br.entry(file).or_default().push((i, *s, *e, t.clone(), *p));
            br_locs.insert(i, (src_line, src_col, el, ec));
        }
    }

    // Build per-file coverage map JSON
    let mut cov_map_parts: Vec<String> = Vec::new();
    for (file, stmts_list) in &file_stmts {
        let sm = build_loc_map_resolved(stmts_list.iter().map(|(gid, _, _)| *gid).collect(), &stmt_locs);
        let fns_list = file_fns.get(file).cloned().unwrap_or_default();
        let fm = build_fn_map_resolved(fns_list.iter().map(|(gid, _, _, n)| (*gid, n.clone())).collect(), &fn_locs);
        let br_list = file_br.get(file).cloned().unwrap_or_default();
        let bm = build_branch_map_resolved(br_list.iter().map(|(gid, _, _, t, _)| (*gid, t.clone())).collect(), &br_locs);
        let s_ids: Vec<String> = stmts_list.iter().map(|(gid, _, _)| gid.to_string()).collect();
        let f_ids: Vec<String> = fns_list.iter().map(|(gid, _, _, _)| gid.to_string()).collect();
        let b_ids: Vec<String> = br_list.iter().map(|(gid, _, _, _, _)| gid.to_string()).collect();
        cov_map_parts.push(format!(
            "\"{file}\":{{\"statementMap\":{sm},\"fnMap\":{fm},\"branchMap\":{bm},\"_sIds\":[{}],\"_fIds\":[{}],\"_bIds\":[{}]}}",
            s_ids.join(","), f_ids.join(","), b_ids.join(",")
        ));
    }
    let cov_map = format!("{{{}}}", cov_map_parts.join(","));

    // Runtime preamble: one entry with all counters (global IDs)
    let fid = filename.replace('\\', "/");
    let si = build_zero_map(stmts.len());
    let fi = build_zero_map(fns.len());
    let bi = build_branch_zero_map(&br);

    // Use typed arrays for counters to avoid Hermes 196607 property limit.
    // Int32Array is compact — no object properties, just indexed access.
    let preamble = format!(
        "function __cov(){{var c=__cov.__c;if(c)return c;var g=globalThis;if(!g.__coverage__)g.__coverage__={{}};if(!g.__coverage__[\"{fid}\"])g.__coverage__[\"{fid}\"]={{path:\"{fid}\",s:new Int32Array({s_len}),f:new Int32Array({f_len}),b:{bi}}};__cov.__c=g.__coverage__[\"{fid}\"];return __cov.__c}}\n",
        s_len = stmts.len(),
        f_len = fns.len(),
    );

    let iife = source.find("(() => {").map(|p| p + 8)
        .or_else(|| source.find("(function() {").map(|p| p + 13))
        .unwrap_or(0) as u32;

    let mut ins: Vec<(u32, String)> = Vec::new();
    ins.push((iife, preamble));
    for (i, (start, end, bare)) in stmts.iter().enumerate() {
        if *bare {
            ins.push((*start, format!("{{__cov().s[{i}]++;")));
            ins.push((*end, "}".to_string()));
        } else {
            ins.push((*start, format!("__cov().s[{i}]++;")));
        }
    }
    for (i, off) in fbo.iter().enumerate() {
        if let Some(p) = off { ins.push((*p, format!("__cov().f[{i}]++;"))); }
    }

    ins.sort_by_key(|i| i.0);
    let mut merged: Vec<(u32, String)> = Vec::new();
    for (o, c) in ins {
        if let Some(last) = merged.last_mut() { if last.0 == o { last.1.push_str(&c); continue; } }
        merged.push((o, c));
    }

    let mut out = String::with_capacity(source.len() + merged.len() * 20);
    let mut last = 0usize;
    for (o, c) in &merged {
        let p = *o as usize;
        if p >= last && p <= source.len() { out.push_str(&source[last..p]); out.push_str(c); last = p; }
    }
    out.push_str(&source[last..]);
    Some((out, cov_map))
}

// --- AST walker ---

/// Scan esbuild's module method keys (`    "path"(`) to get per-file byte ranges.
fn scan_module_boundaries(source: &str) -> Vec<(String, u32, u32)> {
    let mut modules: Vec<(String, u32)> = Vec::new(); // (path, start_offset)
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut line_start = 0usize;

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' || i == len - 1 {
            let line_end = if b == b'\n' { i } else { i + 1 };
            let line = &source[line_start..line_end];
            if line.starts_with("    \"") {
                if let Some(close) = line[5..].find('"') {
                    let after = 5 + close + 1;
                    if after < line.len() && line.as_bytes()[after] == b'(' {
                        let path = &line[5..5 + close];
                        if (path.ends_with(".ts") || path.ends_with(".tsx") || path.ends_with(".js")
                            || path.ends_with(".jsx") || path.ends_with(".mjs") || path.ends_with(".cjs"))
                            && !path.contains("node_modules")
                            && !is_test_file(path)
                            && !path.starts_with("..")
                            && !path.starts_with(".hermes-test-") {
                            modules.push((path.to_string(), line_start as u32));
                        }
                    }
                }
            }
            line_start = i + 1;
        }
    }

    // Convert to (path, start, end) — each module extends to the next module's start
    let mut result: Vec<(String, u32, u32)> = Vec::new();
    for (idx, (path, start)) in modules.iter().enumerate() {
        let end = if idx + 1 < modules.len() { modules[idx + 1].1 } else { len as u32 };
        result.push((path.clone(), *start, end));
    }
    result
}

fn is_test_file(path: &str) -> bool {
    path.contains(".test.") || path.contains(".spec.") || path.contains("__tests__")
}

fn walk_iife(expr: &Expression, stmts: &mut Vec<(u32,u32,bool)>, fns: &mut Vec<(u32,u32,String)>, fbo: &mut Vec<Option<u32>>, br: &mut Vec<(u32,u32,String,u32)>) {
    match expr {
        Expression::ParenthesizedExpression(p) => walk_iife(&p.expression, stmts, fns, fbo, br),
        Expression::ArrowFunctionExpression(a) => { for s in &a.body.statements { walk_stmt(s, stmts, fns, fbo, br, false); } }
        Expression::FunctionExpression(f) => { if let Some(b) = &f.body { for s in &b.statements { walk_stmt(s, stmts, fns, fbo, br, false); } } }
        _ => { walk_expr(expr, stmts, fns, fbo, br); }
    }
}

fn is_bare(s: &Statement) -> bool { !matches!(s, Statement::BlockStatement(_)) }

fn walk_stmt(stmt: &Statement, stmts: &mut Vec<(u32,u32,bool)>, fns: &mut Vec<(u32,u32,String)>, fbo: &mut Vec<Option<u32>>, br: &mut Vec<(u32,u32,String,u32)>, bare: bool) {
    let sp = stmt.span(); if sp.start == sp.end { return; }
    match stmt {
        Statement::BlockStatement(b) => { for s in &b.body { walk_stmt(s, stmts, fns, fbo, br, false); } }
        Statement::FunctionDeclaration(f) => {
            let n = f.id.as_ref().map(|i| i.name.to_string()).unwrap_or("anonymous".into());
            fns.push((sp.start, sp.end, n));
            if let Some(body) = &f.body { fbo.push(Some(body.span.start+1)); for s in &body.statements { walk_stmt(s, stmts, fns, fbo, br, false); } } else { fbo.push(None); }
        }
        Statement::ClassDeclaration(c) => { stmts.push((sp.start,sp.end,bare)); walk_class(&c.body,stmts,fns,fbo,br); }
        Statement::IfStatement(i) => { stmts.push((sp.start,sp.end,bare)); br.push((sp.start,sp.end,"if".into(),2)); walk_stmt(&i.consequent,stmts,fns,fbo,br,is_bare(&i.consequent)); if let Some(a)=&i.alternate{walk_stmt(a,stmts,fns,fbo,br,is_bare(a));} }
        Statement::ForStatement(f) => { stmts.push((sp.start,sp.end,bare)); walk_stmt(&f.body,stmts,fns,fbo,br,is_bare(&f.body)); }
        Statement::ForInStatement(f) => { stmts.push((sp.start,sp.end,bare)); walk_stmt(&f.body,stmts,fns,fbo,br,is_bare(&f.body)); }
        Statement::ForOfStatement(f) => { stmts.push((sp.start,sp.end,bare)); walk_stmt(&f.body,stmts,fns,fbo,br,is_bare(&f.body)); }
        Statement::WhileStatement(w) => { stmts.push((sp.start,sp.end,bare)); walk_stmt(&w.body,stmts,fns,fbo,br,is_bare(&w.body)); }
        Statement::DoWhileStatement(d) => { stmts.push((sp.start,sp.end,bare)); walk_stmt(&d.body,stmts,fns,fbo,br,is_bare(&d.body)); }
        Statement::SwitchStatement(sw) => { stmts.push((sp.start,sp.end,bare)); br.push((sp.start,sp.end,"switch".into(),sw.cases.len() as u32)); for c in &sw.cases { for s in &c.consequent { walk_stmt(s,stmts,fns,fbo,br,false); } } }
        Statement::TryStatement(t) => { stmts.push((sp.start,sp.end,bare)); for s in &t.block.body{walk_stmt(s,stmts,fns,fbo,br,false);} if let Some(h)=&t.handler{for s in &h.body.body{walk_stmt(s,stmts,fns,fbo,br,false);}} if let Some(f)=&t.finalizer{for s in &f.body{walk_stmt(s,stmts,fns,fbo,br,false);}} }
        Statement::ExpressionStatement(e) => { stmts.push((sp.start,sp.end,bare)); walk_expr(&e.expression,stmts,fns,fbo,br); }
        Statement::VariableDeclaration(v) => { stmts.push((sp.start,sp.end,bare)); for d in &v.declarations{if let Some(i)=&d.init{walk_expr(i,stmts,fns,fbo,br);}} }
        Statement::ReturnStatement(r) => { stmts.push((sp.start,sp.end,bare)); if let Some(a)=&r.argument{walk_expr(a,stmts,fns,fbo,br);} }
        Statement::ThrowStatement(_)|Statement::BreakStatement(_)|Statement::ContinueStatement(_)|Statement::LabeledStatement(_) => { stmts.push((sp.start,sp.end,bare)); }
        _ => {}
    }
}

fn walk_expr(expr: &Expression, stmts: &mut Vec<(u32,u32,bool)>, fns: &mut Vec<(u32,u32,String)>, fbo: &mut Vec<Option<u32>>, br: &mut Vec<(u32,u32,String,u32)>) {
    match expr {
        Expression::CallExpression(c) => { walk_expr(&c.callee,stmts,fns,fbo,br); for a in &c.arguments{if let Argument::SpreadElement(s)=a{walk_expr(&s.argument,stmts,fns,fbo,br);}else if let Some(e)=a.as_expression(){walk_expr(e,stmts,fns,fbo,br);}} }
        Expression::ArrowFunctionExpression(a) => { let sp=a.span; fns.push((sp.start,sp.end,"(arrow)".into())); if a.expression{fbo.push(None);for s in &a.body.statements{if let Statement::ExpressionStatement(e)=s{walk_expr(&e.expression,stmts,fns,fbo,br);}}}else{fbo.push(Some(a.body.span.start+1));for s in &a.body.statements{walk_stmt(s,stmts,fns,fbo,br,false);}} }
        Expression::FunctionExpression(f) => { let sp=f.span; let n=f.id.as_ref().map(|i|i.name.to_string()).unwrap_or("(anonymous)".into()); fns.push((sp.start,sp.end,n)); if let Some(b)=&f.body{fbo.push(Some(b.span.start+1));for s in &b.statements{walk_stmt(s,stmts,fns,fbo,br,false);}}else{fbo.push(None);} }
        Expression::ConditionalExpression(c) => { br.push((c.span.start,c.span.end,"cond-expr".into(),2)); walk_expr(&c.test,stmts,fns,fbo,br); walk_expr(&c.consequent,stmts,fns,fbo,br); walk_expr(&c.alternate,stmts,fns,fbo,br); }
        Expression::LogicalExpression(l) => { walk_expr(&l.left,stmts,fns,fbo,br); walk_expr(&l.right,stmts,fns,fbo,br); }
        Expression::AssignmentExpression(a) => { walk_expr(&a.right,stmts,fns,fbo,br); }
        Expression::SequenceExpression(s) => { for e in &s.expressions{walk_expr(e,stmts,fns,fbo,br);} }
        Expression::ObjectExpression(o) => { for p in &o.properties{if let ObjectPropertyKind::ObjectProperty(p)=p{walk_expr(&p.value,stmts,fns,fbo,br);}} }
        Expression::ArrayExpression(a) => { for e in &a.elements{if let ArrayExpressionElement::SpreadElement(s)=e{walk_expr(&s.argument,stmts,fns,fbo,br);}else if let Some(e)=e.as_expression(){walk_expr(e,stmts,fns,fbo,br);}} }
        Expression::ClassExpression(c) => { walk_class(&c.body,stmts,fns,fbo,br); }
        Expression::ParenthesizedExpression(p) => { walk_expr(&p.expression,stmts,fns,fbo,br); }
        Expression::NewExpression(n) => { walk_expr(&n.callee,stmts,fns,fbo,br); for a in &n.arguments{if let Argument::SpreadElement(s)=a{walk_expr(&s.argument,stmts,fns,fbo,br);}else if let Some(e)=a.as_expression(){walk_expr(e,stmts,fns,fbo,br);}} }
        Expression::TemplateLiteral(t) => { for e in &t.expressions{walk_expr(e,stmts,fns,fbo,br);} }
        Expression::TaggedTemplateExpression(t) => { walk_expr(&t.tag,stmts,fns,fbo,br); }
        Expression::UnaryExpression(u) => { walk_expr(&u.argument,stmts,fns,fbo,br); }
        Expression::BinaryExpression(b) => { walk_expr(&b.left,stmts,fns,fbo,br); walk_expr(&b.right,stmts,fns,fbo,br); }
        _ => {}
    }
}

fn walk_class(body: &ClassBody, stmts: &mut Vec<(u32,u32,bool)>, fns: &mut Vec<(u32,u32,String)>, fbo: &mut Vec<Option<u32>>, br: &mut Vec<(u32,u32,String,u32)>) {
    for el in &body.body {
        if let ClassElement::MethodDefinition(m) = el { let n=if let PropertyKey::StaticIdentifier(i)=&m.key{i.name.to_string()}else{"(method)".into()}; fns.push((m.span.start,m.span.end,n)); if let Some(b)=&m.value.body{fbo.push(Some(b.span.start+1));for s in &b.statements{walk_stmt(s,stmts,fns,fbo,br,false);}}else{fbo.push(None);} }
        if let ClassElement::PropertyDefinition(p) = el { if let Some(v)=&p.value{walk_expr(v,stmts,fns,fbo,br);} }
    }
}

// --- Helpers ---

struct LineTable{line_starts:Vec<u32>}
impl LineTable{
    fn new(s:&str)->Self{let mut v=vec![0u32];for(i,c)in s.bytes().enumerate(){if c==b'\n'{v.push(i as u32+1);}}Self{line_starts:v}}
    fn lookup(&self,o:u32)->(u32,u32){let l=match self.line_starts.binary_search(&o){Ok(i)=>i,Err(i)=>i.saturating_sub(1)};(l as u32+1,o.saturating_sub(self.line_starts[l]))}
}
fn build_loc_map(l:&[(u32,u32)],lt:&LineTable,base:u32)->String{let mut o=String::from("{");for(i,(s,e))in l.iter().enumerate(){let(sl,sc)=lt.lookup(*s);let(el,ec)=lt.lookup(*e);let sl=sl.saturating_sub(base);let el=el.saturating_sub(base);if i>0{o.push(',');}o.push_str(&format!("\"{i}\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}"));}o.push('}');o}
fn build_fn_map(f:&[(u32,u32,String)],lt:&LineTable,base:u32)->String{let mut o=String::from("{");for(i,(s,e,n))in f.iter().enumerate(){let(sl,sc)=lt.lookup(*s);let(el,ec)=lt.lookup(*e);let sl=sl.saturating_sub(base);let el=el.saturating_sub(base);if i>0{o.push(',');}let n=n.replace('"',"\\\"");o.push_str(&format!("\"{i}\":{{\"name\":\"{n}\",\"decl\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}}}"));}o.push('}');o}
fn build_branch_map(b:&[(u32,u32,String,u32)],lt:&LineTable,base:u32)->String{let mut o=String::from("{");for(i,(s,e,t,_))in b.iter().enumerate(){let(sl,sc)=lt.lookup(*s);let(el,ec)=lt.lookup(*e);let sl=sl.saturating_sub(base);let el=el.saturating_sub(base);if i>0{o.push(',');}o.push_str(&format!("\"{i}\":{{\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"type\":\"{t}\"}}"));}o.push('}');o}
// Resolved builders: use pre-computed source locations from source map
fn build_loc_map_resolved(gids: Vec<usize>, locs: &BTreeMap<usize, (u32, u32, u32, u32)>) -> String {
    let mut o = String::from("{");
    for (i, gid) in gids.iter().enumerate() {
        if let Some(&(sl, sc, el, ec)) = locs.get(gid) {
            if i > 0 { o.push(','); }
            o.push_str(&format!("\"{i}\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}"));
        }
    }
    o.push('}'); o
}
fn build_fn_map_resolved(items: Vec<(usize, String)>, locs: &BTreeMap<usize, (u32, u32, u32, u32)>) -> String {
    let mut o = String::from("{");
    for (i, (gid, n)) in items.iter().enumerate() {
        if let Some(&(sl, sc, el, ec)) = locs.get(gid) {
            if i > 0 { o.push(','); }
            let n = n.replace('"', "\\\"");
            o.push_str(&format!("\"{i}\":{{\"name\":\"{n}\",\"decl\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}}}"));
        }
    }
    o.push('}'); o
}
fn build_branch_map_resolved(items: Vec<(usize, String)>, locs: &BTreeMap<usize, (u32, u32, u32, u32)>) -> String {
    let mut o = String::from("{");
    for (i, (gid, t)) in items.iter().enumerate() {
        if let Some(&(sl, sc, el, ec)) = locs.get(gid) {
            if i > 0 { o.push(','); }
            o.push_str(&format!("\"{i}\":{{\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"type\":\"{t}\"}}"));
        }
    }
    o.push('}'); o
}

fn build_zero_map(c:usize)->String{let mut o=String::from("{");for i in 0..c{if i>0{o.push(',');}o.push_str(&format!("\"{i}\":0"));}o.push('}');o}
fn build_branch_zero_map(b:&[(u32,u32,String,u32)])->String{let mut o=String::from("{");for(i,(_,_,_,p))in b.iter().enumerate(){if i>0{o.push(',');}let z:Vec<&str>=(0..*p).map(|_|"0").collect();o.push_str(&format!("\"{i}\":[{}]",z.join(",")));}o.push('}');o}

// --- Coverage reporting ---

/// Parse lcov data and print a terminal summary table. Returns total line coverage %.
pub fn print_summary(lcov: &str) -> f64 {
    let mut files: Vec<(String, u64, u64, u64, u64)> = Vec::new(); // (name, hit, total, fn_hit, fn_total)

    let mut cur_file = String::new();
    let mut hit = 0u64;
    let mut total = 0u64;
    let mut fn_hit = 0u64;
    let mut fn_total = 0u64;

    for line in lcov.lines() {
        if let Some(f) = line.strip_prefix("SF:") {
            cur_file = f.to_string();
            hit = 0; total = 0; fn_hit = 0; fn_total = 0;
        } else if let Some(da) = line.strip_prefix("DA:") {
            if let Some((_, count_str)) = da.split_once(',') {
                total += 1;
                if count_str.parse::<u64>().unwrap_or(0) > 0 { hit += 1; }
            }
        } else if line.starts_with("FN:") {
            fn_total += 1;
        } else if let Some(fnda) = line.strip_prefix("FNDA:") {
            if let Some((count_str, _)) = fnda.split_once(',') {
                if count_str.parse::<u64>().unwrap_or(0) > 0 { fn_hit += 1; }
            }
        } else if line == "end_of_record" && !cur_file.is_empty() {
            files.push((cur_file.clone(), hit, total, fn_hit, fn_total));
        }
    }

    if files.is_empty() { return 100.0; }

    // Find max filename length for formatting
    let max_name = files.iter().map(|(n, _, _, _, _)| n.len()).max().unwrap_or(20).min(60);

    eprintln!();
    eprintln!(" {:<width$}  {:>7}  {:>7}", "File", "Lines", "Funcs", width = max_name);
    eprintln!(" {:<width$}  {:>7}  {:>7}", "─".repeat(max_name), "───────", "───────", width = max_name);

    let mut total_hit = 0u64;
    let mut total_lines = 0u64;
    let mut total_fn_hit = 0u64;
    let mut total_fns = 0u64;

    for (name, h, t, fh, ft) in &files {
        let pct = if *t > 0 { (*h as f64 / *t as f64) * 100.0 } else { 100.0 };
        let fn_pct = if *ft > 0 { (*fh as f64 / *ft as f64) * 100.0 } else { 100.0 };
        let color = if pct >= 80.0 { "\x1b[32m" } else if pct >= 50.0 { "\x1b[33m" } else { "\x1b[31m" };
        let fn_color = if fn_pct >= 80.0 { "\x1b[32m" } else if fn_pct >= 50.0 { "\x1b[33m" } else { "\x1b[31m" };

        let display_name = if name.len() > max_name { &name[name.len() - max_name..] } else { name };
        eprintln!(" {:<width$}  {color}{:>5.1}%\x1b[0m  {fn_color}{:>5.1}%\x1b[0m",
            display_name, pct, fn_pct, width = max_name);

        total_hit += h; total_lines += t; total_fn_hit += fh; total_fns += ft;
    }

    let total_pct = if total_lines > 0 { (total_hit as f64 / total_lines as f64) * 100.0 } else { 100.0 };
    let total_fn_pct = if total_fns > 0 { (total_fn_hit as f64 / total_fns as f64) * 100.0 } else { 100.0 };
    let color = if total_pct >= 80.0 { "\x1b[32m" } else if total_pct >= 50.0 { "\x1b[33m" } else { "\x1b[31m" };
    let fn_color = if total_fn_pct >= 80.0 { "\x1b[32m" } else if total_fn_pct >= 50.0 { "\x1b[33m" } else { "\x1b[31m" };

    eprintln!(" {:<width$}  {:>7}  {:>7}", "─".repeat(max_name), "───────", "───────", width = max_name);
    eprintln!(" {:<width$}  {color}{:>5.1}%\x1b[0m  {fn_color}{:>5.1}%\x1b[0m",
        "Total", total_pct, total_fn_pct, width = max_name);
    eprintln!(" {:<width$}  {:>4}/{:<4}  {:>4}/{:<4}",
        "", total_hit, total_lines, total_fn_hit, total_fns, width = max_name);
    eprintln!();
    total_pct
}

/// Generate a self-contained HTML coverage report from lcov data.
/// `project_root` is used to read source files for line-by-line display.
pub fn generate_html_report(lcov: &str, output_path: &std::path::Path, project_root: &std::path::Path) -> Result<(), String> {
    let mut files: Vec<(String, Vec<(u64, u64)>)> = Vec::new(); // (name, [(line, count)])

    let mut cur_file = String::new();
    let mut lines: Vec<(u64, u64)> = Vec::new();

    for line in lcov.lines() {
        if let Some(f) = line.strip_prefix("SF:") {
            cur_file = f.to_string();
            lines = Vec::new();
        } else if let Some(da) = line.strip_prefix("DA:") {
            if let Some((line_str, count_str)) = da.split_once(',') {
                let l = line_str.parse::<u64>().unwrap_or(0);
                let c = count_str.parse::<u64>().unwrap_or(0);
                lines.push((l, c));
            }
        } else if line == "end_of_record" && !cur_file.is_empty() {
            lines.sort_by_key(|(l, _)| *l);
            files.push((cur_file.clone(), lines.clone()));
        }
    }

    let mut html = String::from(r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Coverage Report</title>
<style>
  * { box-sizing: border-box; }
  body { font-family: -apple-system, system-ui, sans-serif; margin: 0; padding: 20px; background: #1a1a2e; color: #eee; }
  h1 { color: #e94560; margin-bottom: 5px; }
  .subtitle { color: #666; margin-bottom: 20px; font-size: 14px; }
  .summary { background: #16213e; padding: 15px; border-radius: 8px; margin-bottom: 20px; }
  .summary table { border-collapse: collapse; width: 100%; }
  .summary th { text-align: left; padding: 8px 12px; border-bottom: 1px solid #333; color: #888; font-size: 13px; }
  .summary td { padding: 8px 12px; font-size: 13px; }
  .summary tr:hover { background: rgba(255,255,255,0.03); }
  .summary .file-link { color: #7db8ff; text-decoration: none; cursor: pointer; }
  .summary .file-link:hover { text-decoration: underline; }
  .pct { font-weight: bold; }
  .high { color: #0f9d58; }
  .mid { color: #f4b400; }
  .low { color: #db4437; }
  .bar { display: inline-block; height: 6px; border-radius: 3px; background: #333; width: 80px; vertical-align: middle; margin-left: 8px; }
  .bar-fill { height: 100%; border-radius: 3px; }
  .file-section { background: #16213e; border-radius: 8px; margin-bottom: 15px; overflow: hidden; }
  .file-header { padding: 12px 15px; background: #0f3460; cursor: pointer; display: flex; justify-content: space-between; align-items: center; }
  .file-header:hover { background: #1a4a7a; }
  .file-name { font-weight: bold; font-size: 14px; }
  .file-body { display: none; overflow-x: auto; }
  .file-body.open { display: block; }
  .source-table { width: 100%; border-collapse: collapse; }
  .source-table td { padding: 0; vertical-align: top; }
  .line { display: flex; font-family: 'SF Mono', Monaco, 'Fira Code', monospace; font-size: 12px; line-height: 1.7; border-bottom: 1px solid rgba(255,255,255,0.02); }
  .line-no { min-width: 45px; width: 45px; text-align: right; padding-right: 8px; color: #444; user-select: none; flex-shrink: 0; }
  .line-count { min-width: 45px; width: 45px; text-align: right; padding-right: 8px; color: #666; user-select: none; flex-shrink: 0; font-size: 11px; }
  .line-code { flex: 1; padding-left: 12px; white-space: pre; tab-size: 2; overflow-x: visible; }
  .covered { background: rgba(15, 157, 88, 0.12); }
  .covered .line-count { color: #0f9d58; }
  .uncovered { background: rgba(219, 68, 55, 0.18); }
  .uncovered .line-code { color: #ffa0a0; }
  .uncovered .line-count { color: #db4437; font-weight: bold; }
  .neutral { }
  .toggle-icon { font-size: 12px; color: #666; margin-right: 8px; transition: transform 0.2s; }
  .file-header.open .toggle-icon { transform: rotate(90deg); }
</style>
</head><body>
<h1>Coverage Report</h1>
"#);

    // Pre-read source files to get accurate line counts
    // Also compute totals for the header
    let file_sources: Vec<Vec<String>> = files.iter().map(|(name, _)| {
        let source_path = project_root.join(name);
        if let Ok(content) = std::fs::read_to_string(&source_path) {
            content.lines().map(|l| l.to_string()).collect()
        } else {
            Vec::new()
        }
    }).collect();

    // Compute per-file stats for header
    let mut grand_total = 0usize;
    let mut grand_covered = 0usize;
    let mut file_pcts: Vec<f64> = Vec::new();
    for (idx, (_, cov_lines)) in files.iter().enumerate() {
        let src_len = file_sources[idx].len() as u64;
        let relevant: Vec<&(u64, u64)> = if src_len > 0 {
            cov_lines.iter().filter(|(l, _)| *l <= src_len).collect()
        } else {
            cov_lines.iter().collect()
        };
        let t = relevant.len();
        let c = relevant.iter().filter(|&&&(_, c)| c > 0).count();
        grand_total += t;
        grand_covered += c;
        file_pcts.push(if t > 0 { (c as f64 / t as f64) * 100.0 } else { 100.0 });
    }
    let grand_pct = if grand_total > 0 { (grand_covered as f64 / grand_total as f64) * 100.0 } else { 100.0 };
    let avg_pct = if !file_pcts.is_empty() { file_pcts.iter().sum::<f64>() / file_pcts.len() as f64 } else { 100.0 };
    let full_covered = file_pcts.iter().filter(|p| **p >= 100.0).count();

    let pct_color = |p: f64| -> &str { if p >= 80.0 { "#0f9d58" } else if p >= 50.0 { "#f4b400" } else { "#db4437" } };

    html.push_str(&format!(r#"<div style="background:#16213e;padding:20px 24px;border-radius:10px;margin-bottom:24px">
  <div style="display:flex;gap:32px;align-items:flex-end;flex-wrap:wrap">
    <div>
      <div style="color:#888;font-size:11px;text-transform:uppercase;letter-spacing:1px;margin-bottom:4px">Total Coverage</div>
      <div style="font-size:36px;font-weight:bold;color:{}">{grand_pct:.1}%</div>
      <div style="color:#666;font-size:13px">{grand_covered} / {grand_total} statements</div>
    </div>
    <div>
      <div style="color:#888;font-size:11px;text-transform:uppercase;letter-spacing:1px;margin-bottom:4px">Avg per File</div>
      <div style="font-size:36px;font-weight:bold;color:{}">{avg_pct:.1}%</div>
      <div style="color:#666;font-size:13px">across {} files</div>
    </div>
    <div>
      <div style="color:#888;font-size:11px;text-transform:uppercase;letter-spacing:1px;margin-bottom:4px">Fully Covered</div>
      <div style="font-size:36px;font-weight:bold;color:#0f9d58">{full_covered}</div>
      <div style="color:#666;font-size:13px">of {} files at 100%</div>
    </div>
  </div>
  <div style="margin-top:14px;background:#333;height:8px;border-radius:4px;overflow:hidden">
    <div style="height:100%;width:{grand_pct:.0}%;background:{};border-radius:4px;transition:width 0.3s"></div>
  </div>
</div>
"#, pct_color(grand_pct), pct_color(avg_pct), files.len(), files.len(), pct_color(grand_pct)));

    // Per-file source code with coverage highlighting
    for (idx, (name, cov_lines)) in files.iter().enumerate() {
        let source_lines = &file_sources[idx];
        let src_len = source_lines.len() as u64;
        let relevant: Vec<&(u64, u64)> = if src_len > 0 {
            cov_lines.iter().filter(|(l, _)| *l <= src_len).collect()
        } else {
            cov_lines.iter().collect()
        };
        let total = relevant.len();
        let covered = relevant.iter().filter(|&&&(_, c)| c > 0).count();
        let pct = if total > 0 { (covered as f64 / total as f64) * 100.0 } else { 100.0 };
        let cls = if pct >= 80.0 { "high" } else if pct >= 50.0 { "mid" } else { "low" };

        html.push_str(&format!(
            "<div class='file-section' id='file-{idx}'><div class='file-header' onclick='toggleFile(\"file-{idx}\")'><span><span class='toggle-icon'>▶</span><span class='file-name'>{name}</span></span><span class='pct {cls}'>{covered}/{total} ({pct:.1}%)</span></div>\n<div class='file-body'>\n"
        ));

        let cov_map: std::collections::HashMap<u64, u64> = cov_lines.iter().copied().collect();

        if !source_lines.is_empty() {
            // Source available: show every source line, overlay coverage data
            for (i, code) in source_lines.iter().enumerate() {
                let line_no = (i + 1) as u64;
                let (cls, count_str) = if let Some(&count) = cov_map.get(&line_no) {
                    if count > 0 { ("covered", format!("{count}x")) }
                    else { ("uncovered", "0x".to_string()) }
                } else { ("neutral", String::new()) };
                let code = html_escape(code);
                html.push_str(&format!(
                    "<div class='line {cls}'><span class='line-no'>{line_no}</span><span class='line-count'>{count_str}</span><span class='line-code'>{code}</span></div>\n"
                ));
            }
        } else {
            // No source: show only lines with coverage data
            let max_line = cov_lines.iter().map(|(l, _)| *l).max().unwrap_or(0);
            for line_no in 1..=max_line {
                let (cls, count_str) = if let Some(&count) = cov_map.get(&line_no) {
                    if count > 0 { ("covered", format!("{count}x")) }
                    else { ("uncovered", "0x".to_string()) }
                } else { ("neutral", String::new()) };
                html.push_str(&format!(
                    "<div class='line {cls}'><span class='line-no'>{line_no}</span><span class='line-count'>{count_str}</span><span class='line-code'></span></div>\n"
                ));
            }
        }

        html.push_str("</div></div>\n");
    }

    html.push_str(r#"<script>
function toggleFile(id) {
  var el = document.getElementById(id);
  var body = el.querySelector('.file-body');
  var header = el.querySelector('.file-header');
  body.classList.toggle('open');
  header.classList.toggle('open');
}
</script>"#);
    html.push_str("</body></html>");

    std::fs::write(output_path, &html).map_err(|e| format!("Failed to write HTML report: {e}"))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// --- Coverage collection ---

pub fn collect_coverage(rt:&crate::hermes::Runtime,map_path:Option<&std::path::Path>)->Option<String>{
    let js=r#"(function(){var c=globalThis.__coverage__;if(!c)return'null';return JSON.stringify(c)})()"#;
    let raw=rt.eval(js,"coverage-collect").ok()?;
    let json_str:String=serde_json::from_str(&raw).unwrap_or(raw.clone());
    if json_str=="null"||json_str.is_empty(){return None;}
    let runtime:serde_json::Value=serde_json::from_str(&json_str).ok()?;

    // Read the per-file map from disk and split runtime counters by file
    if let Some(p)=map_path{
        if let Ok(ms)=std::fs::read_to_string(p){
            if let Ok(file_map)=serde_json::from_str::<serde_json::Value>(&ms){
                // Get the runtime bundle counters
                let bundle_key = runtime.as_object()?.keys().next()?.clone();
                let rt_data = &runtime[&bundle_key];
                let rt_s = rt_data["s"].as_object()?;
                let rt_f = rt_data["f"].as_object()?;
                let rt_b = rt_data["b"].as_object()?;

                // Build per-file coverage entries
                let mut per_file = serde_json::Map::new();
                for (file, fdata) in file_map.as_object()? {
                    let mut entry = serde_json::Map::new();
                    entry.insert("path".into(), serde_json::Value::String(file.clone()));

                    // Copy statementMap, fnMap, branchMap from disk
                    if let Some(v) = fdata.get("statementMap") { entry.insert("statementMap".into(), v.clone()); }
                    if let Some(v) = fdata.get("fnMap") { entry.insert("fnMap".into(), v.clone()); }
                    if let Some(v) = fdata.get("branchMap") { entry.insert("branchMap".into(), v.clone()); }

                    // Split s counters using _sIds mapping
                    if let Some(ids) = fdata["_sIds"].as_array() {
                        let mut s = serde_json::Map::new();
                        for (local, gid) in ids.iter().enumerate() {
                            let val = rt_s.get(&gid.to_string().replace('"', "")).cloned()
                                .unwrap_or(serde_json::Value::Number(0.into()));
                            s.insert(local.to_string(), val);
                        }
                        entry.insert("s".into(), serde_json::Value::Object(s));
                    }
                    // Split f counters
                    if let Some(ids) = fdata["_fIds"].as_array() {
                        let mut f = serde_json::Map::new();
                        for (local, gid) in ids.iter().enumerate() {
                            let val = rt_f.get(&gid.to_string().replace('"', "")).cloned()
                                .unwrap_or(serde_json::Value::Number(0.into()));
                            f.insert(local.to_string(), val);
                        }
                        entry.insert("f".into(), serde_json::Value::Object(f));
                    }
                    // Split b counters
                    if let Some(ids) = fdata["_bIds"].as_array() {
                        let mut b = serde_json::Map::new();
                        for (local, gid) in ids.iter().enumerate() {
                            let val = rt_b.get(&gid.to_string().replace('"', "")).cloned()
                                .unwrap_or(serde_json::json!([]));
                            b.insert(local.to_string(), val);
                        }
                        entry.insert("b".into(), serde_json::Value::Object(b));
                    }

                    per_file.insert(file.clone(), serde_json::Value::Object(entry));
                }
                return Some(coverage_to_lcov(&serde_json::Value::Object(per_file)));
            }
        }
    }

    // Fallback: no map file, use runtime as-is
    Some(coverage_to_lcov(&runtime))
}

fn coverage_to_lcov(cov:&serde_json::Value)->String{
    let mut l=String::new();
    let o=match cov.as_object(){Some(o)=>o,None=>return l};
    for(f,fc)in o{
        l.push_str(&format!("SF:{f}\n"));
        if let Some(fm)=fc["fnMap"].as_object(){for(_,fd)in fm{l.push_str(&format!("FN:{},{}\n",fd["decl"]["start"]["line"].as_u64().unwrap_or(0),fd["name"].as_str().unwrap_or("?")));}if let Some(f)=fc["f"].as_object(){for(id,c)in f{if let Some(fd)=fm.get(id){l.push_str(&format!("FNDA:{},{}\n",c.as_u64().unwrap_or(0),fd["name"].as_str().unwrap_or("?")));}}}}
        if let Some(sm)=fc["statementMap"].as_object(){if let Some(s)=fc["s"].as_object(){let mut lines:BTreeMap<u64,u64>=BTreeMap::new();for(id,st)in sm{let li=st["start"]["line"].as_u64().unwrap_or(0);let c=s.get(id).and_then(|v|v.as_u64()).unwrap_or(0);let e=lines.entry(li).or_insert(0);*e=(*e).max(c);}for(li,c)in &lines{l.push_str(&format!("DA:{li},{c}\n"));}}}
        if let Some(bm)=fc["branchMap"].as_object(){if let Some(b)=fc["b"].as_object(){for(id,bd)in bm{if let Some(cs)=b.get(id).and_then(|v|v.as_array()){let li=bd["loc"]["start"]["line"].as_u64().unwrap_or(0);for(j,c)in cs.iter().enumerate(){l.push_str(&format!("BRDA:{li},{id},{j},{}\n",c.as_u64().unwrap_or(0)));}}}}}
        l.push_str("end_of_record\n");
    }
    l
}
