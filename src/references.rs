use crate::symbols::*;
use crate::utils::{
    find_containing_function, get_line_at_position, get_word_at_position, node_at_position,
};
use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Tree};

#[cfg(test)]
mod tests;

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
}

/// Context for determining what type of reference we're looking at
#[derive(Debug, PartialEq, Copy, Clone)]
enum ReferenceContext {
    Call,     // Inside call instruction
    Global,   // Inside global.get/set
    Local,    // Inside local.get/set/tee
    Branch,   // Inside br/br_if/br_table
    Table,    // Inside table operation
    Memory,   // Inside memory operation
    Type,     // Inside type definition/use
    Function, // Inside function definition
    General,  // General context
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
    uri: &'a str,
    results: &'a mut Vec<Location>,
}

/// Main entry point for providing find-references
pub fn provide_references(
    document: &str,
    symbols: &SymbolTable,
    tree: &Tree,
    position: Position,
    uri: &str,
    include_declaration: bool,
) -> Vec<Location> {
    // Identify what symbol the cursor is on
    let target = match identify_symbol_at_position(document, symbols, tree, position) {
        Some(t) => t,
        None => return vec![],
    };

    // Find all references to this symbol
    let mut references = find_all_references(&target, tree, document, symbols, uri);

    // Optionally include the declaration
    if include_declaration {
        if let Some(def_location) = get_definition_location(&target, symbols, uri) {
            // Prepend definition to results
            references.insert(0, def_location);
        }
    }

    // Sort by position
    references.sort_by(|a, b| {
        let line_cmp = a.range.start.line.cmp(&b.range.start.line);
        if line_cmp == std::cmp::Ordering::Equal {
            a.range.start.character.cmp(&b.range.start.character)
        } else {
            line_cmp
        }
    });

    // Deduplicate
    references.dedup_by(|a, b| a.range == b.range);

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
        let ast_context = determine_reference_context(&node, document);
        if ast_context == ReferenceContext::General {
            // Fallback to line-based detection for incomplete code
            if let Some(line) = get_line_at_position(document, position.line as usize) {
                determine_context_from_line(line)
            } else {
                ReferenceContext::General
            }
        } else {
            ast_context
        }
    } else {
        // Fallback to line-based detection
        if let Some(line) = get_line_at_position(document, position.line as usize) {
            determine_context_from_line(line)
        } else {
            ReferenceContext::General
        }
    };

    // Check if it's a named symbol or numeric index
    if word.starts_with('$') {
        identify_named_symbol(&word, symbols, context, position)
    } else if word.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(index) = word.parse::<usize>() {
            let mut result =
                identify_indexed_symbol(index, symbols, context, position, tree, document);

            // If we couldn't identify the symbol with AST context, try line-based fallback
            if result.is_none() && context == ReferenceContext::Function {
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
    context: ReferenceContext,
    position: Position,
) -> Option<ReferenceTarget> {
    match context {
        ReferenceContext::Call => {
            // Only function calls
            if let Some(func) = symbols.get_function_by_name(word) {
                return Some(ReferenceTarget::Function {
                    name: Some(word.to_string()),
                    index: func.index,
                });
            }
        }
        ReferenceContext::Global => {
            if let Some(global) = symbols.get_global_by_name(word) {
                return Some(ReferenceTarget::Global {
                    name: Some(word.to_string()),
                    index: global.index,
                });
            }
        }
        ReferenceContext::Local => {
            if let Some(func) = find_containing_function(symbols, position) {
                // Check parameters first
                for param in &func.parameters {
                    if param.name.as_ref() == Some(&word.to_string()) {
                        return Some(ReferenceTarget::Parameter {
                            name: Some(word.to_string()),
                            index: param.index,
                            function_start_byte: func.start_byte,
                        });
                    }
                }
                // Then check locals
                for local in &func.locals {
                    if local.name.as_ref() == Some(&word.to_string()) {
                        return Some(ReferenceTarget::Local {
                            name: Some(word.to_string()),
                            index: local.index + func.parameters.len(),
                            function_start_byte: func.start_byte,
                        });
                    }
                }
            }
        }
        ReferenceContext::Branch => {
            if let Some(func) = find_containing_function(symbols, position) {
                for block in &func.blocks {
                    if block.label == word {
                        return Some(ReferenceTarget::BlockLabel {
                            label: word.to_string(),
                            function_start_byte: func.start_byte,
                            line: block.line,
                        });
                    }
                }
            }
        }
        ReferenceContext::Function | ReferenceContext::General => {
            // Try all symbol types, including block labels, locals, and parameters

            // Try function
            if let Some(func) = symbols.get_function_by_name(word) {
                return Some(ReferenceTarget::Function {
                    name: Some(word.to_string()),
                    index: func.index,
                });
            }

            // Try locals/parameters (user might be on the definition)
            if let Some(func) = find_containing_function(symbols, position) {
                // Check parameters
                for param in &func.parameters {
                    if param.name.as_ref() == Some(&word.to_string()) {
                        return Some(ReferenceTarget::Parameter {
                            name: Some(word.to_string()),
                            index: param.index,
                            function_start_byte: func.start_byte,
                        });
                    }
                }
                // Check locals
                for local in &func.locals {
                    if local.name.as_ref() == Some(&word.to_string()) {
                        return Some(ReferenceTarget::Local {
                            name: Some(word.to_string()),
                            index: local.index + func.parameters.len(),
                            function_start_byte: func.start_byte,
                        });
                    }
                }

                // Try block labels (user might be on the block definition)
                for block in &func.blocks {
                    if block.label == word {
                        return Some(ReferenceTarget::BlockLabel {
                            label: word.to_string(),
                            function_start_byte: func.start_byte,
                            line: block.line,
                        });
                    }
                }
            }

            // Try global
            if let Some(global) = symbols.get_global_by_name(word) {
                return Some(ReferenceTarget::Global {
                    name: Some(word.to_string()),
                    index: global.index,
                });
            }
            // Try table
            if let Some(table) = symbols.get_table_by_name(word) {
                return Some(ReferenceTarget::Table {
                    name: Some(word.to_string()),
                    index: table.index,
                });
            }
            // Try memory
            if let Some(memory) = symbols.get_memory_by_name(word) {
                return Some(ReferenceTarget::Memory {
                    name: Some(word.to_string()),
                    index: memory.index,
                });
            }
            // Try type
            if let Some(type_def) = symbols.get_type_by_name(word) {
                return Some(ReferenceTarget::Type {
                    name: Some(word.to_string()),
                    index: type_def.index,
                });
            }
        }
        ReferenceContext::Table => {
            if let Some(table) = symbols.get_table_by_name(word) {
                return Some(ReferenceTarget::Table {
                    name: Some(word.to_string()),
                    index: table.index,
                });
            }
        }
        ReferenceContext::Memory => {
            if let Some(memory) = symbols.get_memory_by_name(word) {
                return Some(ReferenceTarget::Memory {
                    name: Some(word.to_string()),
                    index: memory.index,
                });
            }
        }
        ReferenceContext::Type => {
            if let Some(type_def) = symbols.get_type_by_name(word) {
                return Some(ReferenceTarget::Type {
                    name: Some(word.to_string()),
                    index: type_def.index,
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
    context: ReferenceContext,
    position: Position,
    tree: &Tree,
    document: &str,
) -> Option<ReferenceTarget> {
    match context {
        ReferenceContext::Call => {
            if let Some(func) = symbols.get_function_by_index(index) {
                return Some(ReferenceTarget::Function {
                    name: func.name.clone(),
                    index,
                });
            }
        }
        ReferenceContext::Global => {
            if let Some(global) = symbols.get_global_by_index(index) {
                return Some(ReferenceTarget::Global {
                    name: global.name.clone(),
                    index,
                });
            }
        }
        ReferenceContext::Local => {
            if let Some(func) = find_containing_function(symbols, position) {
                let total_params = func.parameters.len();

                if index < total_params {
                    // It's a parameter
                    if let Some(param) = func.parameters.get(index) {
                        return Some(ReferenceTarget::Parameter {
                            name: param.name.clone(),
                            index,
                            function_start_byte: func.start_byte,
                        });
                    }
                } else {
                    // It's a local
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
        ReferenceContext::Branch => {
            // For branch instructions with numeric depth, resolve the block at that depth
            if let Some(func) = find_containing_function(symbols, position) {
                // Build block stack at the current position
                let block_stack = build_block_stack_at_position(tree, document, position);

                // Resolve the block at the given depth
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
    uri: &str,
) -> Vec<Location> {
    let mut results = Vec::new();
    let mut block_stack = Vec::new();

    walk_tree_for_references(
        tree.root_node(),
        target,
        document,
        symbols,
        uri,
        &mut results,
        &mut block_stack,
    );

    results
}

/// Recursively walk the tree to find all references
fn walk_tree_for_references(
    node: Node,
    target: &ReferenceTarget,
    document: &str,
    symbols: &SymbolTable,
    uri: &str,
    results: &mut Vec<Location>,
    block_stack: &mut Vec<BlockInfo>,
) {
    let kind = node.kind();

    // Track block entry/exit for depth calculation
    let is_block = matches!(
        kind,
        "block_block" | "block_loop" | "block_if" | "expr1_block" | "expr1_loop" | "expr1_if"
    );

    if is_block {
        // Extract block label if present
        let label = extract_block_label(&node, document);
        block_stack.push(BlockInfo {
            label,
            line: node.start_position().row as u32,
        });
    }

    // Check if this node is a reference instruction
    let context = determine_reference_context(&node, document);

    // Only process actual reference contexts, not definition contexts like Function
    if matches!(
        context,
        ReferenceContext::Call
            | ReferenceContext::Global
            | ReferenceContext::Local
            | ReferenceContext::Branch
            | ReferenceContext::Table
            | ReferenceContext::Type
    ) {
        let mut ctx = ReferenceSearchContext {
            document,
            symbols,
            uri,
            results,
        };
        check_node_for_reference(&node, target, &mut ctx, &context, block_stack);

        // For most contexts, we've already processed this subtree, so don't recurse
        // Exception: Branch context may contain nested instructions like local.get in br_if
        if context != ReferenceContext::Branch {
            if is_block {
                block_stack.pop();
            }
            return;
        }
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree_for_references(child, target, document, symbols, uri, results, block_stack);
    }

    // Pop block from stack when exiting
    if is_block {
        block_stack.pop();
    }
}

/// Check if a node contains a reference to the target
fn check_node_for_reference(
    node: &Node,
    target: &ReferenceTarget,
    ctx: &mut ReferenceSearchContext,
    context: &ReferenceContext,
    block_stack: &[BlockInfo],
) {
    // Find identifier or index nodes within this instruction
    find_reference_identifiers(node, target, ctx, context, block_stack);
}

/// Find identifier nodes and check if they match the target
fn find_reference_identifiers(
    node: &Node,
    target: &ReferenceTarget,
    ctx: &mut ReferenceSearchContext,
    context: &ReferenceContext,
    block_stack: &[BlockInfo],
) {
    let kind = node.kind();

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
            if let Ok(lsp_uri) = Url::parse(ctx.uri) {
                let range = node_to_range(node);
                ctx.results.push(Location {
                    uri: lsp_uri,
                    range,
                });
            }
        }
    }

    // Check if this is a numeric index (nat, dec_nat, hex_nat, or index node)
    if kind == "nat" || kind == "dec_nat" || kind == "hex_nat" || kind == "index" {
        let text = &ctx.document[node.byte_range()];
        // Try to parse as a number (skip if it starts with $ which indicates identifier)
        if !text.trim().starts_with('$') {
            if let Ok(index) = parse_nat(text.trim()) {
                if matches_target_index(
                    index,
                    target,
                    context,
                    node.start_position().row as u32,
                    ctx.symbols,
                    block_stack,
                ) {
                    if let Ok(lsp_uri) = Url::parse(ctx.uri) {
                        let range = node_to_range(node);
                        ctx.results.push(Location {
                            uri: lsp_uri,
                            range,
                        });
                    }
                }
            }
        }
    }

    // Recurse to children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_reference_identifiers(&child, target, ctx, context, block_stack);
    }
}

/// Check if an identifier matches the target
fn matches_target_identifier(
    identifier: &str,
    target: &ReferenceTarget,
    context: &ReferenceContext,
    line: u32,
    symbols: &SymbolTable,
    _block_stack: &[BlockInfo],
) -> bool {
    match target {
        ReferenceTarget::Function { name, .. } => {
            // Only match in Call context, not Function context (which is the definition)
            if *context != ReferenceContext::Call {
                return false;
            }
            name.as_ref() == Some(&identifier.to_string())
        }
        ReferenceTarget::Global { name, .. } => {
            if *context != ReferenceContext::Global {
                return false;
            }
            name.as_ref() == Some(&identifier.to_string())
        }
        ReferenceTarget::Local {
            name,
            function_start_byte,
            ..
        } => {
            if *context != ReferenceContext::Local {
                return false;
            }
            if name.as_ref() != Some(&identifier.to_string()) {
                return false;
            }
            // Check scope: must be in the same function
            is_in_same_function_by_line(line, *function_start_byte, symbols)
        }
        ReferenceTarget::Parameter {
            name,
            function_start_byte,
            ..
        } => {
            if *context != ReferenceContext::Local {
                return false;
            }
            if name.as_ref() != Some(&identifier.to_string()) {
                return false;
            }
            // Check scope: must be in the same function
            is_in_same_function_by_line(line, *function_start_byte, symbols)
        }
        ReferenceTarget::BlockLabel {
            label,
            function_start_byte,
            ..
        } => {
            if *context != ReferenceContext::Branch {
                return false;
            }
            if label != identifier {
                return false;
            }
            // Check scope: must be in the same function
            is_in_same_function_by_line(line, *function_start_byte, symbols)
        }
        ReferenceTarget::Table { name, .. } => {
            if *context != ReferenceContext::Table {
                return false;
            }
            name.as_ref() == Some(&identifier.to_string())
        }
        ReferenceTarget::Memory { name, .. } => {
            if *context != ReferenceContext::Memory {
                return false;
            }
            name.as_ref() == Some(&identifier.to_string())
        }
        ReferenceTarget::Type { name, .. } => {
            if *context != ReferenceContext::Type {
                return false;
            }
            name.as_ref() == Some(&identifier.to_string())
        }
    }
}

/// Check if a numeric index matches the target
fn matches_target_index(
    index: usize,
    target: &ReferenceTarget,
    context: &ReferenceContext,
    line: u32,
    symbols: &SymbolTable,
    block_stack: &[BlockInfo],
) -> bool {
    match target {
        ReferenceTarget::Function {
            index: target_index,
            ..
        } => {
            // Only match in Call context, not Function context (which is the definition)
            if *context != ReferenceContext::Call {
                return false;
            }
            index == *target_index
        }
        ReferenceTarget::Global {
            index: target_index,
            ..
        } => {
            if *context != ReferenceContext::Global {
                return false;
            }
            index == *target_index
        }
        ReferenceTarget::Local {
            index: target_index,
            function_start_byte,
            ..
        } => {
            if *context != ReferenceContext::Local {
                return false;
            }
            if index != *target_index {
                return false;
            }
            // Check scope: must be in the same function
            is_in_same_function_by_line(line, *function_start_byte, symbols)
        }
        ReferenceTarget::Parameter {
            index: target_index,
            function_start_byte,
            ..
        } => {
            if *context != ReferenceContext::Local {
                return false;
            }
            if index != *target_index {
                return false;
            }
            // Check scope: must be in the same function
            is_in_same_function_by_line(line, *function_start_byte, symbols)
        }
        ReferenceTarget::BlockLabel {
            label,
            function_start_byte,
            line: target_line,
            ..
        } => {
            if *context != ReferenceContext::Branch {
                return false;
            }
            // Resolve depth to block label
            if let Some(block) = resolve_block_by_depth(index, block_stack) {
                if let Some(ref block_label) = block.label {
                    if block_label == label {
                        // Check scope: must be in the same function
                        return is_in_same_function_by_line(line, *function_start_byte, symbols);
                    }
                } else {
                    // Unnamed block - match by line
                    if block.line == *target_line {
                        return is_in_same_function_by_line(line, *function_start_byte, symbols);
                    }
                }
            }
            false
        }
        ReferenceTarget::Table {
            index: target_index,
            ..
        } => {
            if *context != ReferenceContext::Table {
                return false;
            }
            index == *target_index
        }
        ReferenceTarget::Memory {
            index: target_index,
            ..
        } => {
            if *context != ReferenceContext::Memory {
                return false;
            }
            index == *target_index
        }
        ReferenceTarget::Type {
            index: target_index,
            ..
        } => {
            if *context != ReferenceContext::Type {
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
    // Depth 0 is the last element (innermost), depth 1 is second-to-last, etc.
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
    // Check if this node contains the target byte
    if node.start_byte() > target_byte || node.end_byte() < target_byte {
        return;
    }

    let kind = node.kind();

    // Check if this is a block node
    let is_block = matches!(
        kind,
        "block_block" | "block_loop" | "block_if" | "expr1_block" | "expr1_loop" | "expr1_if"
    );

    if is_block {
        // Extract block label if present
        let label = extract_block_label(&node, document);
        block_stack.push(BlockInfo {
            label,
            line: node.start_position().row as u32,
        });
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        build_block_stack_recursive(child, document, target_byte, block_stack);
    }
}

/// Convert a position to a byte offset
fn position_to_byte(document: &str, position: Position) -> usize {
    let mut byte_offset = 0;
    let mut current_line = 0;

    for (i, ch) in document.char_indices() {
        if current_line == position.line as usize {
            // We're on the target line, now count characters
            let line_start = byte_offset;

            for (char_count, (j, _)) in document[line_start..].char_indices().enumerate() {
                if char_count == position.character as usize {
                    return line_start + j;
                }
            }

            // If we reach here, the character is at the end of the line
            return document
                .len()
                .min(line_start + document[line_start..].len());
        }

        if ch == '\n' {
            current_line += 1;
            byte_offset = i + 1;
        }
    }

    byte_offset
}

/// Check if a line is within the same function as the target
fn is_in_same_function_by_line(
    line: u32,
    target_function_start_byte: usize,
    symbols: &SymbolTable,
) -> bool {
    // Find the function that contains this line
    for func in &symbols.functions {
        if line >= func.line && line <= func.end_line {
            return func.start_byte == target_function_start_byte;
        }
    }
    false
}

/// Determine reference context from AST node
fn determine_reference_context(node: &Node, document: &str) -> ReferenceContext {
    let mut current = *node;

    loop {
        let kind = current.kind();

        // Only check instr_plain nodes, not expr1_plain to avoid duplicates
        // (expr1_plain contains instr_plain, so we'd check the same instruction twice)
        if kind == "instr_plain" {
            let instr_text = &document[current.byte_range()];

            if instr_text.contains("call") && !instr_text.contains("call_indirect") {
                return ReferenceContext::Call;
            } else if instr_text.contains("local.") {
                return ReferenceContext::Local;
            } else if instr_text.contains("global.") {
                return ReferenceContext::Global;
            } else if instr_text.starts_with("br") || instr_text.contains(" br") {
                return ReferenceContext::Branch;
            } else if instr_text.contains("table.") {
                return ReferenceContext::Table;
            } else if instr_text.contains("memory.") {
                return ReferenceContext::Memory;
            }
        }

        // Check for type usage (not definition)
        if kind == "type_use" {
            return ReferenceContext::Type;
        }

        // Check for exports - these should be treated as references!
        if kind == "module_field_export" {
            let export_text = &document[current.byte_range()];
            if export_text.contains("func") {
                return ReferenceContext::Call;
            } else if export_text.contains("global") {
                return ReferenceContext::Global;
            } else if export_text.contains("table") {
                return ReferenceContext::Table;
            } else if export_text.contains("memory") {
                return ReferenceContext::Memory;
            }
        }

        // Check for function definition
        if kind == "module_field_func" {
            return ReferenceContext::Function;
        }

        // Walk up the tree
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            break;
        }
    }

    ReferenceContext::General
}

/// Determine context from line text (fallback for incomplete code)
fn determine_context_from_line(line: &str) -> ReferenceContext {
    if line.contains("call") {
        ReferenceContext::Call
    } else if line.contains("global") {
        ReferenceContext::Global
    } else if line.contains("local") {
        ReferenceContext::Local
    } else if line.contains("br") {
        ReferenceContext::Branch
    } else if line.contains("table") {
        ReferenceContext::Table
    } else if line.contains("memory") {
        ReferenceContext::Memory
    } else if line.contains("type") {
        ReferenceContext::Type
    } else if line.contains("func") {
        ReferenceContext::Function
    } else {
        ReferenceContext::General
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

/// Get the definition location for a target
fn get_definition_location(
    target: &ReferenceTarget,
    symbols: &SymbolTable,
    uri: &str,
) -> Option<Location> {
    let lsp_uri = Url::parse(uri).ok()?;

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
            // Find the function containing this local
            let func = symbols
                .functions
                .iter()
                .find(|f| f.start_byte == *function_start_byte)?;
            let total_params = func.parameters.len();
            if *index >= total_params {
                let local_index = *index - total_params;
                func.locals.get(local_index)?.range.as_ref()?
            } else {
                return None;
            }
        }
        ReferenceTarget::Parameter {
            index,
            function_start_byte,
            ..
        } => {
            // Find the function containing this parameter
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
            // Find the function containing this block
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
    };

    Some(Location {
        uri: lsp_uri,
        range: *range,
    })
}

/// Convert tree-sitter Node to LSP Range
fn node_to_range(node: &Node) -> Range {
    Range {
        start: Position {
            line: node.start_position().row as u32,
            character: node.start_position().column as u32,
        },
        end: Position {
            line: node.end_position().row as u32,
            character: node.end_position().column as u32,
        },
    }
}

/// Parse a natural number (decimal or hex)
fn parse_nat(text: &str) -> Result<usize, std::num::ParseIntError> {
    if text.starts_with("0x") || text.starts_with("0X") {
        usize::from_str_radix(&text[2..], 16)
    } else {
        text.parse::<usize>()
    }
}
