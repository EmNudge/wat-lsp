//! Shared find-references logic for both native and WASM builds.
//!
//! This module provides protocol-independent find-references, returning
//! `Vec<core::types::Range>` instead of LSP-specific `Vec<Location>`.
//! The native `references/mod.rs` is a thin wrapper that converts to LSP types.

// Allow needless_borrow/borrow_deref_ref: &kind and &*kind are required for WASM
// (String -> &str) but appear unnecessary on native (&str -> &str).
#![allow(clippy::needless_borrow, clippy::borrow_deref_ref)]

#[cfg(feature = "native")]
use tree_sitter::{Node, Tree};

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::{Node, Tree};

use crate::core::types::{Position, Range};
use crate::symbol_lookup::{
    find_block_in_function, find_local_in_function, find_param_in_function,
};
use crate::symbols::*;
use crate::utils::{
    determine_context_from_line, determine_instruction_context_at_node, find_child_by_kind,
    find_containing_function, get_line_at_position, get_word_at_position, is_labeled_block_kind,
    node_at_position, node_to_range, position_to_byte, InstructionContext,
};

/// Represents the type of symbol being referenced
#[derive(Debug, PartialEq, Clone)]
pub enum ReferenceTarget {
    Function {
        name: Option<String>,
        index: usize,
    },
    Global {
        name: Option<String>,
        index: usize,
    },
    Local {
        name: Option<String>,
        index: usize,
        function_start_byte: usize,
    },
    Parameter {
        name: Option<String>,
        index: usize,
        function_start_byte: usize,
    },
    BlockLabel {
        label: String,
        function_start_byte: usize,
        line: u32,
    },
    Table {
        name: Option<String>,
        index: usize,
    },
    Memory {
        name: Option<String>,
        index: usize,
    },
    Type {
        name: Option<String>,
        index: usize,
    },
    Tag {
        name: Option<String>,
        index: usize,
    },
    Data {
        name: Option<String>,
        index: usize,
    },
    Elem {
        name: Option<String>,
        index: usize,
    },
}

impl ReferenceTarget {
    /// Returns true if this symbol has a name (i.e. can be renamed).
    /// Block labels are always considered named.
    pub fn has_name(&self) -> bool {
        match self {
            ReferenceTarget::Function { name, .. }
            | ReferenceTarget::Global { name, .. }
            | ReferenceTarget::Local { name, .. }
            | ReferenceTarget::Parameter { name, .. }
            | ReferenceTarget::Table { name, .. }
            | ReferenceTarget::Memory { name, .. }
            | ReferenceTarget::Type { name, .. }
            | ReferenceTarget::Tag { name, .. }
            | ReferenceTarget::Data { name, .. }
            | ReferenceTarget::Elem { name, .. } => name.is_some(),
            ReferenceTarget::BlockLabel { .. } => true,
        }
    }
}

/// Block information for tracking nesting depth
#[derive(Debug, Clone)]
struct BlockInfo {
    label: Option<String>,
    line: u32,
}

/// Context for reference search operations
struct ReferenceSearchContext<'a> {
    document: &'a str,
    symbols: &'a SymbolTable,
    results: &'a mut Vec<Range>,
}

/// Main entry point: find all references to the symbol at the given position.
///
/// Returns protocol-independent `Vec<Range>`. The native wrapper converts
/// these to `Vec<Location>` by attaching a URI.
pub fn provide_references_core(
    document: &str,
    symbols: &SymbolTable,
    tree: &Tree,
    position: Position,
    include_declaration: bool,
) -> Vec<Range> {
    provide_references_core_scoped(document, symbols, tree, position, include_declaration, None)
}

