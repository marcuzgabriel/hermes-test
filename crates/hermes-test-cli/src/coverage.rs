//! Istanbul-compatible coverage instrumentation using OXC VisitMut + Codegen.
//!
//! Parses JS source, walks the AST with VisitMut to:
//!   - Wrap bare control-flow bodies in blocks (fixes the string insertion bug)
//!   - Insert __cov.s[id]++ / __cov.f[id]++ counter statements
//! Then uses OXC Codegen to re-emit syntactically correct JS.

use std::cell::Cell;
use std::collections::BTreeMap;

use oxc::allocator::{Allocator, Box as OxcBox, Vec as OxcVec};
use oxc::ast::ast::*;
use oxc::ast_visit::{walk_mut::*, VisitMut};
use oxc::codegen::Codegen;
use oxc::parser::Parser;
use oxc::span::{GetSpan, SourceType, Span};
use oxc::syntax::node::NodeId;
use oxc::syntax::number::NumberBase;
use oxc::syntax::operator::UpdateOperator;
use oxc_str::Ident;
use oxc::syntax::scope::ScopeFlags;

/// Instrument a JS bundle with Istanbul-compatible coverage counters.
pub fn instrument_bundle(source: &str, filename: &str) -> Option<String> {
    let allocator = Allocator::default();
    let source_type = SourceType::mjs();
    let ret = Parser::new(&allocator, source, source_type).parse();
    if ret.panicked {
        return None;
    }

    let mut program = ret.program;

    let mut instrumentor = CoverageInstrumentor {
        allocator: &allocator,
        stmt_id: 0,
        fn_id: 0,
        branch_id: 0,
        pending_fn_counter: None,
        pending_fn_name: None,
        skip_next_body: false,
        stmts: Vec::new(),
        fns: Vec::new(),
        branches: Vec::new(),
    };

    instrumentor.visit_program(&mut program);

    if instrumentor.stmts.is_empty() && instrumentor.fns.is_empty() {
        return Some(source.to_string());
    }

    // Build preamble with coverage map metadata
    let file_id = filename.replace('\\', "/");
    let lt = LineTable::new(source);
    let stmt_map = build_loc_map(&instrumentor.stmts, &lt);
    let s_init = build_zero_map(instrumentor.stmts.len());
    let fn_map = build_fn_map(&instrumentor.fns, &lt);
    let f_init = build_zero_map(instrumentor.fns.len());
    let branch_map = build_branch_map(&instrumentor.branches, &lt);
    let b_init = build_branch_zero_map(&instrumentor.branches);

    let preamble = format!(
        "var __cov=(function(){{var g=globalThis;if(!g.__coverage__)g.__coverage__={{}};if(!g.__coverage__[\"{file_id}\"])g.__coverage__[\"{file_id}\"]={{path:\"{file_id}\",statementMap:{stmt_map},fnMap:{fn_map},branchMap:{branch_map},s:{s_init},f:{f_init},b:{b_init}}};return g.__coverage__[\"{file_id}\"]}})()\n"
    );

    // Use OXC Codegen to emit the modified AST
    let codegen_result = Codegen::new().build(&program);

    Some(format!("{}{}", preamble, codegen_result.code))
}

// --- VisitMut instrumentor ---

struct CoverageInstrumentor<'a> {
    allocator: &'a Allocator,
    stmt_id: usize,
    fn_id: usize,
    branch_id: usize,
    pending_fn_counter: Option<usize>,
    pending_fn_name: Option<String>,
    skip_next_body: bool,
    // Coverage map metadata (original source locations)
    stmts: Vec<(u32, u32)>,
    fns: Vec<(u32, u32, String)>,
    branches: Vec<(u32, u32, String, u32)>,
}

