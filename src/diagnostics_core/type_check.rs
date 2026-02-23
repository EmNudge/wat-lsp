//! Core type checker implementing the WebAssembly validation algorithm (spec §3.3).
//!
//! This module provides a `TypeChecker` that tracks typed values on a value stack
//! and control frames on a control stack. It replaces the untyped `StackState` to
//! detect both stack underflow AND type mismatches.

use crate::core::types::Diagnostic;
use crate::symbols::{SymbolTable, TypeDef, TypeKind, ValueType};
use crate::utils::node_to_range;

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

/// What kind of control frame this is. Affects label_types resolution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum CtrlOpcode {
    Function,
    Block,
    Loop,
    If,
    TryTable,
}

/// A control frame on the control stack, per spec §3.3 appendix.
#[derive(Debug, Clone)]
pub(super) struct CtrlFrame {
    pub(super) opcode: CtrlOpcode,
    /// Block parameter types (consumed from outer stack on entry)
    pub(super) start_types: Vec<ValueType>,
    /// Block result types (left on stack on exit)
    pub(super) end_types: Vec<ValueType>,
    /// Value stack height when this frame was entered
    pub(super) height: usize,
    /// True after unreachable/br/return — polymorphic stack bottom
    pub(super) unreachable: bool,
    /// Optional label name (e.g., "$loop1") for named branch resolution
    pub(super) label: Option<String>,
}

/// Core type checker state machine implementing Wasm spec validation.
#[derive(Default)]
pub(super) struct TypeChecker {
    /// Value stack of types
    val_stack: Vec<ValueType>,
    /// Control frame stack
    ctrl_stack: Vec<CtrlFrame>,
    /// Collected diagnostics
    pub(super) diagnostics: Vec<Diagnostic>,
}

/// Check if two types are compatible for validation purposes.
/// Unknown matches anything (polymorphic). Otherwise types must match exactly,
/// with basic reference subtyping.
pub(super) fn types_compatible(actual: &ValueType, expected: &ValueType) -> bool {
    if *actual == ValueType::Unknown || *expected == ValueType::Unknown {
        return true;
    }
    if actual == expected {
        return true;
    }
    // Basic reference subtyping
    is_ref_subtype(actual, expected)
}

/// Check if `sub` is a subtype of `sup` for reference types.
fn is_ref_subtype(sub: &ValueType, sup: &ValueType) -> bool {
    use ValueType::*;
    match (sub, sup) {
        // nullref is bottom for internal ref hierarchy (nullable variants only)
        (Nullref, Anyref | Eqref | I31ref | Structref | Arrayref) => true,
        // nullfuncref is bottom for func hierarchy (nullable variants only)
        (NullFuncref, Funcref) => true,
        // nullexternref is bottom for extern hierarchy (nullable variants only)
        (NullExternref, Externref) => true,
        // Bottom types are NOT subtypes of non-nullable variants
        (
            Nullref,
            NonNullAnyref | NonNullEqref | NonNullI31ref | NonNullStructref | NonNullArrayref,
        ) => false,
        (NullFuncref, NonNullFuncref) => false,
        (NullExternref, NonNullExternref) => false,

        // Non-nullable <: nullable (same hierarchy)
        (NonNullFuncref, Funcref) => true,
        (NonNullExternref, Externref) => true,
        (NonNullAnyref, Anyref) => true,
        (NonNullEqref, Eqref | Anyref | NonNullAnyref) => true,
        (NonNullStructref, Structref | Eqref | Anyref | NonNullEqref | NonNullAnyref) => true,
        (NonNullArrayref, Arrayref | Eqref | Anyref | NonNullEqref | NonNullAnyref) => true,
        (NonNullI31ref, I31ref | Eqref | Anyref | NonNullEqref | NonNullAnyref) => true,

        // Nullable abstract hierarchy (existing rules)
        // i31ref, structref, arrayref <: eqref <: anyref
        (I31ref | Structref | Arrayref | Eqref, Anyref) => true,
        (I31ref | Structref | Arrayref, Eqref) => true,

        // Concrete Ref(n)/RefNull(n) are subtypes of all abstract ref supertypes.
        // Without symbol table we can't distinguish struct/array/func kinds, so we
        // accept all abstract supertypes (precise checking done by types_compatible_with_symbols).
        (Ref(_) | RefNull(_), Funcref | Anyref | Eqref | Arrayref | Structref) => true,
        // Non-null concrete refs are subtypes of non-nullable abstract supertypes
        (
            Ref(_),
            NonNullFuncref | NonNullAnyref | NonNullEqref | NonNullArrayref | NonNullStructref,
        ) => true,
        // NullFuncref is bottom of func hierarchy — subtype of all nullable func refs
        (NullFuncref, RefNull(_) | Structref) => true,
        // Nullref is bottom of internal ref hierarchy — subtype of all nullable internal refs
        (Nullref, RefNull(_)) => true,
        // Concrete ref cross-compatibility: without symbols we can't verify type
        // equivalence (e.g., iso-recursive rec group identity), so be permissive.
        // Precise checking done by types_compatible_with_symbols.
        (Ref(_), Ref(_) | RefNull(_)) => true,
        (RefNull(_), RefNull(_)) => true,
        // Structref compat with concrete refs (Structref was used as unresolved-ref placeholder)
        (Structref, Ref(_) | RefNull(_)) => true,
        _ => false,
    }
}