/// Find all references, optionally scoped to a specific module range.
/// When `module_range` is provided, only references within that range are returned.
pub fn provide_references_core_scoped(
    document: &str,
    symbols: &SymbolTable,
    tree: &Tree,
    position: Position,
    include_declaration: bool,
    module_range: Option<Range>,
) -> Vec<Range> {
    let target = match identify_symbol_at_position(document, symbols, tree, position) {
        Some(t) => t,
        None => return vec![],
    };

    let mut references = find_all_references(&target, tree, document, symbols);

    // Filter to module scope if provided
    if let Some(scope) = module_range {
        references.retain(|r| {
            (r.start.line > scope.start.line
                || (r.start.line == scope.start.line && r.start.character >= scope.start.character))
                && (r.end.line < scope.end.line
                    || (r.end.line == scope.end.line && r.end.character <= scope.end.character))
        });
    }

    if include_declaration {
        if let Some(def_range) = get_definition_range(&target, symbols) {
            // Only include declaration if it's within scope
            let in_scope = module_range.is_none_or(|scope| {
                (def_range.start.line > scope.start.line
                    || (def_range.start.line == scope.start.line
                        && def_range.start.character >= scope.start.character))
                    && (def_range.end.line < scope.end.line
                        || (def_range.end.line == scope.end.line
                            && def_range.end.character <= scope.end.character))
            });
            if in_scope {
                references.insert(0, def_range);
            }
        }
    }

    // Sort by position
    references.sort_by(|a, b| {
        let line_cmp = a.start.line.cmp(&b.start.line);
        if line_cmp == std::cmp::Ordering::Equal {
            a.start.character.cmp(&b.start.character)
        } else {
            line_cmp
        }
    });

    // Deduplicate
    references.dedup_by(|a, b| *a == *b);

    references
}

/// Identify what symbol the cursor is positioned on
pub fn identify_symbol_at_position(
    document: &str,
    symbols: &SymbolTable,
    tree: &Tree,
    position: Position,
) -> Option<ReferenceTarget> {
    let word = get_word_at_position(document, position)?;

    // Determine context using AST, with fallback to line matching
    let context = if let Some(node) = node_at_position(tree, document, position) {
        let ast_context = determine_instruction_context_at_node(&node, document);
        if ast_context == InstructionContext::General {
            if let Some(line) = get_line_at_position(document, position.line as usize) {
                determine_context_from_line(line)
            } else {
                InstructionContext::General
            }
        } else {
            ast_context
        }
    } else if let Some(line) = get_line_at_position(document, position.line as usize) {
        determine_context_from_line(line)
    } else {
        InstructionContext::General
    };

    if word.starts_with('$') {
        identify_named_symbol(&word, symbols, context, position)
    } else if word.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(index) = word.parse::<usize>() {
            let mut result =
                identify_indexed_symbol(index, symbols, context, position, tree, document);

            // If we couldn't identify the symbol with AST context, try line-based fallback
            if result.is_none() && context == InstructionContext::Function {
                if let Some(line) = get_line_at_position(document, position.line as usize) {
                    let line_context = determine_context_from_line(line);
                    result = identify_indexed_symbol(
                        index,
                        symbols,
                        line_context,
                        position,
                        tree,
                        document,
                    );
                }
            }

            result
        } else {
            None
        }
    } else {
        None
    }
}

