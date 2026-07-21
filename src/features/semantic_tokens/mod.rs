//! Semantic tokens — server-side classification of identifiers and indices
//! for syntax highlighting.
//!
//! Walks the tree once and classifies every `$identifier` and numeric index
//! using the same symbol tables and context detection that hover, definition,
//! and references already use. Keywords, strings, and literals are left to the
//! client's base grammar; only resolved symbols get tokens, so unresolved
//! references simply stay uncolored.
//!
//! Shared core logic for native and WASM builds. The native LSP wrapper at the
//! bottom converts to the LSP delta-encoded wire format.

// Allow needless_borrow/borrow_deref_ref: &kind and &*kind are required for WASM
// (String -> &str) but appear unnecessary on native (&str -> &str).
#![allow(clippy::needless_borrow, clippy::borrow_deref_ref)]

#[cfg(feature = "native")]
use tree_sitter::{Node, Tree};

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::{Node, Tree};

use crate::core::types::Position;
use crate::parser::ModuleInfo;
use crate::symbol_lookup::{
    find_block_in_function, find_local_in_function, find_param_in_function,
};
use crate::symbols::{SymbolTable, TypeKind};
use crate::utils::{
    context_from_instruction_text, determine_catch_clause_context, find_containing_function,
    is_block_kind, InstructionContext,
};

#[cfg(all(test, feature = "native"))]
mod tests;

/// Semantic classification of a single token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticTokenKind {
    Function,
    Parameter,
    Local,
    Global,
    Table,
    Memory,
    Type,
    Tag,
    Data,
    Elem,
    Label,
    /// Struct field name
    Property,
    /// Module name in `(module $name ...)`
    Module,
}

/// A classified token with absolute document coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticTokenInfo {
    pub line: u32,
    pub start_char: u32,
    pub length: u32,
    pub kind: SemanticTokenKind,
    pub is_declaration: bool,
    pub is_readonly: bool,
}

/// Reference context for a token, derived from its enclosing syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefContext {
    Function,
    Global,
    Local,
    Label,
    Table,
    Memory,
    Type,
    Tag,
    Data,
    Elem,
    /// No specific context — attempt name-based resolution (identifiers only)
    General,
    /// Definitely not a symbol reference (e.g. an i32.const operand)
    Literal,
}

/// Main entry point: classify every identifier and index in the document.
/// Returns tokens in document order with absolute positions.
pub fn provide_semantic_tokens(
    document: &str,
    modules: &[ModuleInfo],
    tree: &Tree,
) -> Vec<SemanticTokenInfo> {
    let mut tokens = Vec::new();
    walk_node(tree.root_node(), document, modules, &mut tokens);
    tokens.sort_by_key(|t| (t.line, t.start_char));
    tokens
}

fn walk_node(
    node: Node,
    document: &str,
    modules: &[ModuleInfo],
    tokens: &mut Vec<SemanticTokenInfo>,
) {
    let kind = node.kind();

    match &*kind {
        "identifier" => {
            classify_identifier(&node, document, modules, tokens);
            return;
        }
        // `nat` may wrap `dec_nat`/`hex_nat`; classify at the outermost numeric
        // node and don't recurse, so each number is considered exactly once.
        "nat" | "dec_nat" | "hex_nat" => {
            classify_nat(&node, document, modules, tokens);
            return;
        }
        "comment_block"
        | "comment_line"
        | "comment_block_annot"
        | "comment_line_annot"
        | "string" => return,
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(child, document, modules, tokens);
    }
}

/// Find the SymbolTable for the module containing the given line.
fn symbols_for_line(modules: &[ModuleInfo], line: u32) -> Option<&SymbolTable> {
    modules
        .iter()
        .find(|m| line >= m.range.start.line && line <= m.range.end.line)
        .or_else(|| modules.first())
        .map(|m| &m.symbols)
}

fn push_token(
    tokens: &mut Vec<SemanticTokenInfo>,
    node: &Node,
    kind: SemanticTokenKind,
    is_declaration: bool,
    is_readonly: bool,
) {
    let start = node.start_position();
    tokens.push(SemanticTokenInfo {
        line: start.row as u32,
        start_char: start.column as u32,
        length: (node.end_byte() - node.start_byte()) as u32,
        kind,
        is_declaration,
        is_readonly,
    });
}

