use crate::core::types::Range;
use crate::symbols::*;
use crate::utils::{
    block_type_from_kind, node_text, node_to_range, BLOCK_KINDS_EXPR, BLOCK_KINDS_STATEMENT,
};

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::{Node, Tree};

#[cfg(feature = "native")]
use crate::tree_sitter_bindings::create_parser;

// For WASM, we'll use the facade types
#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::{Node, Tree};

#[cfg(all(test, feature = "native"))]
mod tests;

/// Parse a WAT document and extract symbols (PUBLIC API - unchanged)
#[cfg(feature = "native")]
pub fn parse_document(text: &str) -> Result<SymbolTable, String> {
    let mut parser = create_parser();
    let tree = parser
        .parse(text, None)
        .ok_or_else(|| "Failed to parse document".to_string())?;

    extract_symbols(&tree, text)
}

/// Parse a WAT document from a pre-parsed tree (works for both native and WASM)
pub fn parse_document_from_tree(tree: &Tree, text: &str) -> Result<SymbolTable, String> {
    extract_symbols(tree, text)
}

/// Extract all symbols from the parse tree
fn extract_symbols(tree: &Tree, source: &str) -> Result<SymbolTable, String> {
    let mut symbol_table = SymbolTable::new();
    let root = tree.root_node();

    // Extract types FIRST so they're available for type_use resolution in imports and functions
    extract_types(&root, source, &mut symbol_table);

    // Extract imports - imports get indices before regular declarations
    // Returns counters for each kind of import
    let import_counts = extract_imports(&root, source, &mut symbol_table);
    symbol_table.num_imported_globals = import_counts.globals;

    // Extract in order: globals, tables, memories, functions
    // (Order matters for index assignment)
    // Pass import counts to offset the indices
    extract_globals_with_offset(&root, source, &mut symbol_table, import_counts.globals);
    extract_tables_with_offset(&root, source, &mut symbol_table, import_counts.tables);
    extract_memories_with_offset(&root, source, &mut symbol_table, import_counts.memories);
    extract_tags_with_offset(&root, source, &mut symbol_table, import_counts.tags);
    extract_functions_with_offset(&root, source, &mut symbol_table, import_counts.functions);

    // Extract data and elem segments
    extract_data_segments(&root, source, &mut symbol_table);
    extract_elem_segments(&root, source, &mut symbol_table);

    Ok(symbol_table)
}

/// Counters for imported items
struct ImportCounts {
    functions: usize,
    globals: usize,
    tables: usize,
    memories: usize,
    tags: usize,
}

/// Extract imports from the module
fn extract_imports(root: &Node, source: &str, symbol_table: &mut SymbolTable) -> ImportCounts {
    let mut counts = ImportCounts {
        functions: 0,
        globals: 0,
        tables: 0,
        memories: 0,
        tags: 0,
    };

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        process_import_field(&field_child, source, symbol_table, &mut counts);
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                process_import_field(&field_child, source, symbol_table, &mut counts);
            }
        }
    }

    counts
}

/// Process a single module field for imports (both standalone and inline forms).
fn process_import_field(
    field_child: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    counts: &mut ImportCounts,
) {
    node_kind!(kind = field_child);

    match kind {
        "module_field_import" => {
            extract_single_import(field_child, source, symbol_table, counts);
        }
        "module_field_func" if node_has_import_child(field_child) => {
            if let Some(func) =
                extract_inline_import_func(field_child, source, counts.functions, symbol_table)
            {
                symbol_table.add_function(func);
                counts.functions += 1;
            }
        }
        "module_field_global" if node_has_import_child(field_child) => {
            if let Some(global) = extract_global(field_child, source, counts.globals) {
                symbol_table.add_global(global);
                counts.globals += 1;
            }
        }
        "module_field_table" if node_has_import_child(field_child) => {
            if let Some(table) = extract_table(field_child, source, counts.tables) {
                symbol_table.add_table(table);
                counts.tables += 1;
            }
        }
        "module_field_memory" if node_has_import_child(field_child) => {
            if let Some(memory) = extract_memory(field_child, source, counts.memories) {
                symbol_table.add_memory(memory);
                counts.memories += 1;
            }
        }
        "module_field_tag" if node_has_import_child(field_child) => {
            if let Some(tag) = extract_tag(field_child, source, counts.tags) {
                symbol_table.add_tag(tag);
                counts.tags += 1;
            }
        }
        _ => {}
    }
}

/// Check if a node has an `import` child (inline import syntax).
fn node_has_import_child(node: &Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        node_kind!(kind = child);
        if kind == "import" {
            return true;
        }
    }
    false
}

/// Extract a function from inline import syntax: `(func $name (import "M" "f") (param i32) (result i32))`
fn extract_inline_import_func(
    func_node: &Node,
    source: &str,
    index: usize,
    symbol_table: &SymbolTable,
) -> Option<Function> {
    let (name, name_range) = extract_identifier_info(func_node, source);

    let mut parameters = extract_parameters(func_node, source);
    let mut results = extract_results(func_node, source);

    // Resolve type_use if no explicit params/results
    if results.is_empty() {
        if let Some((type_params, type_results)) = resolve_type_use(func_node, source, symbol_table)
        {
            results = type_results;
            if parameters.is_empty() {
                parameters = type_params;
            }
        }
    }

    Some(Function {
        name,
        index,
        parameters,
        results,
        locals: Vec::new(),
        blocks: Vec::new(),
        line: func_node.range().start_point.row as u32,
        end_line: func_node.range().end_point.row as u32,
        start_byte: func_node.start_byte(),
        end_byte: func_node.end_byte(),
        range: name_range,
        doc_comment: None,
        has_type_use: has_type_use_child(func_node),
    })
}

