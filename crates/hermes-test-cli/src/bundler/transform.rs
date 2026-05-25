use std::path::{Path, PathBuf};

use super::config::BundleConfig;

/// Pre-transform test files that mock aliased paths. Vitest-style approach:
/// uses OXC AST parser to rewrite imports of mocked modules to lazy Proxy
/// wrappers that read from __HT_file_mocks at access time, while keeping
/// the real module available for non-test code via the active alias.
///
/// Returns a map of original_path → temp_path for transformed files.
#[allow(dead_code)]
pub fn pre_transform_test_files(
    test_files: &[PathBuf],
    mock_modules: &[String],
    cfg: &BundleConfig,
) -> Vec<(PathBuf, PathBuf)> {
    let mut transforms: Vec<(PathBuf, PathBuf)> = Vec::new();

    // Find which mock paths are aliased (those need pre-transform)
    let aliased_mocks: Vec<&String> = mock_modules.iter().filter(|m| {
        cfg.aliases.iter().any(|(alias, _)| {
            *m == alias || m.starts_with(&format!("{alias}/"))
        })
    }).collect();

    if aliased_mocks.is_empty() {
        return transforms;
    }

    for file in test_files {
        let Ok(source) = std::fs::read_to_string(file) else { continue };

        // Check if this file uses any aliased mocks
        let file_aliased_mocks: Vec<&&String> = aliased_mocks.iter()
            .filter(|m| source.contains(&format!("mockModule('{m}'")))
            .collect();

        if file_aliased_mocks.is_empty() {
            continue;
        }

        if let Some(transformed) = oxc_transform_test_file(&source, file, &file_aliased_mocks) {
            let temp_path = file.with_extension("__ht_transformed.ts");
            if std::fs::write(&temp_path, &transformed).is_ok() {
                transforms.push((file.clone(), temp_path));
            }
        }
    }

    transforms
}

/// Use OXC to parse a test file, find imports of mocked aliased paths,
/// and rewrite them to Proxy-based lazy accessors.
///
/// Transform: `import { useX, useY } from '@scope/pkg/hooks';`
/// Becomes:   (import removed, all refs to useX → __htMock0__.useX)
///
/// The Proxy reads from __HT_file_mocks[__currentTestFile]['@scope/pkg/hooks']
/// at access time, so different test files get different mocks.
#[allow(dead_code)]
pub fn oxc_transform_test_file(
    source: &str,
    file_path: &Path,
    aliased_mocks: &[&&String],
) -> Option<String> {
    use oxc::allocator::Allocator;
    use oxc::parser::Parser;
    use oxc::span::SourceType;

    let allocator = Allocator::default();
    let source_type = SourceType::from_path(file_path).unwrap_or_default();
    let ret = Parser::new(&allocator, source, source_type).parse();

    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }

    // Collect: for each aliased mock path, find import declarations and their local names.
    // We'll build text replacements sorted by position (reverse order for safe string splicing).
    struct ImportInfo {
        mock_path: String,
        proxy_var: String,
        import_start: u32,
        import_end: u32,
        // (local_name, span_start, span_end) for all IdentifierReferences to rewrite
        local_names: Vec<String>,
    }

    let mut imports_to_rewrite: Vec<ImportInfo> = Vec::new();

    for stmt in &ret.program.body {
        if let oxc::ast::ast::Statement::ImportDeclaration(import_decl) = stmt {
            let import_source = import_decl.source.value.as_str();
            // Check if this import's source matches any aliased mock
            let mock_idx = aliased_mocks.iter().position(|m| **m == import_source);
            if let Some(idx) = mock_idx {
                let mut local_names = Vec::new();

                // Type-only imports can be ignored (esbuild strips them)
                if import_decl.import_kind == oxc::ast::ast::ImportOrExportKind::Type {
                    continue;
                }

                // Collect named imports: import { a, b as c } from '...'
                if let Some(specifiers) = &import_decl.specifiers {
                    for spec in specifiers {
                        match spec {
                            oxc::ast::ast::ImportDeclarationSpecifier::ImportSpecifier(s) => {
                                if s.import_kind == oxc::ast::ast::ImportOrExportKind::Type {
                                    continue;
                                }
                                local_names.push(s.local.name.to_string());
                            }
                            oxc::ast::ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                                local_names.push(s.local.name.to_string());
                            }
                            oxc::ast::ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                                local_names.push(s.local.name.to_string());
                            }
                        }
                    }
                }

                if !local_names.is_empty() {
                    imports_to_rewrite.push(ImportInfo {
                        mock_path: import_source.to_string(),
                        proxy_var: format!("__htMock{}__", idx),
                        import_start: import_decl.span.start,
                        import_end: import_decl.span.end,
                        local_names,
                    });
                }
            }
        }
    }

    if imports_to_rewrite.is_empty() {
        return None;
    }

    // Now use OXC semantic analysis to find all references to imported names
    // and collect their spans for replacement.
    let semantic_ret = oxc::semantic::SemanticBuilder::new()
        .build(&ret.program);

    let semantic = &semantic_ret.semantic;
    let scoping = semantic.scoping();
    let nodes = semantic.nodes();

    // Collect all replacements: (start, end, replacement_text)
    let mut replacements: Vec<(u32, u32, String)> = Vec::new();

    // 1. Replace import declarations with Proxy wrappers
    for info in &imports_to_rewrite {
        let proxy_code = format!(
            "var {} = new Proxy({{}}, {{ get: function(_t,_p) {{ \
             var _fm = globalThis.__HT_file_mocks; \
             var _f = globalThis.__currentTestFile; \
             var _m = _fm && _f && _fm[_f] && _fm[_f]['{}']; \
             return _m ? _m[_p] : _t[_p]; }} }});",
            info.proxy_var, info.mock_path,
        );
        replacements.push((info.import_start, info.import_end, proxy_code));
    }

    // 2. Find all references to imported names and replace with proxy accessor.
    for info in &imports_to_rewrite {
        for local_name in &info.local_names {
            for symbol_id in scoping.symbol_ids() {
                if scoping.symbol_name(symbol_id) != local_name {
                    continue;
                }
                // Only top-level bindings (root/module scope)
                let scope_id = scoping.symbol_scope_id(symbol_id);
                if scope_id != scoping.root_scope_id() {
                    continue;
                }
                // Found the import binding — replace all references
                for reference in scoping.get_resolved_references(symbol_id) {
                    let node = nodes.get_node(reference.node_id());
                    use oxc::span::GetSpan;
                    let span = node.span();
                    replacements.push((
                        span.start,
                        span.end,
                        format!("{}.{}", info.proxy_var, local_name),
                    ));
                }
                break;
            }
        }
    }

    // Sort replacements in reverse order by start position for safe string splicing
    replacements.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = source.to_string();
    for (start, end, replacement) in &replacements {
        let s = *start as usize;
        let e = *end as usize;
        if s <= result.len() && e <= result.len() && s <= e {
            result = format!("{}{}{}", &result[..s], replacement, &result[e..]);
        }
    }

    Some(result)
}