/// Map a declaration site (identifier as direct child of its defining form)
/// to a token kind. Returns None if the parent is not a declaration site.
fn declaration_kind(
    parent_kind: &str,
    text: &str,
    modules: &[ModuleInfo],
    line: u32,
) -> Option<(SemanticTokenKind, bool)> {
    let kind = match parent_kind {
        "module" => SemanticTokenKind::Module,
        "module_field_func" | "import_desc_func_type" | "import_desc_type_use" => {
            SemanticTokenKind::Function
        }
        "module_field_global" | "import_desc_global_type" => {
            let readonly = symbols_for_line(modules, line)
                .and_then(|s| s.get_global_by_name(text))
                .is_some_and(|g| !g.is_mutable);
            return Some((SemanticTokenKind::Global, readonly));
        }
        "module_field_table" | "import_desc_table_type" => SemanticTokenKind::Table,
        "module_field_memory" | "import_desc_memory_type" => SemanticTokenKind::Memory,
        "module_field_type" | "module_field_rec" => SemanticTokenKind::Type,
        "module_field_tag" | "import_desc_tag_type" => SemanticTokenKind::Tag,
        "module_field_data" => SemanticTokenKind::Data,
        "module_field_elem" => SemanticTokenKind::Elem,
        "func_locals_one" => SemanticTokenKind::Local,
        "func_type_params_one" => SemanticTokenKind::Parameter,
        "field_type" => SemanticTokenKind::Property,
        k if is_block_kind(k) => SemanticTokenKind::Label,
        _ => return None,
    };
    Some((kind, false))
}