impl<'a> CoverageInstrumentor<'a> {
    /// Build `++__cov.{kind}[{id}];` as a Statement
    fn make_counter(&self, kind: &'a str, id: usize) -> Statement<'a> {
        let span = Span::default();

        // ++__cov.{kind}[{id}]
        let update = Expression::UpdateExpression(OxcBox::new_in(
            UpdateExpression {
                node_id: Cell::new(NodeId::DUMMY),
                span,
                operator: UpdateOperator::Increment,
                prefix: true,
                argument: SimpleAssignmentTarget::ComputedMemberExpression(OxcBox::new_in(
                    ComputedMemberExpression {
                        node_id: Cell::new(NodeId::DUMMY),
                        span,
                        object: Expression::StaticMemberExpression(OxcBox::new_in(
                            StaticMemberExpression {
                                node_id: Cell::new(NodeId::DUMMY),
                                span,
                                object: Expression::Identifier(OxcBox::new_in(
                                    IdentifierReference {
                                        node_id: Cell::new(NodeId::DUMMY),
                                        span,
                                        name: Ident::from("__cov"),
                                        reference_id: Cell::new(None),
                                    },
                                    self.allocator,
                                )),
                                property: IdentifierName {
                                    node_id: Cell::new(NodeId::DUMMY),
                                    span,
                                    name: Ident::from(kind),
                                },
                                optional: false,
                            },
                            self.allocator,
                        )),
                        expression: Expression::NumericLiteral(OxcBox::new_in(
                            NumericLiteral {
                                node_id: Cell::new(NodeId::DUMMY),
                                span,
                                value: id as f64,
                                raw: None,
                                base: NumberBase::Decimal,
                            },
                            self.allocator,
                        )),
                        optional: false,
                    },
                    self.allocator,
                )),
            },
            self.allocator,
        ));

        // ++__cov.{kind}[{id}];
        Statement::ExpressionStatement(OxcBox::new_in(
            ExpressionStatement {
                node_id: Cell::new(NodeId::DUMMY),
                span,
                expression: update,
            },
            self.allocator,
        ))
    }

    /// Wrap a bare statement in a BlockStatement
    fn wrap_in_block(&self, stmt: &mut Statement<'a>) {
        let span = Span::default();
        let dummy = Statement::EmptyStatement(OxcBox::new_in(
            EmptyStatement {
                node_id: Cell::new(NodeId::DUMMY),
                span,
            },
            self.allocator,
        ));
        let original = std::mem::replace(stmt, dummy);
        let mut body = OxcVec::new_in(self.allocator);
        body.push(original);
        *stmt = Statement::BlockStatement(OxcBox::new_in(
            BlockStatement {
                node_id: Cell::new(NodeId::DUMMY),
                span,
                body,
                scope_id: Cell::new(None),
            },
            self.allocator,
        ));
    }

    /// Whether this statement type should get a coverage counter
    fn should_count(stmt: &Statement) -> bool {
        matches!(
            stmt,
            Statement::ExpressionStatement(_)
                | Statement::IfStatement(_)
                | Statement::ForStatement(_)
                | Statement::ForInStatement(_)
                | Statement::ForOfStatement(_)
                | Statement::WhileStatement(_)
                | Statement::DoWhileStatement(_)
                | Statement::SwitchStatement(_)
                | Statement::TryStatement(_)
                | Statement::ReturnStatement(_)
                | Statement::ThrowStatement(_)
                | Statement::BreakStatement(_)
                | Statement::ContinueStatement(_)
                | Statement::LabeledStatement(_)
        ) || matches!(stmt, _ if stmt.is_declaration() && matches!(
            stmt.as_declaration(),
            Some(Declaration::VariableDeclaration(_) | Declaration::ClassDeclaration(_))
        ))
    }

    /// Instrument a statement list: visit children first (bottom-up), then interleave counters
    fn instrument_statements(
        &mut self,
        stmts: &mut OxcVec<'a, Statement<'a>>,
        fn_counter: Option<usize>,
    ) {
        // Take ownership of existing statements
        let mut old = std::mem::replace(stmts, OxcVec::new_in(self.allocator));

        // Visit each child first (recursive — inner blocks/functions get instrumented first)
        for stmt in old.iter_mut() {
            self.visit_statement(stmt);
        }

        // Build new list with counters interleaved
        let mut new_list = OxcVec::new_in(self.allocator);

        // Insert function entry counter at top of body
        if let Some(fn_id) = fn_counter {
            new_list.push(self.make_counter("f", fn_id));
        }

        for stmt in old.into_iter() {
            if Self::should_count(&stmt) {
                let sid = self.stmt_id;
                self.stmt_id += 1;
                self.stmts.push((stmt.span().start, stmt.span().end));
                new_list.push(self.make_counter("s", sid));
            }
            new_list.push(stmt);
        }

        *stmts = new_list;
    }
}

impl<'a> VisitMut<'a> for CoverageInstrumentor<'a> {
    // --- Top-level program body ---
    fn visit_program(&mut self, it: &mut Program<'a>) {
        self.instrument_statements(&mut it.body, None);
    }

    // --- Function body (block-body functions and arrows) ---
    fn visit_function_body(&mut self, it: &mut FunctionBody<'a>) {
        if self.skip_next_body {
            self.skip_next_body = false;
            // Still recurse for nested functions inside concise arrow expressions
            walk_function_body(self, it);
            return;
        }
        let fn_counter = self.pending_fn_counter.take();
        self.instrument_statements(&mut it.statements, fn_counter);
    }

    // --- Block statements (if/for/while bodies, try/catch, standalone blocks) ---
    fn visit_block_statement(&mut self, it: &mut BlockStatement<'a>) {
        self.instrument_statements(&mut it.body, None);
    }