/// Check type compatibility with access to the symbol table for concrete GC type resolution.
///
/// This extends `types_compatible()` with knowledge of concrete type definitions,
/// enabling validation of `Ref(n)` / `RefNull(n)` against abstract supertypes
/// (structref, arrayref, eqref, anyref) and concrete parent chains.
pub(super) fn types_compatible_with_symbols(
    actual: &ValueType,
    expected: &ValueType,
    symbols: &SymbolTable,
) -> bool {
    use ValueType::*;
    if *actual == Unknown || *expected == Unknown {
        return true;
    }
    if actual == expected {
        return true;
    }
    // For concrete ref types, use precise symbol-based checking instead of
    // the permissive is_ref_subtype (which treats Ref(a) ~= Ref(b) without symbols).
    let is_concrete = matches!(
        (actual, expected),
        (Ref(_) | RefNull(_), Ref(_) | RefNull(_))
    );
    if is_concrete {
        return is_concrete_ref_subtype(actual, expected, symbols);
    }
    // For non-concrete ref types, use basic subtyping
    if is_ref_subtype(actual, expected) {
        return true;
    }
    is_concrete_ref_subtype(actual, expected, symbols)
}

/// Check concrete reference subtyping using the symbol table.
///
/// Handles:
/// - `Ref(n)` where n is struct → subtype of Structref, Eqref, Anyref
/// - `Ref(n)` where n is array → subtype of Arrayref, Eqref, Anyref
/// - `Ref(n)` where n is func → subtype of Funcref
/// - `Ref(child)` → subtype of `Ref(parent)` via declared parent chain
/// - `Ref(n)` → subtype of `RefNull(n)` (non-null <: nullable)
/// - `RefNull(n)` carries same hierarchy but includes null
fn is_concrete_ref_subtype(sub: &ValueType, sup: &ValueType, symbols: &SymbolTable) -> bool {
    use ValueType::*;

    let (sub_idx, sub_nullable) = match sub {
        Ref(n) => (*n, false),
        RefNull(n) => (*n, true),
        _ => return false,
    };

    // Concrete ref vs concrete ref subtyping via parent chain
    match sup {
        RefNull(sup_n) => {
            // Ref(n) <: RefNull(n) and RefNull(n) <: RefNull(n) via parent chain
            return is_type_subtype(sub_idx as usize, *sup_n as usize, symbols);
        }
        Ref(sup_n) => {
            if sub_nullable {
                // RefNull(n) is NOT a subtype of Ref(m) — nullable cannot satisfy non-null
                return false;
            }
            return is_type_subtype(sub_idx as usize, *sup_n as usize, symbols);
        }
        _ => {}
    }

    // Check concrete type against abstract supertypes
    let type_def = match symbols.get_type_by_index(sub_idx as usize) {
        Some(td) => td,
        None => return false,
    };

    match (&type_def.kind, sup) {
        (TypeKind::Struct { .. }, Structref | Eqref | Anyref) => true,
        (TypeKind::Array { .. }, Arrayref | Eqref | Anyref) => true,
        (TypeKind::Func { .. }, Funcref) => true,
        // Non-null concrete refs can satisfy non-nullable abstract supertypes
        (TypeKind::Struct { .. }, NonNullStructref | NonNullEqref | NonNullAnyref) => !sub_nullable,
        (TypeKind::Array { .. }, NonNullArrayref | NonNullEqref | NonNullAnyref) => !sub_nullable,
        (TypeKind::Func { .. }, NonNullFuncref) => !sub_nullable,
        _ => false,
    }
}