/// Extract a single import declaration
fn extract_single_import(
    import_node: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    counts: &mut ImportCounts,
) {
    let mut cursor = import_node.walk();

    // Find the import_desc wrapper which contains the actual import descriptor
    for child in import_node.children(&mut cursor) {
        if child.kind() == "import_desc" {
            // Look inside import_desc for the specific import type
            let mut desc_cursor = child.walk();
            for desc_child in child.children(&mut desc_cursor) {
                node_kind!(kind = desc_child);
                match kind {
                    "import_desc_func_type" | "import_desc_type_use" => {
                        // Imported function: (func $name? ...)
                        if let Some(func) = extract_imported_function(
                            &desc_child,
                            source,
                            counts.functions,
                            symbol_table,
                        ) {
                            symbol_table.add_function(func);
                            counts.functions += 1;
                        }
                    }
                    "import_desc_global_type" => {
                        // Imported global: (global $name? global_type)
                        if let Some(global) =
                            extract_imported_global(&desc_child, source, counts.globals)
                        {
                            symbol_table.add_global(global);
                            counts.globals += 1;
                        }
                    }
                    "import_desc_table_type" => {
                        // Imported table: (table $name? table_type)
                        if let Some(table) =
                            extract_imported_table(&desc_child, source, counts.tables)
                        {
                            symbol_table.add_table(table);
                            counts.tables += 1;
                        }
                    }
                    "import_desc_memory_type" => {
                        // Imported memory: (memory $name? memory_type)
                        if let Some(memory) =
                            extract_imported_memory(&desc_child, source, counts.memories)
                        {
                            symbol_table.add_memory(memory);
                            counts.memories += 1;
                        }
                    }
                    "import_desc_tag_type" => {
                        // Imported tag: (tag $name? (param ...)?)
                        if let Some(tag) = extract_imported_tag(&desc_child, source, counts.tags) {
                            symbol_table.add_tag(tag);
                            counts.tags += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Extract an imported function
fn extract_imported_function(
    desc_node: &Node,
    source: &str,
    index: usize,
    symbol_table: &SymbolTable,
) -> Option<Function> {
    let (name, name_range) = extract_identifier_info(desc_node, source);

    // For imports, parameters and results can be in multiple func_type nodes:
    // import_desc_func_type -> func_type (params) + func_type (results)
    // Or they could be combined. We need to accumulate all params and results.
    let mut parameters = Vec::new();
    let mut results = Vec::new();

    let mut cursor = desc_node.walk();
    for child in desc_node.children(&mut cursor) {
        if child.kind() == "func_type" {
            // Extract from the func_type node and accumulate
            let new_params = extract_parameters(&child, source);
            let new_results = extract_results(&child, source);
            if !new_params.is_empty() {
                // Re-index parameters to continue the sequence
                let base_idx = parameters.len();
                for mut p in new_params {
                    p.index += base_idx;
                    parameters.push(p);
                }
            }
            if !new_results.is_empty() {
                results.extend(new_results);
            }
        } else if child.kind() == "func_type_params" {
            // Direct params
            let new_params = extract_parameters(desc_node, source);
            if !new_params.is_empty() {
                let base_idx = parameters.len();
                for mut p in new_params {
                    p.index += base_idx;
                    parameters.push(p);
                }
            }
        } else if child.kind() == "func_type_results" {
            let new_results = extract_results(desc_node, source);
            results.extend(new_results);
        }
    }

    // Also check for params directly under desc_node (fallback)
    if parameters.is_empty() {
        parameters = extract_parameters(desc_node, source);
    }
    if results.is_empty() {
        results = extract_results(desc_node, source);
    }

    // Resolve type_use if no explicit params/results
    if results.is_empty() {
        if let Some((type_params, type_results)) = resolve_type_use(desc_node, source, symbol_table)
        {
            results = type_results;
            if parameters.is_empty() {
                parameters = type_params;
            }
        }
    }

    Some(Function {
        name,
        index,
        parameters,
        results,
        locals: Vec::new(), // Imported functions have no locals
        blocks: Vec::new(), // Imported functions have no blocks
        line: desc_node.range().start_point.row as u32,
        end_line: desc_node.range().end_point.row as u32,
        start_byte: desc_node.start_byte(),
        end_byte: desc_node.end_byte(),
        range: name_range,
        doc_comment: None, // Imported functions don't have doc comments
        has_type_use: has_type_use_child(desc_node),
    })
}

/// Extract an imported global
fn extract_imported_global(desc_node: &Node, source: &str, index: usize) -> Option<Global> {
    let (name, name_range) = extract_identifier_info(desc_node, source);

    let mut is_mutable = false;
    let mut var_type = ValueType::Unknown;

    let mut cursor = desc_node.walk();
    for child in desc_node.children(&mut cursor) {
        if child.kind() == "global_type" {
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                if type_child.kind() == "global_type_mut" {
                    is_mutable = true;
                    let mut mut_cursor = type_child.walk();
                    for mut_child in type_child.children(&mut mut_cursor) {
                        if mut_child.kind() == "value_type" {
                            var_type = extract_value_type(&mut_child, source);
                        }
                    }
                } else if type_child.kind() == "global_type_imm" {
                    // Immutable globals: global_type_imm contains the type directly
                    var_type = extract_value_type(&type_child, source);
                } else if type_child.kind() == "value_type" {
                    // Fallback for other grammar structures
                    var_type = extract_value_type(&type_child, source);
                }
            }
        }
    }

    Some(Global {
        name,
        index,
        var_type,
        is_mutable,
        initial_value: None,
        line: desc_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract an imported table
fn extract_imported_table(desc_node: &Node, source: &str, index: usize) -> Option<Table> {
    let (name, name_range) = extract_identifier_info(desc_node, source);

    let mut ref_type = ValueType::Funcref;
    let mut ref_type_non_nullable = false;
    let mut min_limit: u64 = 0;
    let mut max_limit: Option<u64> = None;
    let mut is_table64 = false;

    let mut cursor = desc_node.walk();
    for child in desc_node.children(&mut cursor) {
        if child.kind() == "table_type" {
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                if type_child.kind() == "limits" {
                    let mut limits_cursor = type_child.walk();
                    let mut nat_index = 0;
                    for limit_child in type_child.children(&mut limits_cursor) {
                        if limit_child.kind() == "nat" {
                            let text = node_text(&limit_child, source);
                            if let Some(num) = parse_wat_nat(text) {
                                if nat_index == 0 {
                                    min_limit = num;
                                } else {
                                    max_limit = Some(num);
                                }
                                nat_index += 1;
                            }
                        }
                    }
                } else if type_child.kind() == "ref_type" {
                    ref_type = extract_ref_type(&type_child, source);
                    ref_type_non_nullable = is_ref_type_non_nullable(&type_child, source);
                } else if type_child.kind() == "table64_type" {
                    is_table64 = true;
                }
            }
        }
    }

    Some(Table {
        name,
        index,
        ref_type,
        ref_type_non_nullable,
        limits: (min_limit, max_limit),
        is_table64,
        line: desc_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract an imported memory
fn extract_imported_memory(desc_node: &Node, source: &str, index: usize) -> Option<Memory> {
    let (name, name_range) = extract_identifier_info(desc_node, source);

    let mut min_limit: u64 = 0;
    let mut max_limit: Option<u64> = None;
    let mut is_memory64 = false;
    let mut shared = false;

    let mut cursor = desc_node.walk();
    for child in desc_node.children(&mut cursor) {
        if child.kind() == "memory_type" {
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                // Check for memory64_type (i64 keyword) indicating memory64
                if type_child.kind() == "memory64_type" {
                    is_memory64 = true;
                }
                if type_child.kind() == "limits" {
                    let mut limits_cursor = type_child.walk();
                    let mut nat_index = 0;
                    for limit_child in type_child.children(&mut limits_cursor) {
                        if limit_child.kind() == "nat" {
                            let text = node_text(&limit_child, source);
                            if let Ok(num) = text.parse::<u64>() {
                                if nat_index == 0 {
                                    min_limit = num;
                                } else {
                                    max_limit = Some(num);
                                }
                                nat_index += 1;
                            }
                        }
                        // Check for shared/unshared keyword
                        if limit_child.kind() == "share" {
                            let share_text = node_text(&limit_child, source);
                            shared = share_text == "shared";
                        }
                    }
                }
            }
        }
    }

    Some(Memory {
        name,
        index,
        limits: (min_limit, max_limit),
        is_memory64,
        shared,
        line: desc_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract an imported tag
fn extract_imported_tag(desc_node: &Node, source: &str, index: usize) -> Option<Tag> {
    let (name, name_range) = extract_identifier_info(desc_node, source);

    let mut params = Vec::new();

    // Look for func_type_params in the import_desc_tag_type node
    let mut cursor = desc_node.walk();
    for child in desc_node.children(&mut cursor) {
        if child.kind() == "func_type_params" {
            let mut params_cursor = child.walk();
            for param_child in child.children(&mut params_cursor) {
                if param_child.kind() == "func_type_params_one"
                    || param_child.kind() == "func_type_params_many"
                {
                    let mut param_type_cursor = param_child.walk();
                    for type_child in param_child.children(&mut param_type_cursor) {
                        if type_child.kind() == "value_type" {
                            params.push(extract_value_type(&type_child, source));
                        }
                    }
                } else if param_child.kind() == "value_type" {
                    // Direct value_type children
                    params.push(extract_value_type(&param_child, source));
                }
            }
        }
    }

    Some(Tag {
        name,
        index,
        params,
        line: desc_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract name and name_range from a node's identifier child.
/// This pattern is used across all symbol extraction functions.
fn extract_identifier_info(node: &Node, source: &str) -> (Option<String>, Option<Range>) {
    if let Some(id_node) = crate::utils::find_child_by_kind(node, "identifier") {
        (
            Some(node_text(&id_node, source).to_string()),
            Some(node_to_range(&id_node)),
        )
    } else {
        (None, None)
    }
}

/// Extract function symbols using tree-sitter AST traversal (with offset for imports)
fn extract_functions_with_offset(
    root: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    start_index: usize,
) {
    let mut cursor = root.walk();
    let mut func_index = start_index;

    // Track seen function names to deduplicate (error recovery can create duplicates)
    // Also track by byte range for unnamed functions
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_ranges: std::collections::HashSet<(usize, usize)> =
        std::collections::HashSet::new();

    let mut try_add_func =
        |field_child: &Node,
         func_index: &mut usize,
         seen_names: &mut std::collections::HashSet<String>,
         seen_ranges: &mut std::collections::HashSet<(usize, usize)>| {
            if field_child.kind() == "module_field_func" && !node_has_import_child(field_child) {
                if let Some(func) = extract_function(field_child, source, *func_index, symbol_table)
                {
                    let is_duplicate = func.name.as_ref().map_or(
                        seen_ranges.contains(&(func.start_byte, func.end_byte)),
                        |n| seen_names.contains(n),
                    );
                    if !is_duplicate {
                        if let Some(ref name) = func.name {
                            seen_names.insert(name.clone());
                        } else {
                            seen_ranges.insert((func.start_byte, func.end_byte));
                        }
                        symbol_table.add_function(func);
                        *func_index += 1;
                    }
                }
            }
        };

    // Walk through all children of root (could be module or direct module_field)
    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        try_add_func(
                            &field_child,
                            &mut func_index,
                            &mut seen_names,
                            &mut seen_ranges,
                        );
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                try_add_func(
                    &field_child,
                    &mut func_index,
                    &mut seen_names,
                    &mut seen_ranges,
                );
            }
        }
    }
}

/// Extract doc comment from comments immediately preceding a node.
/// Looks for comment nodes that appear on lines immediately before the target node,
/// similar to how JSDoc works.
///
/// Tree structure: module_field_func's parent is module_field, whose parent is module.
/// Comments appear as siblings of module_field under the module node.
fn extract_doc_comment(node: &Node, source: &str) -> Option<String> {
    let node_start_line = node.range().start_point.row;
    if node_start_line == 0 {
        return None;
    }

    // module_field_func -> module_field -> module
    // We need to find comments that are siblings of module_field (our grandparent)
    let parent = node.parent()?; // module_field
    let grandparent = parent.parent()?; // module

    let parent_start_byte = parent.start_byte();

    let mut comments = Vec::new();
    let mut cursor = grandparent.walk();

    // Collect all comment nodes that appear before the module_field
    for sibling in grandparent.children(&mut cursor) {
        // Stop when we reach or pass our target node's parent
        if sibling.start_byte() >= parent_start_byte {
            break;
        }

        node_kind!(kind = sibling);

        if kind == "comment_line" || kind == "comment_block" {
            let comment_end_line = sibling.range().end_point.row;
            comments.push((comment_end_line, node_text(&sibling, source)));
        }
    }

    if comments.is_empty() {
        return None;
    }

    // Filter to only include comments that form a contiguous block ending at the line before the function
    let mut relevant_comments = Vec::new();
    let mut expected_line = node_start_line - 1;

    // Process comments in reverse order (from closest to function to furthest)
    for (end_line, text) in comments.into_iter().rev() {
        // Allow for blank lines between comments (up to 1 blank line)
        if end_line <= expected_line && expected_line - end_line <= 1 {
            relevant_comments.push(text);
            // Update expected line to look for comments before this one
            expected_line = end_line.saturating_sub(1);
        } else if relevant_comments.is_empty() {
            // If we haven't found any comments yet and this one isn't adjacent, skip
            continue;
        } else {
            // Stop if there's a gap
            break;
        }
    }

    if relevant_comments.is_empty() {
        return None;
    }

    // Reverse to get comments in original order (top to bottom)
    relevant_comments.reverse();

    // Process comment text: remove comment delimiters and clean up
    let processed: Vec<String> = relevant_comments
        .iter()
        .map(|c| {
            let trimmed = c.trim();
            if let Some(stripped) = trimmed.strip_prefix(";;") {
                // Line comment: remove ;; prefix
                stripped.trim().to_string()
            } else if let Some(inner) = trimmed
                .strip_prefix("(;")
                .and_then(|s| s.strip_suffix(";)"))
            {
                // Block comment: remove (; and ;) delimiters
                // Handle multi-line block comments by preserving line structure
                inner
                    .lines()
                    .map(|line| {
                        let l = line.trim();
                        // Remove leading * if present (common doc comment style)
                        l.strip_prefix('*').map(|s| s.trim()).unwrap_or(l)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim()
                    .to_string()
            } else {
                trimmed.to_string()
            }
        })
        .filter(|s| !s.is_empty())
        .collect();

    if processed.is_empty() {
        None
    } else {
        Some(processed.join("\n"))
    }
}

/// Extract a single function from a func node
fn extract_function(
    func_node: &Node,
    source: &str,
    index: usize,
    symbol_table: &SymbolTable,
) -> Option<Function> {
    let range = func_node.range();

    // Try to find function name and its range
    let (name, name_range) = extract_identifier_info(func_node, source);

    // Extract parameters, results, locals, and blocks
    let mut parameters = extract_parameters(func_node, source);
    let mut results = extract_results(func_node, source);
    let locals = extract_locals(func_node, source);
    let blocks = extract_blocks(func_node, source);
    let has_type_use = has_type_use_child(func_node);

    // If results (and/or params) are empty but there's a (type $t) reference, resolve from the type
    if results.is_empty() {
        if let Some((type_params, type_results)) = resolve_type_use(func_node, source, symbol_table)
        {
            results = type_results;
            if parameters.is_empty() {
                parameters = type_params;
            }
        }
    }

    // Extract doc comment from preceding comments
    let doc_comment = extract_doc_comment(func_node, source);

    Some(Function {
        name,
        index,
        parameters,
        results,
        locals,
        blocks,
        line: range.start_point.row as u32,
        end_line: range.end_point.row as u32,
        start_byte: func_node.start_byte(),
        end_byte: func_node.end_byte(),
        range: name_range,
        doc_comment,
        has_type_use,
    })
}

/// Extract parameters from a function node
fn extract_parameters(func_node: &Node, source: &str) -> Vec<Parameter> {
    let mut parameters = Vec::new();
    let mut cursor = func_node.walk();
    let mut param_index = 0;

    // Traverse child nodes looking for func_type_params
    for child in func_node.children(&mut cursor) {
        if child.kind() == "func_type_params" {
            // Extract param from func_type_params node
            let mut params_cursor = child.walk();
            for param_child in child.children(&mut params_cursor) {
                if param_child.kind() == "func_type_params_one" {
                    // Named parameter: (param $name type)
                    let mut name_opt = None;
                    let mut type_opt = None;
                    let mut range_opt = None;

                    let mut one_cursor = param_child.walk();
                    for one_child in param_child.children(&mut one_cursor) {
                        if one_child.kind() == "identifier" {
                            name_opt = Some(node_text(&one_child, source).to_string());
                            range_opt = Some(node_to_range(&one_child));
                        } else if one_child.kind() == "value_type" {
                            type_opt = Some(extract_value_type(&one_child, source));
                        }
                    }

                    if let Some(param_type) = type_opt {
                        parameters.push(Parameter {
                            name: name_opt,
                            param_type,
                            index: param_index,
                            range: range_opt,
                        });
                        param_index += 1;
                    }
                } else if param_child.kind() == "func_type_params_many" {
                    // Unnamed parameters: (param type type ...)
                    let mut many_cursor = param_child.walk();
                    for many_child in param_child.children(&mut many_cursor) {
                        if many_child.kind() == "value_type" {
                            parameters.push(Parameter {
                                name: None,
                                param_type: extract_value_type(&many_child, source),
                                index: param_index,
                                range: None,
                            });
                            param_index += 1;
                        }
                    }
                }
            }
        }
    }

    parameters
}

/// Extract result types from a function node
fn extract_results(func_node: &Node, source: &str) -> Vec<ValueType> {
    let mut results = Vec::new();
    let mut cursor = func_node.walk();

    for child in func_node.children(&mut cursor) {
        if child.kind() == "func_type_results" {
            // Extract value_type from func_type_results
            let mut results_cursor = child.walk();
            for result_child in child.children(&mut results_cursor) {
                if result_child.kind() == "value_type" {
                    results.push(extract_value_type(&result_child, source));
                }
            }
        }
    }

    results
}

/// Check if a node has a `type_use` child (i.e., `(type ...)` reference).
fn has_type_use_child(node: &Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_use" {
            return true;
        }
    }
    false
}

/// Resolve params and results from a `(type $t)` reference on a function node.
/// Returns `Some((params, results))` if a type_use was found and resolved.
fn resolve_type_use(
    func_node: &Node,
    source: &str,
    symbol_table: &SymbolTable,
) -> Option<(Vec<Parameter>, Vec<ValueType>)> {
    let mut cursor = func_node.walk();
    for child in func_node.children(&mut cursor) {
        if child.kind() == "type_use" {
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                node_kind!(kind = type_child);

                if kind == "index" || kind == "identifier" {
                    let type_ref = source[type_child.byte_range()].trim();
                    let type_def = if let Some(td) = symbol_table.get_type_by_name(type_ref) {
                        Some(td)
                    } else if let Ok(idx) = type_ref.parse::<usize>() {
                        symbol_table.get_type_by_index(idx)
                    } else {
                        None
                    };

                    if let Some(td) = type_def {
                        if let TypeKind::Func { params, results } = &td.kind {
                            let parameters = params
                                .iter()
                                .enumerate()
                                .map(|(i, vt)| Parameter {
                                    name: None,
                                    param_type: *vt,
                                    index: i,
                                    range: None,
                                })
                                .collect();
                            return Some((parameters, results.clone()));
                        }
                    }

                    // Fallback: try implicit type resolution for numeric indices
                    // beyond the explicit type count (e.g., types created by inline
                    // function signatures)
                    if let Ok(idx) = type_ref.parse::<usize>() {
                        if let Some((params, results)) = resolve_implicit_type(idx, symbol_table) {
                            let parameters = params
                                .iter()
                                .enumerate()
                                .map(|(i, vt)| Parameter {
                                    name: None,
                                    param_type: *vt,
                                    index: i,
                                    range: None,
                                })
                                .collect();
                            return Some((parameters, results));
                        }
                    }
                }
            }
        }
    }
    None
}

/// Resolve a type index that may refer to an implicit type created by
/// inline function signatures. Builds the implicit type table:
/// explicit func types + unique function signatures not matching any explicit type.
/// Only functions without type_use create implicit types.
fn resolve_implicit_type(
    idx: usize,
    symbol_table: &SymbolTable,
) -> Option<(Vec<ValueType>, Vec<ValueType>)> {
    let mut sigs: Vec<(Vec<ValueType>, Vec<ValueType>)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for type_def in &symbol_table.types {
        if let TypeKind::Func { params, results } = &type_def.kind {
            let sig = (params.clone(), results.clone());
            seen.insert(sig.clone());
            sigs.push(sig);
        }
    }
    for func in &symbol_table.functions {
        if func.has_type_use {
            continue;
        }
        let params: Vec<ValueType> = func.parameters.iter().map(|p| p.param_type).collect();
        let sig = (params, func.results.clone());
        if seen.insert(sig.clone()) {
            sigs.push(sig);
        }
    }
    sigs.get(idx).cloned()
}

/// Extract local variables from a function node
fn extract_locals(func_node: &Node, source: &str) -> Vec<Variable> {
    let mut locals = Vec::new();
    let mut cursor = func_node.walk();
    let mut local_index = 0;

    for child in func_node.children(&mut cursor) {
        if child.kind() == "func_locals" {
            // func_locals can be func_locals_one or func_locals_many
            let mut locals_cursor = child.walk();
            for locals_child in child.children(&mut locals_cursor) {
                if locals_child.kind() == "func_locals_one" {
                    // Named local: (local $name type)
                    let mut name_opt = None;
                    let mut type_opt = None;
                    let mut range_opt = None;

                    let mut one_cursor = locals_child.walk();
                    for one_child in locals_child.children(&mut one_cursor) {
                        if one_child.kind() == "identifier" {
                            name_opt = Some(node_text(&one_child, source).to_string());
                            range_opt = Some(node_to_range(&one_child));
                        } else if one_child.kind() == "value_type" {
                            type_opt = Some(extract_value_type(&one_child, source));
                        }
                    }

                    if let Some(var_type) = type_opt {
                        locals.push(Variable {
                            name: name_opt,
                            var_type,
                            is_mutable: true,
                            index: local_index,
                            range: range_opt,
                        });
                        local_index += 1;
                    }
                } else if locals_child.kind() == "func_locals_many" {
                    // Unnamed locals: (local type type ...)
                    let mut many_cursor = locals_child.walk();
                    for many_child in locals_child.children(&mut many_cursor) {
                        if many_child.kind() == "value_type" {
                            locals.push(Variable {
                                name: None,
                                var_type: extract_value_type(&many_child, source),
                                is_mutable: true,
                                index: local_index,
                                range: None,
                            });
                            local_index += 1;
                        }
                    }
                }
            }
        }
    }

    locals
}

/// Extract block labels from a function node
fn extract_blocks(func_node: &Node, source: &str) -> Vec<BlockLabel> {
    let mut blocks = Vec::new();

    // Recursively traverse function body looking for labeled blocks
    visit_node_for_blocks(func_node, source, &mut blocks);

    blocks
}

/// Recursively visit nodes to find labeled blocks/loops/ifs
fn visit_node_for_blocks(node: &Node, source: &str, blocks: &mut Vec<BlockLabel>) {
    node_kind!(kind_str = node);

    // Check both statement form (block_block) and expression form (expr1_block)
    if BLOCK_KINDS_STATEMENT.contains(&kind_str) || BLOCK_KINDS_EXPR.contains(&kind_str) {
        // Check if it has a label
        if let Some(id_node) = crate::utils::find_child_by_kind(node, "identifier") {
            let label = node_text(&id_node, source).to_string();
            let block_type = block_type_from_kind(kind_str).to_string();

            blocks.push(BlockLabel {
                label,
                block_type,
                line: node.range().start_point.row as u32,
                range: Some(node_to_range(&id_node)),
            });
        }
    }

    // Recurse to children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_node_for_blocks(&child, source, blocks);
    }
}

/// Extract global variables (with offset for imports)
fn extract_globals_with_offset(
    root: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    start_index: usize,
) {
    let mut cursor = root.walk();
    let mut global_index = start_index;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_global"
                            && !node_has_import_child(&field_child)
                        {
                            if let Some(global) = extract_global(&field_child, source, global_index)
                            {
                                symbol_table.add_global(global);
                                global_index += 1;
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_global"
                    && !node_has_import_child(&field_child)
                {
                    if let Some(global) = extract_global(&field_child, source, global_index) {
                        symbol_table.add_global(global);
                        global_index += 1;
                    }
                }
            }
        }
    }
}

/// Extract a single global from a global node
fn extract_global(global_node: &Node, source: &str, index: usize) -> Option<Global> {
    let (name, name_range) = extract_identifier_info(global_node, source);

    let mut is_mutable = false;
    let mut var_type = ValueType::Unknown;

    let mut cursor = global_node.walk();
    for child in global_node.children(&mut cursor) {
        if child.kind() == "global_type" {
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                if type_child.kind() == "global_type_mut" {
                    is_mutable = true;
                    // value_type is nested inside global_type_mut
                    let mut mut_cursor = type_child.walk();
                    for mut_child in type_child.children(&mut mut_cursor) {
                        if mut_child.kind() == "value_type" {
                            var_type = extract_value_type(&mut_child, source);
                        }
                    }
                } else if type_child.kind() == "global_type_imm" {
                    // Immutable globals: global_type_imm contains the type directly
                    var_type = extract_value_type(&type_child, source);
                } else if type_child.kind() == "value_type" {
                    // Fallback: Non-mutable globals might have value_type directly under global_type
                    var_type = extract_value_type(&type_child, source);
                }
            }
        }
    }

    Some(Global {
        name,
        index,
        var_type,
        is_mutable,
        initial_value: None,
        line: global_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract type definitions
fn extract_types(root: &Node, source: &str, symbol_table: &mut SymbolTable) {
    let mut cursor = root.walk();
    let mut type_index = 0;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_type" {
                            if let Some(type_def) = extract_type(&field_child, source, type_index) {
                                symbol_table.add_type(type_def);
                                type_index += 1;
                            }
                        } else if field_child.kind() == "module_field_rec" {
                            // Extract types from rec group
                            type_index =
                                extract_rec_types(&field_child, source, symbol_table, type_index);
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_type" {
                    if let Some(type_def) = extract_type(&field_child, source, type_index) {
                        symbol_table.add_type(type_def);
                        type_index += 1;
                    }
                } else if field_child.kind() == "module_field_rec" {
                    // Extract types from rec group
                    type_index = extract_rec_types(&field_child, source, symbol_table, type_index);
                }
            }
        }
    }
}

/// Extract types from a rec group
fn extract_rec_types(
    rec_node: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    mut type_index: usize,
) -> usize {
    // The rec node's children are flattened: "(", "rec", "(", "type", id?, type_field, ")", "(", "type", ..., ")", ")"
    // We need to parse this as a sequence, looking for patterns: "(" "type" [id] type_field ")"

    let mut cursor = rec_node.walk();
    let children_vec: Vec<_> = rec_node.children(&mut cursor).collect();

    let mut i = 0;
    while i < children_vec.len() {
        let child = &children_vec[i];

        // Look for pattern: "(" followed by "type"
        if child.kind() == "(" && i + 1 < children_vec.len() && children_vec[i + 1].kind() == "type"
        {
            // Found start of a type definition
            i += 2; // Skip "(" and "type"

            // Check for optional identifier
            let (name, name_range) =
                if i < children_vec.len() && children_vec[i].kind() == "identifier" {
                    let id_node = &children_vec[i];
                    i += 1;
                    (
                        Some(node_text(id_node, source).to_string()),
                        Some(node_to_range(id_node)),
                    )
                } else {
                    (None, None)
                };

            // Next should be the type_field or direct struct_type/array_type
            if i < children_vec.len() {
                let type_node = &children_vec[i];
                if type_node.kind() == "struct_type"
                    || type_node.kind() == "array_type"
                    || type_node.kind() == "type_field"
                    || type_node.kind() == "sub_type"
                {
                    // Extract the type
                    if let Some(type_def) = extract_type_from_single_node(
                        type_node, source, type_index, name, name_range,
                    ) {
                        symbol_table.add_type(type_def);
                        type_index += 1;
                    }
                    i += 1;
                }
            }

            // Skip until we find the closing ")"
            while i < children_vec.len() && children_vec[i].kind() != ")" {
                i += 1;
            }
            if i < children_vec.len() && children_vec[i].kind() == ")" {
                i += 1; // Skip the ")"
            }
        } else {
            i += 1;
        }
    }

    type_index
}

/// Extract a type from a single node (struct_type, array_type, or type_field)
fn extract_type_from_single_node(
    type_node: &Node,
    source: &str,
    index: usize,
    name: Option<String>,
    name_range: Option<Range>,
) -> Option<TypeDef> {
    let mut kind = TypeKind::Func {
        params: Vec::new(),
        results: Vec::new(),
    };
    let mut parent_ref: Option<String> = None;
    let mut is_final = false;

    node_kind!(type_kind = type_node);
    match type_kind {
        "sub_type" => {
            // Process sub_type children: (sub final? index? def_type)
            let mut sub_cursor = type_node.walk();
            for sub_child in type_node.children(&mut sub_cursor) {
                node_kind!(sub_kind = sub_child);

                match sub_kind {
                    "index" => {
                        parent_ref = Some(node_text(&sub_child, source).to_string());
                    }
                    "def_type" => {
                        let mut def_cursor = sub_child.walk();
                        for def_child in sub_child.children(&mut def_cursor) {
                            if def_child.kind() == "struct_type" {
                                kind = extract_struct_kind(&def_child, source);
                            } else if def_child.kind() == "array_type" {
                                kind = extract_array_kind(&def_child, source);
                            } else if def_child.kind() == "func_type" {
                                let mut parameters = Vec::new();
                                let mut results = Vec::new();
                                let params = extract_parameters(&def_child, source);
                                for param in params {
                                    parameters.push(param.param_type);
                                }
                                results.extend(extract_results(&def_child, source));
                                kind = TypeKind::Func {
                                    params: parameters,
                                    results,
                                };
                            }
                        }
                    }
                    _ => {
                        if node_text(&sub_child, source) == "final" {
                            is_final = true;
                        }
                        if sub_child.kind() == "struct_type" {
                            kind = extract_struct_kind(&sub_child, source);
                        } else if sub_child.kind() == "array_type" {
                            kind = extract_array_kind(&sub_child, source);
                        }
                    }
                }
            }
        }
        "struct_type" => {
            // Extract struct fields (handles abbreviated multi-field syntax)
            let mut fields = Vec::new();
            let mut cursor = type_node.walk();
            for child in type_node.children(&mut cursor) {
                if child.kind() == "field_type" {
                    fields.extend(extract_field_types(&child, source));
                }
            }
            kind = TypeKind::Struct { fields };
        }
        "array_type" => {
            // Extract array field
            let mut element_type = ValueType::Unknown;
            let mut mutable = false;
            let mut cursor = type_node.walk();
            for child in type_node.children(&mut cursor) {
                if child.kind() == "field_type" {
                    let extracted = extract_field_types(&child, source);
                    if let Some((_, ft, m)) = extracted.into_iter().next() {
                        element_type = ft;
                        mutable = m;
                    }
                }
            }
            kind = TypeKind::Array {
                element_type,
                mutable,
            };
        }
        "type_field" => {
            // type_field wraps a def_type: func_type, struct_type, array_type, or sub_type
            let mut cursor = type_node.walk();
            for child in type_node.children(&mut cursor) {
                node_kind!(child_kind = child);
                match child_kind {
                    "func_type" => {
                        let mut parameters = Vec::new();
                        let mut results = Vec::new();
                        let params = extract_parameters(&child, source);
                        for param in params {
                            parameters.push(param.param_type);
                        }
                        results.extend(extract_results(&child, source));
                        if !parameters.is_empty() || !results.is_empty() {
                            kind = TypeKind::Func {
                                params: parameters,
                                results,
                            };
                        }
                        break;
                    }
                    "struct_type" => {
                        kind = extract_struct_kind(&child, source);
                        break;
                    }
                    "array_type" => {
                        kind = extract_array_kind(&child, source);
                        break;
                    }
                    "sub_type" => {
                        // Delegate to extract_type_from_single_node for sub_type
                        return extract_type_from_single_node(
                            &child, source, index, name, name_range,
                        );
                    }
                    _ => {}
                }
            }
        }
        _ => return None,
    }

    Some(TypeDef {
        name,
        index,
        kind,
        parent: parent_ref,
        is_final,
        line: type_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract field_type information, returning one or more fields.
///
/// Handles abbreviated multi-field syntax: `(field i32 (mut i32))` produces two fields.
/// Only the first field gets the identifier name (if present); subsequent fields are unnamed.
fn extract_field_types(field_node: &Node, source: &str) -> Vec<(Option<String>, ValueType, bool)> {
    let mut field_name = None;
    let mut fields = Vec::new();

    let mut cursor = field_node.walk();
    for child in field_node.children(&mut cursor) {
        node_kind!(ck = child);
        match ck {
            "identifier" => {
                field_name = Some(node_text(&child, source).to_string());
            }
            "storage_type" => {
                let (ft, mutable) = extract_storage_type_with_mut(&child, source);
                // First field gets the name, subsequent fields are unnamed
                let name = if fields.is_empty() {
                    field_name.clone()
                } else {
                    None
                };
                fields.push((name, ft, mutable));
            }
            _ => {}
        }
    }

    // Fallback for field_type nodes that use value_type or packed_type directly
    // (without a storage_type wrapper)
    if fields.is_empty() {
        let mut field_type = ValueType::Unknown;
        let mut mutable = false;
        let mut cursor2 = field_node.walk();
        for child in field_node.children(&mut cursor2) {
            node_kind!(ck = child);
            match ck {
                "value_type" => {
                    field_type = extract_value_type(&child, source);
                }
                "packed_type" => {
                    let text = node_text(&child, source);
                    field_type = match text {
                        "i8" => ValueType::I8,
                        "i16" => ValueType::I16,
                        _ => ValueType::Unknown,
                    };
                }
                _ => {}
            }
        }
        let text = node_text(field_node, source);
        if text.contains("(mut") || text.contains(" mut ") {
            mutable = true;
        }
        fields.push((field_name, field_type, mutable));
    }

    fields
}

/// Extract a single type definition from a type node
fn extract_type(type_node: &Node, source: &str, index: usize) -> Option<TypeDef> {
    let (name, name_range) = extract_identifier_info(type_node, source);

    let mut parameters = Vec::new();
    let mut results = Vec::new();
    let mut kind = TypeKind::Func {
        params: Vec::new(),
        results: Vec::new(),
    };
    let mut parent_ref: Option<String> = None;
    let mut is_final = false;
    let mut cursor = type_node.walk();
    for child in type_node.children(&mut cursor) {
        // Handle sub_type: (sub final? supertype_index? def_type)
        if child.kind() == "sub_type" {
            let mut sub_cursor = child.walk();
            for sub_child in child.children(&mut sub_cursor) {
                node_kind!(sub_kind = sub_child);

                match sub_kind {
                    "index" => {
                        parent_ref = Some(node_text(&sub_child, source).to_string());
                    }
                    "def_type" => {
                        // Process the inner type definition
                        let mut def_cursor = sub_child.walk();
                        for def_child in sub_child.children(&mut def_cursor) {
                            if def_child.kind() == "struct_type" {
                                kind = extract_struct_kind(&def_child, source);
                            } else if def_child.kind() == "array_type" {
                                kind = extract_array_kind(&def_child, source);
                            } else if def_child.kind() == "func_type" {
                                let params = extract_parameters(&def_child, source);
                                for param in params {
                                    parameters.push(param.param_type);
                                }
                                results.extend(extract_results(&def_child, source));
                            }
                        }
                    }
                    _ => {
                        // Check for "final" keyword
                        if node_text(&sub_child, source) == "final" {
                            is_final = true;
                        }
                        // Check if this is a type definition directly
                        if sub_child.kind() == "struct_type" {
                            kind = extract_struct_kind(&sub_child, source);
                        } else if sub_child.kind() == "array_type" {
                            kind = extract_array_kind(&sub_child, source);
                        }
                    }
                }
            }
            continue;
        }
        // Check if this child is directly a struct_type or array_type
        // (happens when type_field is just the type itself)
        if child.kind() == "struct_type" {
            // Extract struct fields directly (handles abbreviated multi-field syntax)
            let mut fields = Vec::new();
            let mut struct_cursor = child.walk();

            for struct_child in child.children(&mut struct_cursor) {
                if struct_child.kind() == "field_type" {
                    fields.extend(extract_field_types(&struct_child, source));
                }
            }
            kind = TypeKind::Struct { fields };
        } else if child.kind() == "array_type" {
            // Extract array field directly
            let mut element_type = ValueType::Unknown;
            let mut mutable = false;

            let mut array_cursor = child.walk();
            for array_child in child.children(&mut array_cursor) {
                if array_child.kind() == "field_type" {
                    let extracted = extract_field_types(&array_child, source);
                    if let Some((_, ft, m)) = extracted.into_iter().next() {
                        element_type = ft;
                        mutable = m;
                    }
                }
            }
            kind = TypeKind::Array {
                element_type,
                mutable,
            };
        } else if child.kind() == "type_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "func_type" {
                    // Extract parameters and results from func_type
                    // Accumulate across all func_type children
                    let params = extract_parameters(&field_child, source);
                    for param in params {
                        parameters.push(param.param_type);
                    }
                    results.extend(extract_results(&field_child, source));
                } else if field_child.kind() == "struct_type" {
                    // Extract struct fields (handles abbreviated multi-field syntax)
                    let mut fields = Vec::new();
                    let mut struct_cursor = field_child.walk();

                    for struct_child in field_child.children(&mut struct_cursor) {
                        if struct_child.kind() == "field_type" {
                            fields.extend(extract_field_types(&struct_child, source));
                        }
                    }
                    kind = TypeKind::Struct { fields };
                } else if field_child.kind() == "array_type" {
                    // Extract array field
                    // array_type: (array field_type)
                    let mut element_type = ValueType::Unknown;
                    let mut mutable = false;

                    let mut array_cursor = field_child.walk();
                    for array_child in field_child.children(&mut array_cursor) {
                        if array_child.kind() == "field_type" {
                            let extracted = extract_field_types(&array_child, source);
                            if let Some((_, ft, m)) = extracted.into_iter().next() {
                                element_type = ft;
                                mutable = m;
                            }
                        }
                    }
                    kind = TypeKind::Array {
                        element_type,
                        mutable,
                    };
                } else if field_child.kind() == "sub_type" {
                    // Handle sub_type inside type_field: (sub final? index? def_type)
                    let mut sub_cursor = field_child.walk();
                    for sub_child in field_child.children(&mut sub_cursor) {
                        node_kind!(sub_kind = sub_child);

                        match sub_kind {
                            "index" => {
                                parent_ref = Some(node_text(&sub_child, source).to_string());
                            }
                            "def_type" => {
                                let mut def_cursor = sub_child.walk();
                                for def_child in sub_child.children(&mut def_cursor) {
                                    if def_child.kind() == "struct_type" {
                                        kind = extract_struct_kind(&def_child, source);
                                    } else if def_child.kind() == "array_type" {
                                        kind = extract_array_kind(&def_child, source);
                                    } else if def_child.kind() == "func_type" {
                                        let params = extract_parameters(&def_child, source);
                                        for param in params {
                                            parameters.push(param.param_type);
                                        }
                                        results.extend(extract_results(&def_child, source));
                                    }
                                }
                            }
                            _ => {
                                if node_text(&sub_child, source) == "final" {
                                    is_final = true;
                                }
                                if sub_child.kind() == "struct_type" {
                                    kind = extract_struct_kind(&sub_child, source);
                                } else if sub_child.kind() == "array_type" {
                                    kind = extract_array_kind(&sub_child, source);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Update kind if we accumulated func parameters/results
    if !parameters.is_empty() || !results.is_empty() {
        kind = TypeKind::Func {
            params: parameters,
            results,
        };
    }

    Some(TypeDef {
        name,
        index,
        kind,
        parent: parent_ref,
        is_final,
        line: type_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Helper to extract struct kind from a struct_type node
fn extract_struct_kind(struct_node: &Node, source: &str) -> TypeKind {
    let mut fields = Vec::new();
    let mut struct_cursor = struct_node.walk();

    for struct_child in struct_node.children(&mut struct_cursor) {
        if struct_child.kind() == "field_type" {
            fields.extend(extract_field_types(&struct_child, source));
        }
    }
    TypeKind::Struct { fields }
}

/// Helper to extract array kind from an array_type node
fn extract_array_kind(array_node: &Node, source: &str) -> TypeKind {
    let mut element_type = ValueType::Unknown;
    let mut mutable = false;

    let mut array_cursor = array_node.walk();
    for array_child in array_node.children(&mut array_cursor) {
        if array_child.kind() == "field_type" {
            let extracted = extract_field_types(&array_child, source);
            if let Some((_, ft, m)) = extracted.into_iter().next() {
                element_type = ft;
                mutable = m;
            }
        }
    }
    TypeKind::Array {
        element_type,
        mutable,
    }
}

/// Extract table definitions (with offset for imports)
fn extract_tables_with_offset(
    root: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    start_index: usize,
) {
    let mut cursor = root.walk();
    let mut table_index = start_index;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_table"
                            && !node_has_import_child(&field_child)
                        {
                            if let Some(table) = extract_table(&field_child, source, table_index) {
                                symbol_table.add_table(table);
                                table_index += 1;
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_table"
                    && !node_has_import_child(&field_child)
                {
                    if let Some(table) = extract_table(&field_child, source, table_index) {
                        symbol_table.add_table(table);
                        table_index += 1;
                    }
                }
            }
        }
    }
}

/// Extract a single table from a table node
fn extract_table(table_node: &Node, source: &str, index: usize) -> Option<Table> {
    let (name, name_range) = extract_identifier_info(table_node, source);

    let mut ref_type = ValueType::Funcref;
    let mut ref_type_non_nullable = false;
    let mut min_limit: u64 = 0;
    let mut max_limit: Option<u64> = None;
    let mut is_table64 = false;

    let mut cursor = table_node.walk();
    for child in table_node.children(&mut cursor) {
        // Handle table_fields_type wrapper
        if child.kind() == "table_fields_type" {
            let mut fields_cursor = child.walk();
            for fields_child in child.children(&mut fields_cursor) {
                if fields_child.kind() == "table_type" {
                    let mut type_cursor = fields_child.walk();
                    for type_child in fields_child.children(&mut type_cursor) {
                        if type_child.kind() == "limits" {
                            let mut limits_cursor = type_child.walk();
                            let mut nat_index = 0;
                            for limit_child in type_child.children(&mut limits_cursor) {
                                if limit_child.kind() == "nat" {
                                    let text = node_text(&limit_child, source);
                                    if let Some(num) = parse_wat_nat(text) {
                                        if nat_index == 0 {
                                            min_limit = num;
                                        } else {
                                            max_limit = Some(num);
                                        }
                                        nat_index += 1;
                                    }
                                }
                            }
                        } else if type_child.kind() == "ref_type" {
                            ref_type = extract_ref_type(&type_child, source);
                            ref_type_non_nullable = is_ref_type_non_nullable(&type_child, source);
                        } else if type_child.kind() == "table64_type" {
                            is_table64 = true;
                        }
                    }
                }
            }
        } else if child.kind() == "table_type" {
            // Handle direct table_type (without wrapper)
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                if type_child.kind() == "limits" {
                    let mut limits_cursor = type_child.walk();
                    let mut nat_index = 0;
                    for limit_child in type_child.children(&mut limits_cursor) {
                        if limit_child.kind() == "nat" {
                            let text = node_text(&limit_child, source);
                            if let Some(num) = parse_wat_nat(text) {
                                if nat_index == 0 {
                                    min_limit = num;
                                } else {
                                    max_limit = Some(num);
                                }
                                nat_index += 1;
                            }
                        }
                    }
                } else if type_child.kind() == "ref_type" {
                    ref_type = extract_ref_type(&type_child, source);
                    ref_type_non_nullable = is_ref_type_non_nullable(&type_child, source);
                } else if type_child.kind() == "table64_type" {
                    is_table64 = true;
                }
            }
        }
    }

    Some(Table {
        name,
        index,
        ref_type,
        ref_type_non_nullable,
        limits: (min_limit, max_limit),
        is_table64,
        line: table_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract memory definitions (with offset for imports)
fn extract_memories_with_offset(
    root: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    start_index: usize,
) {
    let mut cursor = root.walk();
    let mut memory_index = start_index;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_memory"
                            && !node_has_import_child(&field_child)
                        {
                            if let Some(memory) = extract_memory(&field_child, source, memory_index)
                            {
                                symbol_table.add_memory(memory);
                                memory_index += 1;
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_memory"
                    && !node_has_import_child(&field_child)
                {
                    if let Some(memory) = extract_memory(&field_child, source, memory_index) {
                        symbol_table.add_memory(memory);
                        memory_index += 1;
                    }
                }
            }
        }
    }
}

/// Extract a single memory from a memory node
fn extract_memory(memory_node: &Node, source: &str, index: usize) -> Option<Memory> {
    let (name, name_range) = extract_identifier_info(memory_node, source);

    let mut min_limit: u64 = 0;
    let mut max_limit: Option<u64> = None;
    let mut is_memory64 = false;
    let mut shared = false;

    // Helper to extract limits from a limits node
    let extract_limits =
        |limits_node: &Node, min: &mut u64, max: &mut Option<u64>, shared_flag: &mut bool| {
            let mut limits_cursor = limits_node.walk();
            let mut nat_index = 0;
            for limit_child in limits_node.children(&mut limits_cursor) {
                if limit_child.kind() == "nat" {
                    let text = node_text(&limit_child, source);
                    if let Some(num) = parse_wat_nat(text) {
                        if nat_index == 0 {
                            *min = num;
                        } else {
                            *max = Some(num);
                        }
                        nat_index += 1;
                    }
                }
                // Check for shared/unshared keyword
                if limit_child.kind() == "share" {
                    let share_text = node_text(&limit_child, source);
                    *shared_flag = share_text == "shared";
                }
            }
        };

    let mut cursor = memory_node.walk();
    for child in memory_node.children(&mut cursor) {
        // Check for memory64_type (i64 keyword) at memory node level
        if child.kind() == "memory64_type" {
            is_memory64 = true;
        }
        if child.kind() == "memory_fields_type" {
            let mut fields_cursor = child.walk();
            for fields_child in child.children(&mut fields_cursor) {
                // Check for memory64_type in memory_fields_type
                if fields_child.kind() == "memory64_type" {
                    is_memory64 = true;
                }
                if fields_child.kind() == "memory_type" {
                    let mut type_cursor = fields_child.walk();
                    for type_child in fields_child.children(&mut type_cursor) {
                        // Check for memory64_type in memory_type
                        if type_child.kind() == "memory64_type" {
                            is_memory64 = true;
                        }
                        if type_child.kind() == "limits" {
                            extract_limits(
                                &type_child,
                                &mut min_limit,
                                &mut max_limit,
                                &mut shared,
                            );
                        }
                    }
                }
            }
        } else if child.kind() == "memory_type" {
            // Handle direct memory_type (without wrapper)
            let mut type_cursor = child.walk();
            for type_child in child.children(&mut type_cursor) {
                // Check for memory64_type in memory_type
                if type_child.kind() == "memory64_type" {
                    is_memory64 = true;
                }
                if type_child.kind() == "limits" {
                    extract_limits(&type_child, &mut min_limit, &mut max_limit, &mut shared);
                }
            }
        }
    }

    Some(Memory {
        name,
        index,
        limits: (min_limit, max_limit),
        is_memory64,
        shared,
        line: memory_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract tag definitions
fn extract_tags_with_offset(
    root: &Node,
    source: &str,
    symbol_table: &mut SymbolTable,
    offset: usize,
) {
    let mut cursor = root.walk();
    let mut tag_index = offset;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_tag"
                            && !node_has_import_child(&field_child)
                        {
                            if let Some(tag) = extract_tag(&field_child, source, tag_index) {
                                symbol_table.add_tag(tag);
                                tag_index += 1;
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_tag" && !node_has_import_child(&field_child)
                {
                    if let Some(tag) = extract_tag(&field_child, source, tag_index) {
                        symbol_table.add_tag(tag);
                        tag_index += 1;
                    }
                }
            }
        }
    }
}

/// Extract a single tag from a tag node
fn extract_tag(tag_node: &Node, source: &str, index: usize) -> Option<Tag> {
    let (name, name_range) = extract_identifier_info(tag_node, source);

    let mut params = Vec::new();

    // func_type_params is a direct child of module_field_tag (no tag_type wrapper)
    let mut cursor = tag_node.walk();
    for child in tag_node.children(&mut cursor) {
        if child.kind() == "func_type_params" {
            let mut params_cursor = child.walk();
            for param_child in child.children(&mut params_cursor) {
                if param_child.kind() == "func_type_params_one"
                    || param_child.kind() == "func_type_params_many"
                {
                    let mut param_type_cursor = param_child.walk();
                    for type_child in param_child.children(&mut param_type_cursor) {
                        if type_child.kind() == "value_type" {
                            params.push(extract_value_type(&type_child, source));
                        }
                    }
                }
            }
        }
    }

    Some(Tag {
        name,
        index,
        params,
        line: tag_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Parse a WAT natural number, handling hex (0x...) and underscore separators.
pub(crate) fn parse_wat_nat(text: &str) -> Option<u64> {
    let text = text.trim().replace('_', "");
    if text.starts_with("0x") || text.starts_with("0X") {
        u64::from_str_radix(&text[2..], 16).ok()
    } else {
        text.parse::<u64>().ok()
    }
}

/// Extract value type from a value_type node (handles nested structure)
fn extract_value_type(value_type_node: &Node, source: &str) -> ValueType {
    // Check strict match first (for direct children like "i32" in some contexts)
    let text = node_text(value_type_node, source);
    if let Some(vt) = ValueType::try_parse(text) {
        return vt;
    }

    let mut cursor = value_type_node.walk();
    for child in value_type_node.children(&mut cursor) {
        node_kind!(child_kind = child);
        match child_kind {
            "value_type_num_type" => {
                let text = node_text(&child, source);
                // Handle i8/i16 which might be direct children or nested
                if text == "i8" {
                    return ValueType::I8;
                }
                if text == "i16" {
                    return ValueType::I16;
                }

                let mut num_cursor = child.walk();
                for num_child in child.children(&mut num_cursor) {
                    node_kind!(num_kind = num_child);
                    match num_kind {
                        "num_type_i32" => return ValueType::I32,
                        "num_type_i64" => return ValueType::I64,
                        "num_type_f32" => return ValueType::F32,
                        "num_type_f64" => return ValueType::F64,
                        "num_type_v128" => return ValueType::V128,
                        _ => {
                            if node_text(&num_child, source) == "i8" {
                                return ValueType::I8;
                            }
                            if node_text(&num_child, source) == "i16" {
                                return ValueType::I16;
                            }
                        }
                    }
                }
            }
            "value_type_ref_type" => {
                // Delegate to extract_ref_type
                return extract_ref_type(&child, source);
            }
            // Direct matches for some parser structures
            "ref_type" => return extract_ref_type(&child, source),
            _ => {
                // Check if child itself is a type keyword
                let text = node_text(&child, source);
                if let Some(vt) = simple_type_from_str(text) {
                    return vt;
                }
            }
        }
    }
    ValueType::Unknown
}

/// Extract value type and mutability from a storage_type node.
///
/// Returns (type, is_mutable). A `mut_storage_type` child indicates mutability.
fn extract_storage_type_with_mut(storage_node: &Node, source: &str) -> (ValueType, bool) {
    let mut cursor = storage_node.walk();
    for child in storage_node.children(&mut cursor) {
        node_kind!(ck = child);
        match ck {
            "value_type" => return (extract_value_type(&child, source), false),
            "packed_type" => {
                let text = node_text(&child, source);
                let vt = match text {
                    "i8" => ValueType::I8,
                    "i16" => ValueType::I16,
                    _ => ValueType::Unknown,
                };
                return (vt, false);
            }
            "mut_storage_type" => {
                // Mutable wrapper — extract inner type
                let mut inner_cursor = child.walk();
                for inner_child in child.children(&mut inner_cursor) {
                    node_kind!(ik = inner_child);
                    match ik {
                        "value_type" => return (extract_value_type(&inner_child, source), true),
                        "packed_type" => {
                            let text = node_text(&inner_child, source);
                            let vt = match text {
                                "i8" => ValueType::I8,
                                "i16" => ValueType::I16,
                                _ => ValueType::Unknown,
                            };
                            return (vt, true);
                        }
                        _ => {}
                    }
                }
                return (ValueType::Unknown, true);
            }
            _ => {}
        }
    }
    (ValueType::Unknown, false)
}

/// Alias for ValueType::try_parse for convenience in tree traversal code.
/// Use this when traversing tree-sitter nodes and checking type strings.
#[inline]
fn simple_type_from_str(s: &str) -> Option<ValueType> {
    ValueType::try_parse(s)
}

/// Check if a ref_type AST node represents a non-nullable reference type.
/// Returns true for `(ref func)`, `(ref $t)`, etc. (without `null`).
/// Returns false for `funcref`, `externref`, `(ref null func)`, etc.
fn is_ref_type_non_nullable(node: &Node, source: &str) -> bool {
    node_kind!(kind = node);
    match kind {
        "ref_type_ref" | "ref_type_concrete" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if &source[child.byte_range()] == "null" {
                    return false;
                }
            }
            true
        }
        "ref_type_funcref" | "ref_type_externref" => false,
        _ => {
            let text = source[node.byte_range()].trim();
            if matches!(
                text,
                "funcref"
                    | "externref"
                    | "anyref"
                    | "eqref"
                    | "i31ref"
                    | "structref"
                    | "arrayref"
                    | "nullref"
                    | "nullfuncref"
                    | "nullexternref"
            ) {
                return false;
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if is_ref_type_non_nullable(&child, source) {
                    return true;
                }
            }
            false
        }
    }
}

/// Extract reference type from a ref_type node (handles nested structure)
fn extract_ref_type(ref_type_node: &Node, source: &str) -> ValueType {
    // Check strict match first
    let text = node_text(ref_type_node, source);
    if let Some(vt) = simple_type_from_str(text) {
        return vt;
    }

    let mut cursor = ref_type_node.walk();
    for child in ref_type_node.children(&mut cursor) {
        node_kind!(child_kind = child);
        match child_kind {
            "ref_type_funcref" => return ValueType::Funcref,
            "ref_type_externref" => return ValueType::Externref,
            "ref_type_concrete" => {
                // (ref null? $index)
                // Find index child
                let mut concrete_cursor = child.walk();
                let mut index_val = None;
                let mut nullable = false;

                for c in child.children(&mut concrete_cursor) {
                    if c.kind() == "index" {
                        // Parse index (could be numeric or identifier)
                        // For now we just store 0 or some indicator, or we need to resolve it?
                        // ValueType::Ref(u32) stores a numeric index.
                        // But if it's an identifier, we can't resolve it yet without symbol table lookup.
                        // However, Symbols extractor runs in passes. Globals using Ref types might refer to types not yet seen?
                        // No, Types are extracted before globals.
                        // But we don't have access to the SymbolTable here easily.
                        // We will rely on later resolution or just store 0 for now if identifier.
                        // Improving this requires major refactor to pass SymbolTable.
                        // For now, let's just return Ref(0) if we can't parse, or Ref(index).
                        let idx_text = node_text(&c, source);
                        if let Ok(idx) = idx_text.parse::<u32>() {
                            index_val = Some(idx);
                        } else {
                            // It's a named index ($type).
                            // We arguably should store the name in ValueType, but ValueType::Ref takes u32.
                            // Let's stick to parsing what we can.
                        }
                    } else if c.kind() == "null" {
                        // "null" keyword
                        nullable = true;
                    } else if node_text(&c, source) == "null" {
                        nullable = true;
                    }
                }

                if let Some(idx) = index_val {
                    return if nullable {
                        ValueType::RefNull(idx)
                    } else {
                        ValueType::Ref(idx)
                    };
                }
                // Fallback for named types: map to generic StructRef or similar?
                // Or maybe we treat Ref(0) as "some typed ref".
                return ValueType::Structref; // Safe fallback?
            }
            "ref_type_ref" => {
                // (ref null? kind) or (ref null? $type)
                // Walk children to extract nullable and index
                let mut concrete_cursor = child.walk();
                let mut index_val = None;
                let mut nullable = false;
                let mut ref_kind = None;

                for c in child.children(&mut concrete_cursor) {
                    node_kind!(ck = c);
                    if ck == "index" {
                        let idx_text = node_text(&c, source);
                        if let Ok(idx) = idx_text.parse::<u32>() {
                            index_val = Some(idx);
                        }
                        // Named index: leave index_val as None, fallback to Structref
                    } else if ck == "null" || node_text(&c, source) == "null" {
                        nullable = true;
                    } else if ck == "ref_kind" {
                        ref_kind = Some(node_text(&c, source));
                    }
                }

                // If it's a ref_kind like (ref func), (ref extern), etc.
                if let Some(kind) = ref_kind {
                    return match kind {
                        "func" => ValueType::Funcref,
                        "extern" => ValueType::Externref,
                        "any" => ValueType::Anyref,
                        "eq" => ValueType::Eqref,
                        "i31" => ValueType::I31ref,
                        "struct" => ValueType::Structref,
                        "array" => ValueType::Arrayref,
                        "null" | "none" => ValueType::Nullref,
                        "nofunc" => ValueType::NullFuncref,
                        "noextern" => ValueType::NullExternref,
                        _ => ValueType::Funcref,
                    };
                }

                if let Some(idx) = index_val {
                    return if nullable {
                        ValueType::RefNull(idx)
                    } else {
                        ValueType::Ref(idx)
                    };
                }
                return ValueType::Structref; // Fallback for named types
            }
            // Handle intermediate ref_type wrapper node (e.g. value_type_ref_type → ref_type)
            "ref_type" => return extract_ref_type(&child, source),
            _ => {
                let text = node_text(&child, source);
                if let Some(vt) = simple_type_from_str(text) {
                    return vt;
                }
            }
        }
    }

    // Check for direct string matches in children (e.g. "anyref")
    for child in ref_type_node.children(&mut cursor) {
        let text = node_text(&child, source);
        if let Some(vt) = simple_type_from_str(text) {
            return vt;
        }
    }

    ValueType::Funcref // Default to funcref logic if all else fails
}

/// Extract data segments from the module
fn extract_data_segments(root: &Node, source: &str, symbol_table: &mut SymbolTable) {
    let mut cursor = root.walk();
    let mut data_index = 0;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_data" {
                            if let Some(data) =
                                extract_data_segment(&field_child, source, data_index)
                            {
                                symbol_table.add_data(data);
                                data_index += 1;
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_data" {
                    if let Some(data) = extract_data_segment(&field_child, source, data_index) {
                        symbol_table.add_data(data);
                        data_index += 1;
                    }
                }
            }
        }
    }
}

/// Extract a single data segment
fn extract_data_segment(data_node: &Node, source: &str, index: usize) -> Option<DataSegment> {
    // Data segment identifiers are wrapped in an "index" node
    let (name, name_range) = find_identifier_in_data_or_elem(data_node, source);

    // Extract string content - look for string literals
    let mut content = String::new();
    let mut byte_length = 0;
    let text = node_text(data_node, source);

    // Find quoted strings in the data segment
    let mut in_string = false;
    let mut escape_next = false;
    for c in text.chars() {
        if escape_next {
            content.push(c);
            byte_length += 1;
            escape_next = false;
        } else if c == '\\' && in_string {
            escape_next = true;
            content.push(c);
        } else if c == '"' {
            in_string = !in_string;
        } else if in_string {
            content.push(c);
            byte_length += 1;
        }
    }

    Some(DataSegment {
        name,
        index,
        content,
        byte_length,
        line: data_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Extract elem segments from the module
fn extract_elem_segments(root: &Node, source: &str, symbol_table: &mut SymbolTable) {
    let mut cursor = root.walk();
    let mut elem_index = 0;

    for child in root.children(&mut cursor) {
        if child.kind() == "module" {
            let mut module_cursor = child.walk();
            for module_child in child.children(&mut module_cursor) {
                if module_child.kind() == "module_field" {
                    let mut field_cursor = module_child.walk();
                    for field_child in module_child.children(&mut field_cursor) {
                        if field_child.kind() == "module_field_elem" {
                            if let Some(elem) =
                                extract_elem_segment(&field_child, source, elem_index)
                            {
                                symbol_table.add_elem(elem);
                                elem_index += 1;
                            }
                        }
                    }
                }
            }
        } else if child.kind() == "module_field" {
            let mut field_cursor = child.walk();
            for field_child in child.children(&mut field_cursor) {
                if field_child.kind() == "module_field_elem" {
                    if let Some(elem) = extract_elem_segment(&field_child, source, elem_index) {
                        symbol_table.add_elem(elem);
                        elem_index += 1;
                    }
                }
            }
        }
    }
}

/// Extract a single elem segment
fn extract_elem_segment(elem_node: &Node, source: &str, index: usize) -> Option<ElemSegment> {
    // Elem segment identifiers are wrapped in an "index" node
    let (name, name_range) = find_identifier_in_data_or_elem(elem_node, source);

    // Extract function names from elem segment
    let mut func_names = Vec::new();

    // Look for index nodes which could be function references
    let text = node_text(elem_node, source);

    // Find function references after "func" keyword
    if let Some(func_pos) = text.find(" func ") {
        let after_func = &text[func_pos + 6..];
        // Extract all $identifiers
        let mut remaining = after_func;
        while let Some(dollar_pos) = remaining.find('$') {
            let name_start = dollar_pos;
            let name_end = remaining[name_start + 1..]
                .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                .map(|i| name_start + i + 1)
                .unwrap_or(remaining.len());
            let func_name = &remaining[name_start..name_end];
            func_names.push(func_name.to_string());
            remaining = &remaining[name_end..];
        }
    }

    Some(ElemSegment {
        name,
        index,
        func_names,
        line: elem_node.range().start_point.row as u32,
        range: name_range,
    })
}

/// Find identifier in data or elem segment nodes.
/// These have identifiers wrapped in an "index" node, unlike other constructs.
fn find_identifier_in_data_or_elem(node: &Node, source: &str) -> (Option<String>, Option<Range>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Direct identifier (shouldn't happen, but handle it)
        if child.kind() == "identifier" {
            return (
                Some(node_text(&child, source).to_string()),
                Some(node_to_range(&child)),
            );
        }
        // Index node that contains the identifier
        if child.kind() == "index" {
            let mut index_cursor = child.walk();
            for index_child in child.children(&mut index_cursor) {
                if index_child.kind() == "identifier" {
                    return (
                        Some(node_text(&index_child, source).to_string()),
                        Some(node_to_range(&index_child)),
                    );
                }
            }
        }
    }
    (None, None)
}