/// Identify a named symbol (e.g., $funcName, $varName)
fn identify_named_symbol(
    word: &str,
    symbols: &SymbolTable,
    context: InstructionContext,
    position: Position,
) -> Option<ReferenceTarget> {
    match context {
        InstructionContext::Call => {
            if let Some(func) = symbols.get_function_by_name(word) {
                return Some(ReferenceTarget::Function {
                    name: Some(word.to_string()),
                    index: func.index,
                });
            }
        }
        InstructionContext::Global => {
            if let Some(global) = symbols.get_global_by_name(word) {
                return Some(ReferenceTarget::Global {
                    name: Some(word.to_string()),
                    index: global.index,
                });
            }
        }
        InstructionContext::Local => {
            if let Some(func) = find_containing_function(symbols, position) {
                if let Some(param) = find_param_in_function(word, func) {
                    return Some(ReferenceTarget::Parameter {
                        name: Some(word.to_string()),
                        index: param.index,
                        function_start_byte: func.start_byte,
                    });
                }
                if let Some(local) = find_local_in_function(word, func) {
                    return Some(ReferenceTarget::Local {
                        name: Some(word.to_string()),
                        index: local.index + func.parameters.len(),
                        function_start_byte: func.start_byte,
                    });
                }
            }
        }
        InstructionContext::Branch | InstructionContext::Block => {
            if let Some(func) = find_containing_function(symbols, position) {
                if let Some(block) = find_block_in_function(word, func) {
                    return Some(ReferenceTarget::BlockLabel {
                        label: word.to_string(),
                        function_start_byte: func.start_byte,
                        line: block.line,
                    });
                }
            }
        }
        InstructionContext::Function | InstructionContext::General => {
            if let Some(func) = symbols.get_function_by_name(word) {
                return Some(ReferenceTarget::Function {
                    name: Some(word.to_string()),
                    index: func.index,
                });
            }

            if let Some(func) = find_containing_function(symbols, position) {
                if let Some(param) = find_param_in_function(word, func) {
                    return Some(ReferenceTarget::Parameter {
                        name: Some(word.to_string()),
                        index: param.index,
                        function_start_byte: func.start_byte,
                    });
                }
                if let Some(local) = find_local_in_function(word, func) {
                    return Some(ReferenceTarget::Local {
                        name: Some(word.to_string()),
                        index: local.index + func.parameters.len(),
                        function_start_byte: func.start_byte,
                    });
                }
                if let Some(block) = find_block_in_function(word, func) {
                    return Some(ReferenceTarget::BlockLabel {
                        label: word.to_string(),
                        function_start_byte: func.start_byte,
                        line: block.line,
                    });
                }
            }

            if let Some(global) = symbols.get_global_by_name(word) {
                return Some(ReferenceTarget::Global {
                    name: Some(word.to_string()),
                    index: global.index,
                });
            }
            if let Some(table) = symbols.get_table_by_name(word) {
                return Some(ReferenceTarget::Table {
                    name: Some(word.to_string()),
                    index: table.index,
                });
            }
            if let Some(memory) = symbols.get_memory_by_name(word) {
                return Some(ReferenceTarget::Memory {
                    name: Some(word.to_string()),
                    index: memory.index,
                });
            }
            if let Some(type_def) = symbols.get_type_by_name(word) {
                return Some(ReferenceTarget::Type {
                    name: Some(word.to_string()),
                    index: type_def.index,
                });
            }
            if let Some(tag) = symbols.get_tag_by_name(word) {
                return Some(ReferenceTarget::Tag {
                    name: Some(word.to_string()),
                    index: tag.index,
                });
            }
        }
        InstructionContext::Table => {
            if let Some(table) = symbols.get_table_by_name(word) {
                return Some(ReferenceTarget::Table {
                    name: Some(word.to_string()),
                    index: table.index,
                });
            }
        }
        InstructionContext::Memory => {
            if let Some(memory) = symbols.get_memory_by_name(word) {
                return Some(ReferenceTarget::Memory {
                    name: Some(word.to_string()),
                    index: memory.index,
                });
            }
        }
        InstructionContext::Type => {
            if let Some(type_def) = symbols.get_type_by_name(word) {
                return Some(ReferenceTarget::Type {
                    name: Some(word.to_string()),
                    index: type_def.index,
                });
            }
        }
        InstructionContext::Tag => {
            if let Some(tag) = symbols.get_tag_by_name(word) {
                return Some(ReferenceTarget::Tag {
                    name: Some(word.to_string()),
                    index: tag.index,
                });
            }
        }
        InstructionContext::Data => {
            if let Some(data) = symbols.get_data_by_name(word) {
                return Some(ReferenceTarget::Data {
                    name: Some(word.to_string()),
                    index: data.index,
                });
            }
        }
        InstructionContext::Elem => {
            if let Some(elem) = symbols.get_elem_by_name(word) {
                return Some(ReferenceTarget::Elem {
                    name: Some(word.to_string()),
                    index: elem.index,
                });
            }
        }
    }

    None
}