/// Check if `child_idx` is a declared subtype of `parent_idx` via the parent chain.
///
/// Walks up the `TypeDef.parent` chain from child, checking if parent_idx is encountered.
/// Max depth of 64 prevents infinite loops from cyclic declarations.
fn is_type_subtype(child_idx: usize, parent_idx: usize, symbols: &SymbolTable) -> bool {
    if child_idx == parent_idx {
        return true;
    }

    // Check iso-recursive type equivalence (types in different rec groups
    // can be equivalent if the rec groups have the same shape)
    if are_types_equivalent(child_idx, parent_idx, symbols) {
        return true;
    }

    // Walk the declared parent chain
    let mut current_idx = child_idx;
    for _ in 0..64 {
        let type_def = match symbols.get_type_by_index(current_idx) {
            Some(td) => td,
            None => return false,
        };

        let parent_ref = match &type_def.parent {
            Some(p) => p.as_str(),
            None => return false,
        };

        // Resolve parent reference to index
        let resolved_idx = if parent_ref.starts_with('$') {
            match symbols.get_type_by_name(parent_ref) {
                Some(td) => td.index,
                None => return false,
            }
        } else {
            match parent_ref.parse::<usize>() {
                Ok(idx) => idx,
                Err(_) => return false,
            }
        };

        if resolved_idx == parent_idx {
            return true;
        }
        // Also check equivalence with the target
        if are_types_equivalent(resolved_idx, parent_idx, symbols) {
            return true;
        }
        current_idx = resolved_idx;
    }

    false
}

/// Check iso-recursive type equivalence between two type indices.
/// Two types are equivalent if they are in rec groups with the same shape
/// (same size, same relative position, and structurally identical type definitions
/// with internal references mapped by relative position).
fn are_types_equivalent(idx_a: usize, idx_b: usize, symbols: &SymbolTable) -> bool {
    if idx_a == idx_b {
        return true;
    }
    let td_a = match symbols.get_type_by_index(idx_a) {
        Some(td) => td,
        None => return false,
    };
    let td_b = match symbols.get_type_by_index(idx_b) {
        Some(td) => td,
        None => return false,
    };

    // Must be in rec groups of the same size
    if td_a.rec_group_size != td_b.rec_group_size {
        return false;
    }

    // Compute offsets within rec groups
    let group_a_start = idx_a - (idx_a - find_rec_group_start(idx_a, symbols));
    let group_b_start = idx_b - (idx_b - find_rec_group_start(idx_b, symbols));
    let offset_a = idx_a - group_a_start;
    let offset_b = idx_b - group_b_start;

    // Must be at the same position within their rec groups
    if offset_a != offset_b {
        return false;
    }

    // Compare all types in both rec groups pairwise
    let size = td_a.rec_group_size;
    for i in 0..size {
        let ta = match symbols.get_type_by_index(group_a_start + i) {
            Some(t) => t,
            None => return false,
        };
        let tb = match symbols.get_type_by_index(group_b_start + i) {
            Some(t) => t,
            None => return false,
        };
        if !types_structurally_equal(ta, tb, group_a_start, group_b_start, size, symbols) {
            return false;
        }
    }
    true
}

