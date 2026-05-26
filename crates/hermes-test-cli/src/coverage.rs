//! Post-bundle coverage instrumentation.
//! Adds Istanbul-style counters to the bundler's perfect output.

use std::collections::BTreeMap;
use oxc::allocator::Allocator;
use oxc::ast::ast::*;
use oxc::parser::Parser;
use oxc::span::{GetSpan, SourceType};

pub fn instrument_bundle(source: &str, filename: &str) -> Option<(String, String)> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, SourceType::mjs()).parse();
    if ret.panicked { return None; }

    let mut stmts: Vec<(u32, u32, bool)> = Vec::new();
    let mut fns: Vec<(u32, u32, String)> = Vec::new();
    let mut fbo: Vec<Option<u32>> = Vec::new();
    let mut br: Vec<(u32, u32, String, u32)> = Vec::new();

    // Walk inside the IIFE body, not the IIFE call itself.
    // esbuild wraps everything in (() => { ... })() — we instrument the body.
    for s in &ret.program.body {
        // The top-level is typically one ExpressionStatement containing the IIFE call.
        // Walk INTO its arrow/function body, not the call itself.
        if let Statement::ExpressionStatement(e) = s {
            if let Expression::CallExpression(call) = &e.expression {
                // (() => { body })() — the callee is a ParenthesizedExpression wrapping an arrow
                walk_expr_iife_body(&call.callee, &mut stmts, &mut fns, &mut fbo, &mut br);
                continue;
            }
        }
        walk_stmt(s, &mut stmts, &mut fns, &mut fbo, &mut br, false);
    }

    if stmts.is_empty() && fns.is_empty() {
        return Some((source.to_string(), "{}".to_string()));
    }

    let fid = filename.replace('\\', "/");
    let lt = LineTable::new(source);
    let spans: Vec<(u32,u32)> = stmts.iter().map(|(s,e,_)|(*s,*e)).collect();
    let sm = build_loc_map(&spans, &lt);
    let fm = build_fn_map(&fns, &lt);
    let bm = build_branch_map(&br, &lt);
    let cov_map = format!("{{\"{fid}\":{{\"statementMap\":{sm},\"fnMap\":{fm},\"branchMap\":{bm}}}}}");
    let si = build_zero_map(stmts.len());
    let fi = build_zero_map(fns.len());
    let bi = build_branch_zero_map(&br);

    // Istanbul pattern: hoisted function with cache
    let preamble = format!(
        "function __cov(){{var c=__cov.__c;if(c)return c;var g=globalThis;if(!g.__coverage__)g.__coverage__={{}};if(!g.__coverage__[\"{fid}\"])g.__coverage__[\"{fid}\"]={{path:\"{fid}\",s:{si},f:{fi},b:{bi}}};__cov.__c=g.__coverage__[\"{fid}\"];return __cov.__c}}\n"
    );

    // Insert preamble inside the IIFE
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

/// Unwrap esbuild's IIFE callee and walk its body statements.
fn walk_expr_iife_body(expr: &Expression, stmts: &mut Vec<(u32,u32,bool)>, fns: &mut Vec<(u32,u32,String)>, fbo: &mut Vec<Option<u32>>, br: &mut Vec<(u32,u32,String,u32)>) {
    match expr {
        Expression::ParenthesizedExpression(p) => walk_expr_iife_body(&p.expression, stmts, fns, fbo, br),
        Expression::ArrowFunctionExpression(a) => {
            for s in &a.body.statements { walk_stmt(s, stmts, fns, fbo, br, false); }
        }
        Expression::FunctionExpression(f) => {
            if let Some(body) = &f.body {
                for s in &body.statements { walk_stmt(s, stmts, fns, fbo, br, false); }
            }
        }
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

struct LineTable{line_starts:Vec<u32>}
impl LineTable{
    fn new(s:&str)->Self{let mut v=vec![0u32];for(i,c)in s.bytes().enumerate(){if c==b'\n'{v.push(i as u32+1);}}Self{line_starts:v}}
    fn lookup(&self,o:u32)->(u32,u32){let l=match self.line_starts.binary_search(&o){Ok(i)=>i,Err(i)=>i.saturating_sub(1)};(l as u32+1,o.saturating_sub(self.line_starts[l]))}
}
fn build_loc_map(l:&[(u32,u32)],lt:&LineTable)->String{let mut o=String::from("{");for(i,(s,e))in l.iter().enumerate(){let(sl,sc)=lt.lookup(*s);let(el,ec)=lt.lookup(*e);if i>0{o.push(',');}o.push_str(&format!("\"{i}\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}"));}o.push('}');o}
fn build_fn_map(f:&[(u32,u32,String)],lt:&LineTable)->String{let mut o=String::from("{");for(i,(s,e,n))in f.iter().enumerate(){let(sl,sc)=lt.lookup(*s);let(el,ec)=lt.lookup(*e);if i>0{o.push(',');}let n=n.replace('"',"\\\"");o.push_str(&format!("\"{i}\":{{\"name\":\"{n}\",\"decl\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}}}"));}o.push('}');o}
fn build_branch_map(b:&[(u32,u32,String,u32)],lt:&LineTable)->String{let mut o=String::from("{");for(i,(s,e,t,_))in b.iter().enumerate(){let(sl,sc)=lt.lookup(*s);let(el,ec)=lt.lookup(*e);if i>0{o.push(',');}o.push_str(&format!("\"{i}\":{{\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"type\":\"{t}\"}}"));}o.push('}');o}
fn build_zero_map(c:usize)->String{let mut o=String::from("{");for i in 0..c{if i>0{o.push(',');}o.push_str(&format!("\"{i}\":0"));}o.push('}');o}
fn build_branch_zero_map(b:&[(u32,u32,String,u32)])->String{let mut o=String::from("{");for(i,(_,_,_,p))in b.iter().enumerate(){if i>0{o.push(',');}let z:Vec<&str>=(0..*p).map(|_|"0").collect();o.push_str(&format!("\"{i}\":[{}]",z.join(",")));}o.push('}');o}

pub fn collect_coverage(rt:&crate::hermes::Runtime,map_path:Option<&std::path::Path>)->Option<String>{
    let js=r#"(function(){var c=globalThis.__coverage__;if(!c)return'null';return JSON.stringify(c)})()"#;
    let raw=rt.eval(js,"coverage-collect").ok()?;
    let json_str:String=serde_json::from_str(&raw).unwrap_or(raw.clone());
    if json_str=="null"||json_str.is_empty(){return None;}
    let mut cov:serde_json::Value=serde_json::from_str(&json_str).ok()?;
    if let Some(p)=map_path{if let Ok(ms)=std::fs::read_to_string(p){if let Ok(m)=serde_json::from_str::<serde_json::Value>(&ms){if let(Some(co),Some(mo))=(cov.as_object_mut(),m.as_object()){for(f,md)in mo{if let Some(fc)=co.get_mut(f){if let(Some(fo),Some(d))=(fc.as_object_mut(),md.as_object()){for(k,v)in d{fo.insert(k.clone(),v.clone());}}}}}}}}
    Some(coverage_to_lcov(&cov))
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