/// Identify a numeric index symbol (e.g., call 0, local.get 1)
fn identify_indexed_symbol(
    index: usize,
    symbols: &SymbolTable,
    context: InstructionContext,
    position: Position,
    tree: &Tree,
    document: &str,
) -> Option<ReferenceTarget> {
    match context {
        InstructionContext::Call => {
            if let Some(func) = symbols.get_function_by_index(index) {
                return Some(ReferenceTarget::Function {
                    name: func.name.clone(),
                    index,
                });
            }
        }
        InstructionContext::Global => {
            if let Some(global) = symbols.get_global_by_index(index) {
                return Some(ReferenceTarget::Global {
                    name: global.name.clone(),
                    index,
                });
            }
        }
        InstructionContext::Local => {
            if let Some(func) = find_containing_function(symbols, position) {
                let total_params = func.parameters.len();

                if index < total_params {
                    if let Some(param) = func.parameters.get(index) {
                        return Some(ReferenceTarget::Parameter {
                            name: param.name.clone(),
                            index,
                            function_start_byte: func.start_byte,
                        });
                    }
                } else {
                    let local_index = index - total_params;
                    if let Some(local) = func.locals.get(local_index) {
                        return Some(ReferenceTarget::Local {
                            name: local.name.clone(),
                            index,
                            function_start_byte: func.start_byte,
                        });
                    }
                }
            }
        }
        InstructionContext::Type => {
            if let Some(type_def) = symbols.get_type_by_index(index) {
                return Some(ReferenceTarget::Type {
                    name: type_def.name.clone(),
                    index,
                });
            }
        }
        InstructionContext::Tag => {
            if let Some(tag) = symbols.get_tag_by_index(index) {
                return Some(ReferenceTarget::Tag {
                    name: tag.name.clone(),
                    index,
                });
            }
        }
        InstructionContext::Branch => {
            if let Some(func) = find_containing_function(symbols, position) {
                let block_stack = build_block_stack_at_position(tree, document, position);

                if let Some(block) = resolve_block_by_depth(index, &block_stack) {
                    return Some(ReferenceTarget::BlockLabel {
                        label: block
                            .label
                            .clone()
                            .unwrap_or_else(|| format!("@{}", block.line)),
                        function_start_byte: func.start_byte,
                        line: block.line,
                    });
                }
            }
        }
        _ => {}
    }

    None
}

/// Find all references to the target symbol
fn find_all_references(
    target: &ReferenceTarget,
    tree: &Tree,
    document: &str,
    symbols: &SymbolTable,
) -> Vec<Range> {
    let mut results = Vec::new();
    let mut block_stack = Vec::new();

    walk_tree_for_references(
        tree.root_node(),
        target,
        document,
        symbols,
        &mut results,
        &mut block_stack,
    );

    results
}

/// Determine reference context for export descriptors
fn determine_export_context(node: &Node) -> Option<InstructionContext> {
    let kind = node.kind();
    // Use &*kind to get &str from both native (&str) and WASM (String)
    match &*kind {
        "export_desc_func" => Some(InstructionContext::Call),
        "export_desc_global" => Some(InstructionContext::Global),
        "export_desc_table" => Some(InstructionContext::Table),
        "export_desc_memory" => Some(InstructionContext::Memory),
        _ => None,
    }
}

/// Recursively walk the tree to find all references
fn walk_tree_for_references(
    node: Node,
    target: &ReferenceTarget,
    document: &str,
    symbols: &SymbolTable,
    results: &mut Vec<Range>,
    block_stack: &mut Vec<BlockInfo>,
) {
    let kind = node.kind();

    // Track block entry/exit for depth calculation
    let is_block = is_labeled_block_kind(&kind);

    if is_block {
        let label = extract_block_label(&node, document);
        block_stack.push(BlockInfo {
            label,
            line: node.start_position().row as u32,
        });
    }

    // Check for export descriptors
    if let Some(export_context) = determine_export_context(&node) {
        let mut ctx = ReferenceSearchContext {
            document,
            symbols,
            results,
        };
        check_node_for_reference(&node, target, &mut ctx, &export_context, block_stack);
    }

    // Check for elem_list with elem_kind: all index children are function references
    if &*kind == "elem_list" && find_child_by_kind(&node, "elem_kind").is_some() {
        let call_context = InstructionContext::Call;
        let mut inner_cursor = node.walk();
        for child in node.children(&mut inner_cursor) {
            let child_kind = child.kind();
            if &*child_kind == "index" {
                let mut ctx = ReferenceSearchContext {
                    document,
                    symbols,
                    results,
                };
                check_node_for_reference(&child, target, &mut ctx, &call_context, block_stack);
            }
        }
        if is_block {
            block_stack.pop();
        }
        return;
    }

    // Check for table_use: index child is a table reference
    if &*kind == "table_use" {
        let table_context = InstructionContext::Table;
        let mut inner_cursor = node.walk();
        for child in node.children(&mut inner_cursor) {
            let child_kind = child.kind();
            if &*child_kind == "index" {
                let mut ctx = ReferenceSearchContext {
                    document,
                    symbols,
                    results,
                };
                check_node_for_reference(&child, target, &mut ctx, &table_context, block_stack);
            }
        }
        if is_block {
            block_stack.pop();
        }
        return;
    }

    // Check for memory_use: index child is a memory reference
    if &*kind == "memory_use" {
        let memory_context = InstructionContext::Memory;
        let mut inner_cursor = node.walk();
        for child in node.children(&mut inner_cursor) {
            let child_kind = child.kind();
            if &*child_kind == "index" {
                let mut ctx = ReferenceSearchContext {
                    document,
                    symbols,
                    results,
                };
                check_node_for_reference(&child, target, &mut ctx, &memory_context, block_stack);
            }
        }
        if is_block {
            block_stack.pop();
        }
        return;
    }

    // Check for module_field_elem: bare index children after offset are function references
    if &*kind == "module_field_elem" {
        let mut inner_cursor = node.walk();
        let mut past_offset = false;
        for child in node.children(&mut inner_cursor) {
            let child_kind = child.kind();
            if &*child_kind == "offset" {
                past_offset = true;
            } else if &*child_kind == "index" && past_offset {
                let call_context = InstructionContext::Call;
                let mut ctx = ReferenceSearchContext {
                    document,
                    symbols,
                    results,
                };
                check_node_for_reference(&child, target, &mut ctx, &call_context, block_stack);
            }
        }
        // Fall through to normal recursion for table_use, elem_list, offset children
    }

    // Check if this node is a reference instruction
    let context = determine_instruction_context_at_node(&node, document);

    if matches!(
        context,
        InstructionContext::Call
            | InstructionContext::Global
            | InstructionContext::Local
            | InstructionContext::Branch
            | InstructionContext::Table
            | InstructionContext::Type
            | InstructionContext::Tag
            | InstructionContext::Memory
    ) {
        let mut ctx = ReferenceSearchContext {
            document,
            symbols,
            results,
        };
        check_node_for_reference(&node, target, &mut ctx, &context, block_stack);

        // For most contexts, we've already processed this subtree, so don't recurse
        // Exception: Branch context may contain nested instructions like local.get in br_if
        if context != InstructionContext::Branch {
            if is_block {
                block_stack.pop();
            }
            return;
        }
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree_for_references(child, target, document, symbols, results, block_stack);
    }

    if is_block {
        block_stack.pop();
    }
}