/// Find the start index of the rec group containing the given type index.
fn find_rec_group_start(idx: usize, symbols: &SymbolTable) -> usize {
    let td = match symbols.get_type_by_index(idx) {
        Some(t) => t,
        None => return idx,
    };
    let rec_id = td.rec_group_id;
    // Scan backward to find the first type with the same rec_group_id
    let mut start = idx;
    while start > 0 {
        if let Some(prev) = symbols.get_type_by_index(start - 1) {
            if prev.rec_group_id == rec_id {
                start -= 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    start
}

/// Check if two type definitions are structurally equal, mapping internal references.
fn types_structurally_equal(
    a: &TypeDef,
    b: &TypeDef,
    group_a_start: usize,
    group_b_start: usize,
    group_size: usize,
    symbols: &SymbolTable,
) -> bool {
    // Finality must match
    if a.is_final != b.is_final {
        return false;
    }
    // Parent declarations must match (mapped)
    match (&a.parent, &b.parent) {
        (None, None) => {}
        (Some(pa), Some(pb)) => {
            let ra = resolve_type_ref(pa, symbols);
            let rb = resolve_type_ref(pb, symbols);
            if !refs_equivalent(ra, rb, group_a_start, group_b_start, group_size, symbols) {
                return false;
            }
        }
        _ => return false,
    }
    // Kind must match
    match (&a.kind, &b.kind) {
        (
            TypeKind::Func {
                params: pa,
                results: ra,
            },
            TypeKind::Func {
                params: pb,
                results: rb,
            },
        ) => {
            if pa.len() != pb.len() || ra.len() != rb.len() {
                return false;
            }
            for (va, vb) in pa.iter().zip(pb.iter()) {
                if !value_types_equivalent(
                    va,
                    vb,
                    group_a_start,
                    group_b_start,
                    group_size,
                    symbols,
                ) {
                    return false;
                }
            }
            for (va, vb) in ra.iter().zip(rb.iter()) {
                if !value_types_equivalent(
                    va,
                    vb,
                    group_a_start,
                    group_b_start,
                    group_size,
                    symbols,
                ) {
                    return false;
                }
            }
            true
        }
        (TypeKind::Struct { fields: fa }, TypeKind::Struct { fields: fb }) => {
            if fa.len() != fb.len() {
                return false;
            }
            for ((_, ta, ma), (_, tb, mb)) in fa.iter().zip(fb.iter()) {
                if ma != mb {
                    return false;
                }
                if !value_types_equivalent(
                    ta,
                    tb,
                    group_a_start,
                    group_b_start,
                    group_size,
                    symbols,
                ) {
                    return false;
                }
            }
            true
        }
        (
            TypeKind::Array {
                element_type: ea,
                mutable: ma,
            },
            TypeKind::Array {
                element_type: eb,
                mutable: mb,
            },
        ) => {
            ma == mb
                && value_types_equivalent(ea, eb, group_a_start, group_b_start, group_size, symbols)
        }
        _ => false,
    }
}

/// Check if two value types are equivalent with rec group reference mapping.
fn value_types_equivalent(
    a: &ValueType,
    b: &ValueType,
    group_a_start: usize,
    group_b_start: usize,
    group_size: usize,
    symbols: &SymbolTable,
) -> bool {
    use ValueType::*;
    match (a, b) {
        (Ref(na), Ref(nb)) | (RefNull(na), RefNull(nb)) => refs_equivalent(
            Some(*na as usize),
            Some(*nb as usize),
            group_a_start,
            group_b_start,
            group_size,
            symbols,
        ),
        _ => a == b,
    }
}

/// Check if two type references are equivalent under rec group mapping.
fn refs_equivalent(
    a: Option<usize>,
    b: Option<usize>,
    group_a_start: usize,
    group_b_start: usize,
    group_size: usize,
    symbols: &SymbolTable,
) -> bool {
    match (a, b) {
        (Some(ra), Some(rb)) => {
            let in_group_a = ra >= group_a_start && ra < group_a_start + group_size;
            let in_group_b = rb >= group_b_start && rb < group_b_start + group_size;
            if in_group_a && in_group_b {
                // Both internal — compare relative positions
                (ra - group_a_start) == (rb - group_b_start)
            } else if !in_group_a && !in_group_b {
                // Both external — check type equivalence recursively
                are_types_equivalent(ra, rb, symbols)
            } else {
                // One internal, one external — not equivalent
                false
            }
        }
        (None, None) => true,
        _ => false,
    }
}

/// Resolve a type reference string to a type index.
fn resolve_type_ref(ref_str: &str, symbols: &SymbolTable) -> Option<usize> {
    if ref_str.starts_with('$') {
        symbols.get_type_by_name(ref_str).map(|t| t.index)
    } else {
        ref_str.parse::<usize>().ok()
    }
}

impl TypeChecker {
    /// Create a new TypeChecker.
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Push a value type onto the value stack.
    pub(super) fn push_val(&mut self, ty: ValueType) {
        self.val_stack.push(ty);
    }

    /// Pop a value from the value stack.
    /// If stack is at or below frame height AND frame is unreachable, returns Unknown.
    /// If stack underflows (below frame height, not unreachable), returns None.
    pub(super) fn pop_val(&mut self) -> Option<ValueType> {
        let height = self.current_frame_height();
        if self.val_stack.len() == height {
            if self.is_current_unreachable() {
                return Some(ValueType::Unknown);
            }
            return None; // underflow
        }
        self.val_stack.pop()
    }

    /// Pop a value and check it matches the expected type.
    /// Returns the actual type popped, or None on underflow.
    /// Emits diagnostic on type mismatch or underflow.
    pub(super) fn pop_expect(&mut self, expected: &ValueType, node: &Node) -> Option<ValueType> {
        let actual = self.pop_val();
        match actual {
            Some(ref ty) => {
                if !types_compatible(ty, expected) {
                    let range = node_to_range(node);
                    self.diagnostics.push(
                        Diagnostic::error(
                            range,
                            format!("type mismatch: expected {}, found {}", expected, ty),
                        )
                        .with_code("type-mismatch"),
                    );
                }
                actual
            }
            None => {
                // Underflow in non-unreachable frame
                let range = node_to_range(node);
                self.diagnostics.push(
                    Diagnostic::error(
                        range,
                        format!("type mismatch: expected {} but stack was empty", expected),
                    )
                    .with_code("type-mismatch"),
                );
                None
            }
        }
    }

    /// Pop a value and check it matches, without emitting diagnostics (caller handles errors).
    /// Returns (actual_type, matched). matched is false if type mismatch.
    /// Push multiple values onto the stack.
    pub(super) fn push_vals(&mut self, types: &[ValueType]) {
        self.val_stack.extend_from_slice(types);
    }

    /// Pop multiple values and check types (in reverse order, as per spec).
    /// Returns true if all popped successfully and matched.
    /// Does NOT emit diagnostics — callers should use `pop_vals_for_instr` for
    /// instruction-level checks with proper diagnostic messages.
    pub(super) fn pop_vals(&mut self, expected: &[ValueType], node: &Node) -> bool {
        self.pop_vals_inner(expected, node, None)
    }

    /// Pop multiple values for a named instruction.
    /// On underflow, emits a "Stack underflow" diagnostic with instruction name.
    /// On type mismatch, emits a "type mismatch" diagnostic.
    pub(super) fn pop_vals_for_instr(
        &mut self,
        expected: &[ValueType],
        node: &Node,
        instr_name: &str,
    ) -> bool {
        self.pop_vals_inner(expected, node, Some(instr_name))
    }

    /// Inner implementation for pop_vals with optional instruction name for diagnostics.
    fn pop_vals_inner(
        &mut self,
        expected: &[ValueType],
        node: &Node,
        instr_name: Option<&str>,
    ) -> bool {
        // Check for underflow first (before modifying the stack)
        let height = self.current_frame_height();
        let available = self.val_stack.len().saturating_sub(height);
        if available < expected.len() && !self.is_current_unreachable() {
            // Underflow — emit diagnostic and drain what we can
            let range = node_to_range(node);
            let needed = expected.len();
            let value_word = if needed == 1 { "value" } else { "values" };
            if let Some(name) = instr_name {
                self.diagnostics.push(
                    Diagnostic::error(
                        range,
                        format!(
                            "Stack underflow: '{}' requires {} {} but only {} available on stack",
                            name, needed, value_word, available
                        ),
                    )
                    .with_code("stack-underflow"),
                );
            } else {
                self.diagnostics.push(
                    Diagnostic::error(
                        range,
                        format!(
                            "type mismatch: expected {} {} on stack, found {}",
                            needed, value_word, available
                        ),
                    )
                    .with_code("type-mismatch"),
                );
            }
            // Pop whatever is available (to keep stack consistent)
            self.val_stack.truncate(height);
            return false;
        }

        let mut ok = true;
        // Pop in reverse order (last expected type is on top of stack)
        for ty in expected.iter().rev() {
            match self.pop_val() {
                Some(ref actual) => {
                    if !types_compatible(actual, ty) {
                        let range = node_to_range(node);
                        self.diagnostics.push(
                            Diagnostic::error(
                                range,
                                format!("type mismatch: expected {}, found {}", ty, actual),
                            )
                            .with_code("type-mismatch"),
                        );
                        ok = false;
                    }
                }
                None => {
                    // Should not happen (checked above), but guard anyway
                    ok = false;
                }
            }
        }
        ok
    }

    /// Enter a new control frame (block, loop, if, function).
    pub(super) fn push_ctrl(
        &mut self,
        opcode: CtrlOpcode,
        start_types: Vec<ValueType>,
        end_types: Vec<ValueType>,
    ) {
        self.push_ctrl_labeled(opcode, start_types, end_types, None);
    }

    /// Enter a new control frame with an optional label name for named branch resolution.
    pub(super) fn push_ctrl_labeled(
        &mut self,
        opcode: CtrlOpcode,
        start_types: Vec<ValueType>,
        end_types: Vec<ValueType>,
        label: Option<String>,
    ) {
        let height = self.val_stack.len();
        // Push start_types onto val_stack first, then move into frame (avoids clone)
        self.push_vals(&start_types);
        self.ctrl_stack.push(CtrlFrame {
            opcode,
            start_types,
            end_types,
            height,
            unreachable: false,
            label,
        });
    }

    /// Exit a control frame. Validates that end_types match the stack.
    /// Returns the popped frame, or None if ctrl_stack is empty.
    pub(super) fn pop_ctrl(&mut self, node: &Node) {
        if self.ctrl_stack.is_empty() {
            return;
        }
        let frame = self.ctrl_stack.last().unwrap();
        let end_types = frame.end_types.clone();
        let height = frame.height;

        // Pop expected end types from the stack
        self.pop_vals(&end_types, node);

        // Check that we're back to frame height
        // Even in unreachable context, concrete values pushed after unreachable
        // count as excess values (spec §3.3 validation rules).
        let extra = self.val_stack.len().saturating_sub(height);
        if extra > 0 {
            let range = node_to_range(node);
            self.diagnostics.push(
                Diagnostic::error(
                    range,
                    format!(
                        "type mismatch: block leaves {} extra value(s) on stack",
                        extra
                    ),
                )
                .with_code("type-mismatch"),
            );
        }

        // Restore val_stack to frame height
        self.val_stack.truncate(height);

        self.ctrl_stack.pop();

        // Push end_types onto outer stack (reuse the already-cloned vec)
        self.push_vals(&end_types);
    }

    /// Handle the else transition in an if block.
    /// Validates the then branch produced end_types, resets stack to frame height,
    /// and pushes start_types for the else branch.
    pub(super) fn else_transition(&mut self, node: &Node) {
        if let Some(frame) = self.ctrl_stack.last() {
            let end_types = frame.end_types.clone();
            let start_types = frame.start_types.clone();
            let height = frame.height;
            let was_unreachable = frame.unreachable;

            // Validate then branch produced the expected end types
            if !was_unreachable {
                self.pop_vals(&end_types, node);
                // Check for excess values
                if self.val_stack.len() > height {
                    let extra = self.val_stack.len() - height;
                    let range = node_to_range(node);
                    self.diagnostics.push(
                        Diagnostic::error(
                            range,
                            format!(
                                "type mismatch: block leaves {} extra value(s) on stack",
                                extra
                            ),
                        )
                        .with_code("type-mismatch"),
                    );
                }
            }

            // Reset stack to frame height + start_types for else branch
            self.val_stack.truncate(height);
            self.push_vals(&start_types);

            // Reset unreachable flag for else branch
            if let Some(frame) = self.ctrl_stack.last_mut() {
                frame.unreachable = false;
            }
        }
    }

    /// Mark the current frame as unreachable.
    /// Truncates val_stack to frame height (polymorphic bottom).
    pub(super) fn mark_unreachable(&mut self) {
        if let Some(frame) = self.ctrl_stack.last_mut() {
            self.val_stack.truncate(frame.height);
            frame.unreachable = true;
        }
    }

    /// Check if the current frame is in an unreachable state (polymorphic stack).
    pub(super) fn is_unreachable(&self) -> bool {
        self.ctrl_stack
            .last()
            .map(|f| f.unreachable)
            .unwrap_or(false)
    }

    /// Get the label types for a frame at the given depth.
    /// For loops, label types are start_types (br restarts with params).
    /// For everything else, label types are end_types.
    pub(super) fn label_types(&self, depth: usize) -> Option<&[ValueType]> {
        let idx = self.ctrl_stack.len().checked_sub(1 + depth)?;
        let frame = &self.ctrl_stack[idx];
        Some(match frame.opcode {
            CtrlOpcode::Loop => &frame.start_types,
            _ => &frame.end_types,
        })
    }

    /// Look up label types at depth, copy them, and pop them for the given instruction.
    /// Returns the label types if found (owned, for callers that need to push them back).
    pub(super) fn pop_label_types_for_instr(
        &mut self,
        depth: usize,
        node: &Node,
        instr_name: &str,
    ) -> Option<Vec<ValueType>> {
        let label_types = self.label_types(depth)?.to_vec();
        self.pop_vals_for_instr(&label_types, node, instr_name);
        Some(label_types)
    }

    /// Get an owned copy of label types at the given depth.
    /// Useful when the caller needs both the types and a mutable borrow on self.
    pub(super) fn label_types_vec(&self, depth: usize) -> Option<Vec<ValueType>> {
        self.label_types(depth).map(|t| t.to_vec())
    }

    /// Get the current control frame's height.
    fn current_frame_height(&self) -> usize {
        self.ctrl_stack.last().map(|f| f.height).unwrap_or(0)
    }

    /// Check if the current frame is unreachable.
    fn is_current_unreachable(&self) -> bool {
        self.ctrl_stack
            .last()
            .map(|f| f.unreachable)
            .unwrap_or(false)
    }

    /// Resolve a named label (e.g., "$loop1") to a control stack depth.
    /// Returns the depth (0 = current frame) if found.
    pub(super) fn resolve_label_depth(&self, label: &str) -> Option<usize> {
        for (i, frame) in self.ctrl_stack.iter().rev().enumerate() {
            if let Some(ref name) = frame.label {
                if name == label {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Peek at the Nth value from the top of the stack (0 = top).
    /// Returns None if the stack doesn't have enough values.
    pub(super) fn peek(&self, n: usize) -> Option<&ValueType> {
        let len = self.val_stack.len();
        let height = self.current_frame_height();
        if len > height + n {
            self.val_stack.get(len - 1 - n)
        } else {
            None
        }
    }

    /// Get the current control stack depth.
    pub(super) fn ctrl_depth(&self) -> usize {
        self.ctrl_stack.len()
    }

    /// Pop the function's return types for a `return` instruction.
    /// Copies end_types from the function frame and pops them from the stack.
    pub(super) fn pop_function_return_types(&mut self, node: &Node, instr_name: &str) {
        if let Some(end_types) = self.ctrl_stack.first().map(|f| f.end_types.clone()) {
            self.pop_vals_for_instr(&end_types, node, instr_name);
        }
    }

    /// Take all collected diagnostics out of the checker.
    pub(super) fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbols::{TypeDef, TypeKind};

    fn make_symbols_with_types(types: Vec<TypeDef>) -> SymbolTable {
        let mut symbols = SymbolTable::default();
        for td in types {
            symbols.add_type(td);
        }
        symbols
    }

    fn struct_type(index: usize, name: Option<&str>, parent: Option<&str>) -> TypeDef {
        TypeDef {
            name: name.map(|s| s.to_string()),
            index,
            kind: TypeKind::Struct {
                fields: vec![(None, ValueType::I32, false)],
            },
            parent: parent.map(|s| s.to_string()),
            is_final: false,
            line: 0,
            range: None,
            rec_group_id: 0,
            rec_group_size: 1,
        }
    }

    fn array_type(index: usize, name: Option<&str>) -> TypeDef {
        TypeDef {
            name: name.map(|s| s.to_string()),
            index,
            kind: TypeKind::Array {
                element_type: ValueType::I32,
                mutable: false,
            },
            parent: None,
            is_final: false,
            line: 0,
            range: None,
            rec_group_id: 0,
            rec_group_size: 1,
        }
    }

    fn func_type(index: usize, name: Option<&str>) -> TypeDef {
        TypeDef {
            name: name.map(|s| s.to_string()),
            index,
            kind: TypeKind::Func {
                params: vec![],
                results: vec![],
            },
            parent: None,
            is_final: false,
            line: 0,
            range: None,
            rec_group_id: 0,
            rec_group_size: 1,
        }
    }

    #[test]
    fn test_is_type_subtype_self() {
        let symbols = make_symbols_with_types(vec![struct_type(0, Some("$a"), None)]);
        assert!(is_type_subtype(0, 0, &symbols));
    }

    #[test]
    fn test_is_type_subtype_direct_parent() {
        let symbols = make_symbols_with_types(vec![
            struct_type(0, Some("$parent"), None),
            struct_type(1, Some("$child"), Some("$parent")),
        ]);
        assert!(is_type_subtype(1, 0, &symbols));
        assert!(!is_type_subtype(0, 1, &symbols));
    }

    #[test]
    fn test_is_type_subtype_grandparent() {
        let symbols = make_symbols_with_types(vec![
            struct_type(0, Some("$grand"), None),
            struct_type(1, Some("$parent"), Some("$grand")),
            struct_type(2, Some("$child"), Some("$parent")),
        ]);
        assert!(is_type_subtype(2, 0, &symbols));
        assert!(is_type_subtype(2, 1, &symbols));
        assert!(!is_type_subtype(0, 2, &symbols));
    }

    #[test]
    fn test_is_type_subtype_unrelated() {
        let symbols = make_symbols_with_types(vec![
            struct_type(0, Some("$a"), None),
            struct_type(1, Some("$b"), None),
        ]);
        assert!(!is_type_subtype(0, 1, &symbols));
        assert!(!is_type_subtype(1, 0, &symbols));
    }

    #[test]
    fn test_concrete_struct_ref_subtype_of_structref() {
        let symbols = make_symbols_with_types(vec![struct_type(0, Some("$s"), None)]);
        assert!(types_compatible_with_symbols(
            &ValueType::Ref(0),
            &ValueType::Structref,
            &symbols
        ));
    }

    #[test]
    fn test_concrete_struct_ref_subtype_of_eqref() {
        let symbols = make_symbols_with_types(vec![struct_type(0, Some("$s"), None)]);
        assert!(types_compatible_with_symbols(
            &ValueType::Ref(0),
            &ValueType::Eqref,
            &symbols
        ));
    }

    #[test]
    fn test_concrete_struct_ref_subtype_of_anyref() {
        let symbols = make_symbols_with_types(vec![struct_type(0, Some("$s"), None)]);
        assert!(types_compatible_with_symbols(
            &ValueType::Ref(0),
            &ValueType::Anyref,
            &symbols
        ));
    }

    #[test]
    fn test_concrete_array_ref_subtype_of_arrayref() {
        let symbols = make_symbols_with_types(vec![array_type(0, Some("$a"))]);
        assert!(types_compatible_with_symbols(
            &ValueType::Ref(0),
            &ValueType::Arrayref,
            &symbols
        ));
    }

    #[test]
    fn test_concrete_ref_compatible_with_structref_placeholder() {
        // Structref is used as a placeholder for unresolved named ref types in the parser,
        // so Ref(n) <-> Structref is treated as compatible (see is_ref_subtype).
        // This means we can't distinguish "real" structref from unresolved named refs.
        let symbols = make_symbols_with_types(vec![func_type(0, Some("$f"))]);
        assert!(types_compatible_with_symbols(
            &ValueType::Ref(0),
            &ValueType::Structref,
            &symbols
        ));
    }

    #[test]
    fn test_ref_subtype_of_refnull_same_index() {
        let symbols = make_symbols_with_types(vec![struct_type(0, Some("$s"), None)]);
        assert!(types_compatible_with_symbols(
            &ValueType::Ref(0),
            &ValueType::RefNull(0),
            &symbols
        ));
    }

    #[test]
    fn test_refnull_not_subtype_of_ref() {
        let symbols = make_symbols_with_types(vec![struct_type(0, Some("$s"), None)]);
        assert!(!types_compatible_with_symbols(
            &ValueType::RefNull(0),
            &ValueType::Ref(0),
            &symbols
        ));
    }

    #[test]
    fn test_concrete_ref_parent_chain() {
        let symbols = make_symbols_with_types(vec![
            struct_type(0, Some("$parent"), None),
            struct_type(1, Some("$child"), Some("$parent")),
        ]);
        assert!(types_compatible_with_symbols(
            &ValueType::Ref(1),
            &ValueType::Ref(0),
            &symbols
        ));
        assert!(types_compatible_with_symbols(
            &ValueType::Ref(1),
            &ValueType::RefNull(0),
            &symbols
        ));
    }
}