    // --- Functions: record metadata and set pending counter ---
    fn visit_function(&mut self, it: &mut Function<'a>, flags: ScopeFlags) {
        let name = self
            .pending_fn_name
            .take()
            .unwrap_or_else(|| {
                it.id
                    .as_ref()
                    .map(|id| id.name.to_string())
                    .unwrap_or_else(|| "(anonymous)".into())
            });
        let fn_id = self.fn_id;
        self.fn_id += 1;
        self.fns.push((it.span.start, it.span.end, name));
        if it.body.is_some() {
            self.pending_fn_counter = Some(fn_id);
        }
        walk_function(self, it, flags);
    }

    // --- Arrow functions ---
    fn visit_arrow_function_expression(&mut self, it: &mut ArrowFunctionExpression<'a>) {
        let fn_id = self.fn_id;
        self.fn_id += 1;
        self.fns.push((it.span.start, it.span.end, "(arrow)".into()));
        if it.expression {
            // Concise arrow: () => expr — skip body instrumentation
            self.skip_next_body = true;
        } else {
            self.pending_fn_counter = Some(fn_id);
        }
        walk_arrow_function_expression(self, it);
    }

    // --- Method definitions: extract name for pending_fn_name ---
    fn visit_method_definition(&mut self, it: &mut MethodDefinition<'a>) {
        let name = match &it.key {
            PropertyKey::StaticIdentifier(id) => id.name.to_string(),
            _ => "(method)".into(),
        };
        self.pending_fn_name = Some(name);
        walk_method_definition(self, it);
    }

    // --- Control structures: wrap bare bodies, record branches ---

    fn visit_if_statement(&mut self, it: &mut IfStatement<'a>) {
        // Record branch
        self.branches
            .push((it.span.start, it.span.end, "if".into(), 2));

        // Wrap bare consequent in block
        if !matches!(&it.consequent, Statement::BlockStatement(_)) {
            self.wrap_in_block(&mut it.consequent);
        }
        // Wrap bare alternate in block
        if let Some(alt) = &mut it.alternate {
            if !matches!(alt, Statement::BlockStatement(_)) {
                self.wrap_in_block(alt);
            }
        }

        walk_if_statement(self, it);
    }

    fn visit_for_statement(&mut self, it: &mut ForStatement<'a>) {
        if !matches!(&it.body, Statement::BlockStatement(_)) {
            self.wrap_in_block(&mut it.body);
        }
        walk_for_statement(self, it);
    }

    fn visit_for_in_statement(&mut self, it: &mut ForInStatement<'a>) {
        if !matches!(&it.body, Statement::BlockStatement(_)) {
            self.wrap_in_block(&mut it.body);
        }
        walk_for_in_statement(self, it);
    }

    fn visit_for_of_statement(&mut self, it: &mut ForOfStatement<'a>) {
        if !matches!(&it.body, Statement::BlockStatement(_)) {
            self.wrap_in_block(&mut it.body);
        }
        walk_for_of_statement(self, it);
    }

    fn visit_while_statement(&mut self, it: &mut WhileStatement<'a>) {
        if !matches!(&it.body, Statement::BlockStatement(_)) {
            self.wrap_in_block(&mut it.body);
        }
        walk_while_statement(self, it);
    }

    fn visit_do_while_statement(&mut self, it: &mut DoWhileStatement<'a>) {
        if !matches!(&it.body, Statement::BlockStatement(_)) {
            self.wrap_in_block(&mut it.body);
        }
        walk_do_while_statement(self, it);
    }

    fn visit_switch_statement(&mut self, it: &mut SwitchStatement<'a>) {
        self.branches.push((
            it.span.start,
            it.span.end,
            "switch".into(),
            it.cases.len() as u32,
        ));
        walk_switch_statement(self, it);
    }

    // --- Switch cases: instrument consequent statement list ---
    fn visit_switch_case(&mut self, it: &mut SwitchCase<'a>) {
        // Visit test expression (for nested functions etc.)
        if let Some(test) = &mut it.test {
            self.visit_expression(test);
        }
        // Instrument consequent statements
        self.instrument_statements(&mut it.consequent, None);
    }

    // --- Conditional expressions: record branch ---
    fn visit_conditional_expression(&mut self, it: &mut ConditionalExpression<'a>) {
        self.branches.push((
            it.span.start,
            it.span.end,
            "cond-expr".into(),
            2,
        ));
        walk_conditional_expression(self, it);
    }
}

// --- Helpers (unchanged from original) ---

/// Pre-computed line offset table for O(1) line/col lookups.
struct LineTable {
    line_starts: Vec<u32>,
}

impl LineTable {
    fn new(source: &str) -> Self {
        let mut starts = vec![0u32];
        for (i, ch) in source.bytes().enumerate() {
            if ch == b'\n' {
                starts.push(i as u32 + 1);
            }
        }
        Self {
            line_starts: starts,
        }
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
        if i > 0 {
            o.push(',');
        }
        o.push_str(&format!(
            "\"{i}\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}"
        ));
    }
    o.push('}');
    o
}