/// Check if a node contains a reference to the target
fn check_node_for_reference(
    node: &Node,
    target: &ReferenceTarget,
    ctx: &mut ReferenceSearchContext,
    context: &InstructionContext,
    block_stack: &[BlockInfo],
) {
    find_reference_identifiers(node, target, ctx, context, block_stack, true);
}

/// Check if a node represents a nested expression that has its own context
fn is_nested_expression(kind: &str) -> bool {
    matches!(kind, "expr" | "instr")
}

/// Find identifier nodes and check if they match the target
fn find_reference_identifiers(
    node: &Node,
    target: &ReferenceTarget,
    ctx: &mut ReferenceSearchContext,
    context: &InstructionContext,
    block_stack: &[BlockInfo],
    is_root: bool,
) {
    let kind = node.kind();

    // Skip nested expressions - they have their own context
    if !is_root && is_nested_expression(&kind) {
        return;
    }

    // Check if this is an identifier node
    if kind == "identifier" {
        let text = &ctx.document[node.byte_range()];
        if matches_target_identifier(
            text,
            target,
            context,
            node.start_position().row as u32,
            ctx.symbols,
            block_stack,
        ) {
            ctx.results.push(node_to_range(node));
        }
    }

    // Check if this is a numeric index
    if matches!(&*kind, "nat" | "dec_nat" | "hex_nat" | "index") {
        let text = &ctx.document[node.byte_range()];
        if !text.trim().starts_with('$') {
            if let Some(index) = crate::parser::parse_wat_nat(text).map(|v| v as usize) {
                if matches_target_index(
                    index,
                    target,
                    context,
                    node.start_position().row as u32,
                    ctx.symbols,
                    block_stack,
                ) {
                    ctx.results.push(node_to_range(node));
                }
            }
        }
    }

    // Recurse to children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_reference_identifiers(&child, target, ctx, context, block_stack, false);
    }
}