/// Determine the reference context for an identifier or index by walking up
/// the tree. Unlike `determine_instruction_context`, this stops at the first
/// enclosing plain instruction: an operand of `i32.const` must not inherit the
/// context of an outer `call` expression.
fn reference_context(node: &Node, document: &str) -> RefContext {
    // Catch clauses need positional disambiguation (first index is the tag,
    // the rest are branch labels).
    if let Some(ctx) = determine_catch_clause_context(node, document) {
        return match ctx {
            InstructionContext::Tag => RefContext::Tag,
            _ => RefContext::Label,
        };
    }

    let original_start = node.start_byte();
    let mut current = node_copy!(node);

    loop {
        node_kind!(kind = current);
        match kind {
            // The nearest plain instruction decides; if it takes no symbolic
            // operand (e.g. i32.const), the token is a literal.
            "instr_plain" | "expr1_plain" => {
                return context_from_instruction_text(&document[current.byte_range()])
                    .map_or(RefContext::Literal, instruction_ref_context);
            }
            // call_indirect / return_call_indirect: a direct index child is the
            // table; the type is wrapped in type_use (matched before we get here).
            "instr_call" | "expr1_call" | "instr_list_call" => return RefContext::Table,
            "type_use" | "ref_type_ref" | "ref_type_concrete" | "module_field_type"
            | "module_field_rec" => return RefContext::Type,
            "export_desc_func" | "module_field_start" => return RefContext::Function,
            "export_desc_global" => return RefContext::Global,
            "export_desc_table" | "table_use" => return RefContext::Table,
            "export_desc_memory" | "memory_use" => return RefContext::Memory,
            "export_desc_tag" => return RefContext::Tag,
            // (elem func $f1 $f2 ...) — indexes are function references
            "elem_list" => {
                if crate::utils::find_child_by_kind(&current, "elem_kind").is_some() {
                    return RefContext::Function;
                }
            }
            // Bare indexes after the offset in (elem (i32.const 0) $f) are
            // function references; anything else in the elem field is not.
            "module_field_elem" => {
                let mut cursor = current.walk();
                let mut past_offset = false;
                for child in current.children(&mut cursor) {
                    node_kind!(ck = child);
                    if ck == "offset" {
                        past_offset = true;
                    } else if ck == "index" && past_offset {
                        let cr = child.byte_range();
                        if cr.start <= original_start && original_start < cr.end {
                            return RefContext::Function;
                        }
                    }
                }
                return RefContext::Literal;
            }
            _ => {}
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }

    RefContext::General
}

fn instruction_ref_context(ctx: InstructionContext) -> RefContext {
    match ctx {
        InstructionContext::Call => RefContext::Function,
        InstructionContext::Global => RefContext::Global,
        InstructionContext::Local => RefContext::Local,
        InstructionContext::Branch | InstructionContext::Block => RefContext::Label,
        InstructionContext::Table => RefContext::Table,
        InstructionContext::Memory => RefContext::Memory,
        InstructionContext::Type => RefContext::Type,
        InstructionContext::Tag => RefContext::Tag,
        InstructionContext::Data => RefContext::Data,
        InstructionContext::Elem => RefContext::Elem,
        InstructionContext::Function | InstructionContext::General => RefContext::General,
    }
}

/// True if any struct type in the module has a field with this name.
fn is_struct_field_name(symbols: &SymbolTable, name: &str) -> bool {
    symbols.types.iter().any(|t| {
        matches!(&t.kind, TypeKind::Struct { fields }
            if fields.iter().any(|(n, _, _)| n.as_deref() == Some(name)))
    })
}

fn classify_identifier(
    node: &Node,
    document: &str,
    modules: &[ModuleInfo],
    tokens: &mut Vec<SemanticTokenInfo>,
) {
    let text = &document[node.byte_range()];
    let line = node.start_position().row as u32;

    // Declaration sites: the identifier is a direct child of its defining form.
    if let Some(parent) = node.parent() {
        node_kind!(pk = parent);
        if let Some((kind, readonly)) = declaration_kind(pk, text, modules, line) {
            push_token(tokens, node, kind, true, readonly);
            return;
        }
    }

    let Some(symbols) = symbols_for_line(modules, line) else {
        return;
    };
    let position = Position::new(line, node.start_position().column as u32);

    let token = match reference_context(node, document) {
        RefContext::Function => symbols
            .get_function_by_name(text)
            .map(|_| (SemanticTokenKind::Function, false)),
        RefContext::Global => symbols
            .get_global_by_name(text)
            .map(|g| (SemanticTokenKind::Global, !g.is_mutable)),
        RefContext::Local => resolve_local_name(symbols, position, text),
        RefContext::Label => find_containing_function(symbols, position)
            .and_then(|f| find_block_in_function(text, f))
            .map(|_| (SemanticTokenKind::Label, false)),
        RefContext::Table => symbols
            .get_table_by_name(text)
            .map(|_| (SemanticTokenKind::Table, false)),
        RefContext::Memory => symbols
            .get_memory_by_name(text)
            .map(|_| (SemanticTokenKind::Memory, false)),
        RefContext::Type => resolve_type_context_name(symbols, position, text),
        RefContext::Tag => symbols
            .get_tag_by_name(text)
            .map(|_| (SemanticTokenKind::Tag, false)),
        RefContext::Data => symbols
            .get_data_by_name(text)
            .map(|_| (SemanticTokenKind::Data, false))
            .or_else(|| {
                // memory.init $mem $data — first index is the memory
                symbols
                    .get_memory_by_name(text)
                    .map(|_| (SemanticTokenKind::Memory, false))
            }),
        RefContext::Elem => symbols
            .get_elem_by_name(text)
            .map(|_| (SemanticTokenKind::Elem, false))
            .or_else(|| {
                // table.init $table $elem — first index is the table
                symbols
                    .get_table_by_name(text)
                    .map(|_| (SemanticTokenKind::Table, false))
            }),
        RefContext::General => resolve_general_name(symbols, position, text),
        RefContext::Literal => None,
    };

    if let Some((kind, readonly)) = token {
        push_token(tokens, node, kind, false, readonly);
    }
}

/// Resolve a named local-context reference to a parameter or local.
fn resolve_local_name(
    symbols: &SymbolTable,
    position: Position,
    text: &str,
) -> Option<(SemanticTokenKind, bool)> {
    let func = find_containing_function(symbols, position)?;
    if find_param_in_function(text, func).is_some() {
        Some((SemanticTokenKind::Parameter, false))
    } else {
        find_local_in_function(text, func).map(|_| (SemanticTokenKind::Local, false))
    }
}

/// Resolve a Type-context identifier: type name, struct field, or (for
/// br_on_cast-style instructions whose first index is a label) a block label.
fn resolve_type_context_name(
    symbols: &SymbolTable,
    position: Position,
    text: &str,
) -> Option<(SemanticTokenKind, bool)> {
    if symbols.get_type_by_name(text).is_some() {
        return Some((SemanticTokenKind::Type, false));
    }
    if is_struct_field_name(symbols, text) {
        return Some((SemanticTokenKind::Property, false));
    }
    find_containing_function(symbols, position)
        .and_then(|f| find_block_in_function(text, f))
        .map(|_| (SemanticTokenKind::Label, false))
}

/// Fallback resolution when no syntactic context was found, mirroring the
/// lookup order used by references' General context.
fn resolve_general_name(
    symbols: &SymbolTable,
    position: Position,
    text: &str,
) -> Option<(SemanticTokenKind, bool)> {
    if symbols.get_function_by_name(text).is_some() {
        return Some((SemanticTokenKind::Function, false));
    }
    if let Some(func) = find_containing_function(symbols, position) {
        if find_param_in_function(text, func).is_some() {
            return Some((SemanticTokenKind::Parameter, false));
        }
        if find_local_in_function(text, func).is_some() {
            return Some((SemanticTokenKind::Local, false));
        }
        if find_block_in_function(text, func).is_some() {
            return Some((SemanticTokenKind::Label, false));
        }
    }
    if let Some(global) = symbols.get_global_by_name(text) {
        return Some((SemanticTokenKind::Global, !global.is_mutable));
    }
    if symbols.get_table_by_name(text).is_some() {
        return Some((SemanticTokenKind::Table, false));
    }
    if symbols.get_memory_by_name(text).is_some() {
        return Some((SemanticTokenKind::Memory, false));
    }
    if symbols.get_type_by_name(text).is_some() {
        return Some((SemanticTokenKind::Type, false));
    }
    if symbols.get_tag_by_name(text).is_some() {
        return Some((SemanticTokenKind::Tag, false));
    }
    if symbols.get_data_by_name(text).is_some() {
        return Some((SemanticTokenKind::Data, false));
    }
    if symbols.get_elem_by_name(text).is_some() {
        return Some((SemanticTokenKind::Elem, false));
    }
    if is_struct_field_name(symbols, text) {
        return Some((SemanticTokenKind::Property, false));
    }
    None
}

fn classify_nat(
    node: &Node,
    document: &str,
    modules: &[ModuleInfo],
    tokens: &mut Vec<SemanticTokenInfo>,
) {
    let text = &document[node.byte_range()];
    let Some(index) = crate::parser::parse_wat_nat(text).map(|v| v as usize) else {
        return;
    };

    let line = node.start_position().row as u32;
    let Some(symbols) = symbols_for_line(modules, line) else {
        return;
    };
    let position = Position::new(line, node.start_position().column as u32);

    let token = match reference_context(node, document) {
        RefContext::Function => symbols
            .get_function_by_index(index)
            .map(|_| (SemanticTokenKind::Function, false)),
        RefContext::Global => symbols
            .get_global_by_index(index)
            .map(|g| (SemanticTokenKind::Global, !g.is_mutable)),
        RefContext::Local => find_containing_function(symbols, position).and_then(|func| {
            if index < func.parameters.len() {
                Some((SemanticTokenKind::Parameter, false))
            } else {
                func.locals
                    .get(index - func.parameters.len())
                    .map(|_| (SemanticTokenKind::Local, false))
            }
        }),
        // Numeric branch targets are relative depths; color them as labels
        // whenever they appear inside a function.
        RefContext::Label => {
            find_containing_function(symbols, position).map(|_| (SemanticTokenKind::Label, false))
        }
        RefContext::Table => symbols
            .get_table_by_index(index)
            .map(|_| (SemanticTokenKind::Table, false)),
        RefContext::Memory => symbols
            .get_memory_by_index(index)
            .map(|_| (SemanticTokenKind::Memory, false)),
        RefContext::Type => symbols
            .get_type_by_index(index)
            .map(|_| (SemanticTokenKind::Type, false)),
        RefContext::Tag => symbols
            .get_tag_by_index(index)
            .map(|_| (SemanticTokenKind::Tag, false)),
        RefContext::Data => symbols
            .get_data_by_index(index)
            .map(|_| (SemanticTokenKind::Data, false)),
        RefContext::Elem => symbols
            .get_elem_by_index(index)
            .map(|_| (SemanticTokenKind::Elem, false)),
        // Bare numbers with no symbolic context are literals.
        RefContext::General | RefContext::Literal => None,
    };

    if let Some((kind, readonly)) = token {
        push_token(tokens, node, kind, false, readonly);
    }
}

// ============================================================================
// Native LSP wrapper — legend and delta encoding
// ============================================================================

#[cfg(feature = "native")]
use tower_lsp::lsp_types::{Range, SemanticToken, SemanticTokenModifier, SemanticTokenType};

/// Token types legend. Index into this array = `token_type` on the wire.
#[cfg(feature = "native")]
pub const TOKEN_TYPES: [SemanticTokenType; 8] = [
    SemanticTokenType::FUNCTION,     // 0
    SemanticTokenType::PARAMETER,    // 1
    SemanticTokenType::VARIABLE,     // 2: locals, globals, tables, memories, data, elems
    SemanticTokenType::TYPE,         // 3
    SemanticTokenType::PROPERTY,     // 4: struct fields
    SemanticTokenType::EVENT,        // 5: exception tags
    SemanticTokenType::NAMESPACE,    // 6: module names
    SemanticTokenType::new("label"), // 7: block labels (rust-analyzer precedent)
];

/// Token modifiers legend. Bit position in this array = bit in the wire bitset.
#[cfg(feature = "native")]
pub const TOKEN_MODIFIERS: [SemanticTokenModifier; 3] = [
    SemanticTokenModifier::DECLARATION, // 1 << 0
    SemanticTokenModifier::READONLY,    // 1 << 1
    SemanticTokenModifier::STATIC,      // 1 << 2: module-level entities
];

#[cfg(feature = "native")]
fn token_type_index(kind: SemanticTokenKind) -> u32 {
    match kind {
        SemanticTokenKind::Function => 0,
        SemanticTokenKind::Parameter => 1,
        SemanticTokenKind::Local
        | SemanticTokenKind::Global
        | SemanticTokenKind::Table
        | SemanticTokenKind::Memory
        | SemanticTokenKind::Data
        | SemanticTokenKind::Elem => 2,
        SemanticTokenKind::Type => 3,
        SemanticTokenKind::Property => 4,
        SemanticTokenKind::Tag => 5,
        SemanticTokenKind::Module => 6,
        SemanticTokenKind::Label => 7,
    }
}

#[cfg(feature = "native")]
fn token_modifier_bitset(token: &SemanticTokenInfo) -> u32 {
    let mut bits = 0;
    if token.is_declaration {
        bits |= 1 << 0;
    }
    if token.is_readonly {
        bits |= 1 << 1;
    }
    if matches!(
        token.kind,
        SemanticTokenKind::Global
            | SemanticTokenKind::Table
            | SemanticTokenKind::Memory
            | SemanticTokenKind::Data
            | SemanticTokenKind::Elem
    ) {
        bits |= 1 << 2;
    }
    bits
}

/// Delta-encode absolute tokens into the LSP wire format.
#[cfg(feature = "native")]
fn encode_tokens(tokens: &[SemanticTokenInfo]) -> Vec<SemanticToken> {
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;
    tokens
        .iter()
        .map(|t| {
            let delta_line = t.line - prev_line;
            let delta_start = if delta_line == 0 {
                t.start_char - prev_start
            } else {
                t.start_char
            };
            prev_line = t.line;
            prev_start = t.start_char;
            SemanticToken {
                delta_line,
                delta_start,
                length: t.length,
                token_type: token_type_index(t.kind),
                token_modifiers_bitset: token_modifier_bitset(t),
            }
        })
        .collect()
}

/// Full-document semantic tokens in LSP wire format (native wrapper).
#[cfg(feature = "native")]
pub fn provide_semantic_tokens_lsp(
    document: &str,
    modules: &[ModuleInfo],
    tree: &Tree,
) -> Vec<SemanticToken> {
    encode_tokens(&provide_semantic_tokens(document, modules, tree))
}

/// Semantic tokens for a document range in LSP wire format (native wrapper).
#[cfg(feature = "native")]
pub fn provide_semantic_tokens_range_lsp(
    document: &str,
    modules: &[ModuleInfo],
    tree: &Tree,
    range: Range,
) -> Vec<SemanticToken> {
    let tokens: Vec<SemanticTokenInfo> = provide_semantic_tokens(document, modules, tree)
        .into_iter()
        .filter(|t| t.line >= range.start.line && t.line <= range.end.line)
        .collect();
    encode_tokens(&tokens)
}