fn build_fn_map(fns: &[(u32, u32, String)], lt: &LineTable) -> String {
    let mut o = String::from("{");
    for (i, (s, e, name)) in fns.iter().enumerate() {
        let (sl, sc) = lt.lookup(*s);
        let (el, ec) = lt.lookup(*e);
        if i > 0 {
            o.push(',');
        }
        let n = name.replace('"', "\\\"");
        o.push_str(&format!(
            "\"{i}\":{{\"name\":\"{n}\",\"decl\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}}}}"
        ));
    }
    o.push('}');
    o
}

fn build_branch_map(branches: &[(u32, u32, String, u32)], lt: &LineTable) -> String {
    let mut o = String::from("{");
    for (i, (s, e, btype, _)) in branches.iter().enumerate() {
        let (sl, sc) = lt.lookup(*s);
        let (el, ec) = lt.lookup(*e);
        if i > 0 {
            o.push(',');
        }
        o.push_str(&format!(
            "\"{i}\":{{\"loc\":{{\"start\":{{\"line\":{sl},\"column\":{sc}}},\"end\":{{\"line\":{el},\"column\":{ec}}}}},\"type\":\"{btype}\"}}"
        ));
    }
    o.push('}');
    o
}

fn build_zero_map(count: usize) -> String {
    let mut o = String::from("{");
    for i in 0..count {
        if i > 0 {
            o.push(',');
        }
        o.push_str(&format!("\"{i}\":0"));
    }
    o.push('}');
    o
}

fn build_branch_zero_map(branches: &[(u32, u32, String, u32)]) -> String {
    let mut o = String::from("{");
    for (i, (_, _, _, paths)) in branches.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let z: Vec<&str> = (0..*paths).map(|_| "0").collect();
        o.push_str(&format!("\"{i}\":[{}]", z.join(",")));
    }
    o.push('}');
    o
}

// --- Coverage collection ---

pub fn collect_coverage(rt: &crate::hermes::Runtime) -> Option<String> {
    let js = r#"(function(){var c=globalThis.__coverage__;if(!c)return'null';return JSON.stringify(c)})()"#;
    let raw = rt.eval(js, "coverage-collect").ok()?;
    let json_str: String = serde_json::from_str(&raw).unwrap_or(raw.clone());
    if json_str == "null" || json_str.is_empty() {
        return None;
    }
    let coverage: serde_json::Value = serde_json::from_str(&json_str).ok()?;
    Some(coverage_to_lcov(&coverage))
}

fn coverage_to_lcov(coverage: &serde_json::Value) -> String {
    let mut lcov = String::new();
    let obj = match coverage.as_object() {
        Some(o) => o,
        None => return lcov,
    };
    for (filename, fc) in obj {
        lcov.push_str(&format!("SF:{filename}\n"));
        if let Some(fm) = fc["fnMap"].as_object() {
            for (_, fd) in fm {
                lcov.push_str(&format!(
                    "FN:{},{}\n",
                    fd["decl"]["start"]["line"].as_u64().unwrap_or(0),
                    fd["name"].as_str().unwrap_or("?")
                ));
            }
            if let Some(f) = fc["f"].as_object() {
                for (id, c) in f {
                    if let Some(fd) = fm.get(id) {
                        lcov.push_str(&format!(
                            "FNDA:{},{}\n",
                            c.as_u64().unwrap_or(0),
                            fd["name"].as_str().unwrap_or("?")
                        ));
                    }
                }
            }
        }
        if let Some(sm) = fc["statementMap"].as_object() {
            if let Some(s) = fc["s"].as_object() {
                let mut lines: BTreeMap<u64, u64> = BTreeMap::new();
                for (id, st) in sm {
                    let l = st["start"]["line"].as_u64().unwrap_or(0);
                    let c = s.get(id).and_then(|v| v.as_u64()).unwrap_or(0);
                    let e = lines.entry(l).or_insert(0);
                    *e = (*e).max(c);
                }
                for (l, c) in &lines {
                    lcov.push_str(&format!("DA:{l},{c}\n"));
                }
            }
        }
        if let Some(bm) = fc["branchMap"].as_object() {
            if let Some(b) = fc["b"].as_object() {
                for (id, bd) in bm {
                    if let Some(cs) = b.get(id).and_then(|v| v.as_array()) {
                        let l = bd["loc"]["start"]["line"].as_u64().unwrap_or(0);
                        for (j, c) in cs.iter().enumerate() {
                            lcov.push_str(&format!(
                                "BRDA:{l},{id},{j},{}\n",
                                c.as_u64().unwrap_or(0)
                            ));
                        }
                    }
                }
            }
        }
        lcov.push_str("end_of_record\n");
    }
    lcov
}