/// Check if an identifier matches the target
fn matches_target_identifier(
    identifier: &str,
    target: &ReferenceTarget,
    context: &InstructionContext,
    line: u32,
    symbols: &SymbolTable,
    _block_stack: &[BlockInfo],
) -> bool {
    match target {
        ReferenceTarget::Function { name, .. } => {
            if *context != InstructionContext::Call {
                return false;
            }
            name.as_deref() == Some(identifier)
        }
        ReferenceTarget::Global { name, .. } => {
            if *context != InstructionContext::Global {
                return false;
            }
            name.as_deref() == Some(identifier)
        }
        ReferenceTarget::Local {
            name,
            function_start_byte,
            ..
        } => {
            if *context != InstructionContext::Local {
                return false;
            }
            if name.as_deref() != Some(identifier) {
                return false;
            }
            is_in_same_function_by_line(line, *function_start_byte, symbols)
        }
        ReferenceTarget::Parameter {
            name,
            function_start_byte,
            ..
        } => {
            if *context != InstructionContext::Local {
                return false;
            }
            if name.as_deref() != Some(identifier) {
                return false;
            }
            is_in_same_function_by_line(line, *function_start_byte, symbols)
        }
        ReferenceTarget::BlockLabel {
            label,
            function_start_byte,
            ..
        } => {
            if *context != InstructionContext::Branch {
                return false;
            }
            if label != identifier {
                return false;
            }
            is_in_same_function_by_line(line, *function_start_byte, symbols)
        }
        ReferenceTarget::Table { name, .. } => {
            if *context != InstructionContext::Table {
                return false;
            }
            name.as_deref() == Some(identifier)
        }
        ReferenceTarget::Memory { name, .. } => {
            if *context != InstructionContext::Memory {
                return false;
            }
            name.as_deref() == Some(identifier)
        }
        ReferenceTarget::Type { name, .. } => {
            if *context != InstructionContext::Type {
                return false;
            }
            name.as_deref() == Some(identifier)
        }
        ReferenceTarget::Tag { name, .. } => {
            if *context != InstructionContext::Tag {
                return false;
            }
            name.as_deref() == Some(identifier)
        }
        ReferenceTarget::Data { name, .. } => {
            if *context != InstructionContext::Data {
                return false;
            }
            name.as_deref() == Some(identifier)
        }
        ReferenceTarget::Elem { name, .. } => {
            if *context != InstructionContext::Elem {
                return false;
            }
            name.as_deref() == Some(identifier)
        }
    }
}

/// Check if a numeric index matches the target
fn matches_target_index(
    index: usize,
    target: &ReferenceTarget,
    context: &InstructionContext,
    line: u32,
    symbols: &SymbolTable,
    block_stack: &[BlockInfo],
) -> bool {
    match target {
        ReferenceTarget::Function {
            index: target_index,
            ..
        } => {
            if *context != InstructionContext::Call {
                return false;
            }
            index == *target_index
        }
        ReferenceTarget::Global {
            index: target_index,
            ..
        } => {
            if *context != InstructionContext::Global {
                return false;
            }
            index == *target_index
        }
        ReferenceTarget::Local {
            index: target_index,
            function_start_byte,
            ..
        } => {
            if *context != InstructionContext::Local {
                return false;
            }
            if index != *target_index {
                return false;
            }
            is_in_same_function_by_line(line, *function_start_byte, symbols)
        }
        ReferenceTarget::Parameter {
            index: target_index,
            function_start_byte,
            ..
        } => {
            if *context != InstructionContext::Local {
                return false;
            }
            if index != *target_index {
                return false;
            }
            is_in_same_function_by_line(line, *function_start_byte, symbols)
        }
        ReferenceTarget::BlockLabel {
            label,
            function_start_byte,
            line: target_line,
            ..
        } => {
            if *context != InstructionContext::Branch {
                return false;
            }
            if let Some(block) = resolve_block_by_depth(index, block_stack) {
                if let Some(ref block_label) = block.label {
                    if block_label == label {
                        return is_in_same_function_by_line(line, *function_start_byte, symbols);
                    }
                } else if block.line == *target_line {
                    return is_in_same_function_by_line(line, *function_start_byte, symbols);
                }
            }
            false
        }
        ReferenceTarget::Table {
            index: target_index,
            ..
        } => {
            if *context != InstructionContext::Table {
                return false;
            }
            index == *target_index
        }
        ReferenceTarget::Memory {
            index: target_index,
            ..
        } => {
            if *context != InstructionContext::Memory {
                return false;
            }
            index == *target_index
        }
        ReferenceTarget::Type {
            index: target_index,
            ..
        } => {
            if *context != InstructionContext::Type {
                return false;
            }
            index == *target_index
        }
        ReferenceTarget::Tag {
            index: target_index,
            ..
        } => {
            if *context != InstructionContext::Tag {
                return false;
            }
            index == *target_index
        }
        ReferenceTarget::Data {
            index: target_index,
            ..
        } => {
            if *context != InstructionContext::Data {
                return false;
            }
            index == *target_index
        }
        ReferenceTarget::Elem {
            index: target_index,
            ..
        } => {
            if *context != InstructionContext::Elem {
                return false;
            }
            index == *target_index
        }
    }
}

/// Resolve a block by depth (0 = innermost, 1 = next outer, etc.)
fn resolve_block_by_depth(depth: usize, block_stack: &[BlockInfo]) -> Option<&BlockInfo> {
    let stack_len = block_stack.len();
    if depth >= stack_len {
        return None;
    }
    block_stack.get(stack_len - 1 - depth)
}

/// Build the block stack at a given position
fn build_block_stack_at_position(
    tree: &Tree,
    document: &str,
    position: Position,
) -> Vec<BlockInfo> {
    let mut block_stack = Vec::new();
    let target_byte = position_to_byte(document, position);

    build_block_stack_recursive(tree.root_node(), document, target_byte, &mut block_stack);

    block_stack
}

/// Recursively build the block stack by finding all blocks containing the target byte
fn build_block_stack_recursive(
    node: Node,
    document: &str,
    target_byte: usize,
    block_stack: &mut Vec<BlockInfo>,
) {
    if node.start_byte() > target_byte || node.end_byte() < target_byte {
        return;
    }

    let kind = node.kind();
    let is_block = is_labeled_block_kind(&kind);

    if is_block {
        let label = extract_block_label(&node, document);
        block_stack.push(BlockInfo {
            label,
            line: node.start_position().row as u32,
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        build_block_stack_recursive(child, document, target_byte, block_stack);
    }
}

/// Extract block label from a block node
fn extract_block_label(node: &Node, document: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let text = &document[child.byte_range()];
            return Some(text.to_string());
        }
    }
    None
}

/// Get the definition range for a target
pub fn get_definition_range(target: &ReferenceTarget, symbols: &SymbolTable) -> Option<Range> {
    let range = match target {
        ReferenceTarget::Function { index, .. } => {
            symbols.get_function_by_index(*index)?.range.as_ref()?
        }
        ReferenceTarget::Global { index, .. } => {
            symbols.get_global_by_index(*index)?.range.as_ref()?
        }
        ReferenceTarget::Local {
            index,
            function_start_byte,
            ..
        } => {
            let func = symbols
                .functions
                .iter()
                .find(|f| f.start_byte == *function_start_byte)?;

            if *index < func.parameters.len() {
                func.parameters.get(*index)?.range.as_ref()?
            } else {
                func.locals
                    .get(*index - func.parameters.len())?
                    .range
                    .as_ref()?
            }
        }
        ReferenceTarget::Parameter {
            index,
            function_start_byte,
            ..
        } => {
            let func = symbols
                .functions
                .iter()
                .find(|f| f.start_byte == *function_start_byte)?;
            func.parameters.get(*index)?.range.as_ref()?
        }
        ReferenceTarget::BlockLabel {
            label,
            function_start_byte,
            ..
        } => {
            let func = symbols
                .functions
                .iter()
                .find(|f| f.start_byte == *function_start_byte)?;
            func.blocks
                .iter()
                .find(|b| b.label == *label)?
                .range
                .as_ref()?
        }
        ReferenceTarget::Table { index, .. } => {
            symbols.get_table_by_index(*index)?.range.as_ref()?
        }
        ReferenceTarget::Memory { index, .. } => {
            symbols.get_memory_by_index(*index)?.range.as_ref()?
        }
        ReferenceTarget::Type { index, .. } => symbols.get_type_by_index(*index)?.range.as_ref()?,
        ReferenceTarget::Tag { index, .. } => symbols.get_tag_by_index(*index)?.range.as_ref()?,
        ReferenceTarget::Data { index, .. } => symbols.get_data_by_index(*index)?.range.as_ref()?,
        ReferenceTarget::Elem { index, .. } => symbols.get_elem_by_index(*index)?.range.as_ref()?,
    };

    Some(*range)
}

/// Check if a line is within the same function as the target
fn is_in_same_function_by_line(
    line: u32,
    target_function_start_byte: usize,
    symbols: &SymbolTable,
) -> bool {
    symbols
        .find_function_containing_line(line)
        .is_some_and(|func| func.start_byte == target_function_start_byte)
}
