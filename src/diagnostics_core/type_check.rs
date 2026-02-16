//! Core type checker implementing the WebAssembly validation algorithm (spec §3.3).
//!
//! This module provides a `TypeChecker` that tracks typed values on a value stack
//! and control frames on a control stack. It replaces the untyped `StackState` to
//! detect both stack underflow AND type mismatches.

use crate::core::types::Diagnostic;
use crate::symbols::{SymbolTable, TypeKind, ValueType};
use crate::utils::node_to_range;

// Use the appropriate tree-sitter types based on feature
#[cfg(feature = "native")]
use tree_sitter::Node;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use crate::ts_facade::Node;

/// What kind of control frame this is. Affects label_types resolution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CtrlOpcode {
    Function,
    Block,
    Loop,
    If,
    TryTable,
}

/// A control frame on the control stack, per spec §3.3 appendix.
#[derive(Debug, Clone)]
pub struct CtrlFrame {
    pub opcode: CtrlOpcode,
    /// Block parameter types (consumed from outer stack on entry)
    pub start_types: Vec<ValueType>,
    /// Block result types (left on stack on exit)
    pub end_types: Vec<ValueType>,
    /// Value stack height when this frame was entered
    pub height: usize,
    /// True after unreachable/br/return — polymorphic stack bottom
    pub unreachable: bool,
    /// Optional label name (e.g., "$loop1") for named branch resolution
    pub label: Option<String>,
}

/// Core type checker state machine implementing Wasm spec validation.
#[derive(Default)]
pub struct TypeChecker {
    /// Value stack of types
    val_stack: Vec<ValueType>,
    /// Control frame stack
    ctrl_stack: Vec<CtrlFrame>,
    /// Collected diagnostics
    pub diagnostics: Vec<Diagnostic>,
}

/// Check if two types are compatible for validation purposes.
/// Unknown matches anything (polymorphic). Otherwise types must match exactly,
/// with basic reference subtyping.
pub fn types_compatible(actual: &ValueType, expected: &ValueType) -> bool {
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
        // nullref is bottom for internal ref hierarchy
        (Nullref, Anyref | Eqref | I31ref | Structref | Arrayref) => true,
        // nullfuncref is bottom for func hierarchy
        (NullFuncref, Funcref) => true,
        // nullexternref is bottom for extern hierarchy
        (NullExternref, Externref) => true,
        // i31ref, structref, arrayref <: eqref <: anyref
        (I31ref | Structref | Arrayref | Eqref, Anyref) => true,
        (I31ref | Structref | Arrayref, Eqref) => true,
        // Ref(n) / RefNull(n) — without symbol table, assume func hierarchy only
        (Ref(_), Funcref) => true,
        (RefNull(_), Funcref) => true,
        // Non-null <: nullable for same index
        (Ref(a), RefNull(b)) if a == b => true,
        _ => false,
    }
}

/// Check type compatibility with access to the symbol table for concrete GC type resolution.
///
/// This extends `types_compatible()` with knowledge of concrete type definitions,
/// enabling validation of `Ref(n)` / `RefNull(n)` against abstract supertypes
/// (structref, arrayref, eqref, anyref) and concrete parent chains.
pub fn types_compatible_with_symbols(
    actual: &ValueType,
    expected: &ValueType,
    symbols: &SymbolTable,
) -> bool {
    // Fast path: check without symbols first
    if types_compatible(actual, expected) {
        return true;
    }
    // Concrete ref subtyping requires symbols
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
    // In the Wasm spec, abstract types like structref/arrayref/funcref are nullable,
    // so both Ref(n) and RefNull(n) are subtypes.
    let type_def = match symbols.get_type_by_index(sub_idx as usize) {
        Some(td) => td,
        None => return false,
    };

    matches!(
        (&type_def.kind, sup),
        (TypeKind::Struct { .. }, Structref | Eqref | Anyref)
            | (TypeKind::Array { .. }, Arrayref | Eqref | Anyref)
            | (TypeKind::Func { .. }, Funcref)
    )
}

/// Check if `child_idx` is a declared subtype of `parent_idx` via the parent chain.
///
/// Walks up the `TypeDef.parent` chain from child, checking if parent_idx is encountered.
/// Max depth of 64 prevents infinite loops from cyclic declarations.
fn is_type_subtype(child_idx: usize, parent_idx: usize, symbols: &SymbolTable) -> bool {
    if child_idx == parent_idx {
        return true;
    }

    let mut current_idx = child_idx;
    for _ in 0..64 {
        let type_def = match symbols.get_type_by_index(current_idx) {
            Some(td) => td,
            None => return false,
        };

        let parent_ref = match &type_def.parent {
            Some(p) => p.clone(),
            None => return false,
        };

        // Resolve parent reference to index
        let resolved_idx = if parent_ref.starts_with('$') {
            match symbols.get_type_by_name(&parent_ref) {
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
        current_idx = resolved_idx;
    }

    false
}

impl TypeChecker {
    /// Create a new TypeChecker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a value type onto the value stack.
    pub fn push_val(&mut self, ty: ValueType) {
        self.val_stack.push(ty);
    }

    /// Pop a value from the value stack.
    /// If stack is at or below frame height AND frame is unreachable, returns Unknown.
    /// If stack underflows (below frame height, not unreachable), returns None.
    pub fn pop_val(&mut self) -> Option<ValueType> {
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
    pub fn pop_expect(&mut self, expected: &ValueType, node: &Node) -> Option<ValueType> {
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
    pub fn pop_expect_silent(&mut self, expected: &ValueType) -> (Option<ValueType>, bool) {
        let actual = self.pop_val();
        match actual {
            Some(ref ty) => {
                let matched = types_compatible(ty, expected);
                (actual, matched)
            }
            None => (None, false),
        }
    }

    /// Push multiple values onto the stack.
    pub fn push_vals(&mut self, types: &[ValueType]) {
        for ty in types {
            self.push_val(ty.clone());
        }
    }

    /// Pop multiple values and check types (in reverse order, as per spec).
    /// Returns true if all popped successfully and matched.
    /// Does NOT emit diagnostics — callers should use `pop_vals_for_instr` for
    /// instruction-level checks with proper diagnostic messages.
    pub fn pop_vals(&mut self, expected: &[ValueType], node: &Node) -> bool {
        self.pop_vals_inner(expected, node, None)
    }

    /// Pop multiple values for a named instruction.
    /// On underflow, emits a "Stack underflow" diagnostic with instruction name.
    /// On type mismatch, emits a "type mismatch" diagnostic.
    pub fn pop_vals_for_instr(
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

    /// Pop multiple values without type checking (just count).
    /// Returns the number actually popped (may be less than requested on underflow).
    pub fn pop_n(&mut self, n: usize) -> usize {
        let height = self.current_frame_height();
        let available = self.val_stack.len() - height;
        let to_pop = n.min(available);
        // If unreachable, we can always pop
        if self.is_current_unreachable() {
            let actual_pop = n.min(self.val_stack.len().saturating_sub(height));
            self.val_stack
                .truncate(self.val_stack.len().saturating_sub(actual_pop));
            return n; // pretend we popped all
        }
        self.val_stack.truncate(self.val_stack.len() - to_pop);
        to_pop
    }

    /// Enter a new control frame (block, loop, if, function).
    pub fn push_ctrl(
        &mut self,
        opcode: CtrlOpcode,
        start_types: Vec<ValueType>,
        end_types: Vec<ValueType>,
    ) {
        self.push_ctrl_labeled(opcode, start_types, end_types, None);
    }

    /// Enter a new control frame with an optional label name for named branch resolution.
    pub fn push_ctrl_labeled(
        &mut self,
        opcode: CtrlOpcode,
        start_types: Vec<ValueType>,
        end_types: Vec<ValueType>,
        label: Option<String>,
    ) {
        let height = self.val_stack.len();
        self.ctrl_stack.push(CtrlFrame {
            opcode,
            start_types: start_types.clone(),
            end_types,
            height,
            unreachable: false,
            label,
        });
        // Push start_types onto val_stack (block params become initial stack)
        self.push_vals(&start_types);
    }

    /// Exit a control frame. Validates that end_types match the stack.
    /// Returns the popped frame, or None if ctrl_stack is empty.
    pub fn pop_ctrl(&mut self, node: &Node) -> Option<CtrlFrame> {
        if self.ctrl_stack.is_empty() {
            return None;
        }
        let frame = self.ctrl_stack.last().unwrap();
        let end_types = frame.end_types.clone();
        let height = frame.height;

        // Pop expected end types from the stack
        self.pop_vals(&end_types, node);

        // Check that we're back to frame height
        if self.val_stack.len() != height && !self.is_current_unreachable() {
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
        }

        // Restore val_stack to frame height
        self.val_stack.truncate(height);

        let frame = self.ctrl_stack.pop().unwrap();

        // Push end_types onto outer stack
        self.push_vals(&frame.end_types);

        Some(frame)
    }

    /// Handle the else transition in an if block.
    /// Validates the then branch produced end_types, resets stack to frame height,
    /// and pushes start_types for the else branch.
    pub fn else_transition(&mut self, node: &Node) {
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
    pub fn mark_unreachable(&mut self) {
        if let Some(frame) = self.ctrl_stack.last_mut() {
            self.val_stack.truncate(frame.height);
            frame.unreachable = true;
        }
    }

    /// Get the label types for a frame at the given depth.
    /// For loops, label types are start_types (br restarts with params).
    /// For everything else, label types are end_types.
    pub fn label_types(&self, depth: usize) -> Option<&[ValueType]> {
        let idx = self.ctrl_stack.len().checked_sub(1 + depth)?;
        let frame = &self.ctrl_stack[idx];
        Some(match frame.opcode {
            CtrlOpcode::Loop => &frame.start_types,
            _ => &frame.end_types,
        })
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
    pub fn resolve_label_depth(&self, label: &str) -> Option<usize> {
        for (i, frame) in self.ctrl_stack.iter().rev().enumerate() {
            if let Some(ref name) = frame.label {
                if name == label {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Get the current stack depth above the current frame height.
    pub fn stack_depth(&self) -> usize {
        self.val_stack
            .len()
            .saturating_sub(self.current_frame_height())
    }

    /// Get the current control stack depth.
    pub fn ctrl_depth(&self) -> usize {
        self.ctrl_stack.len()
    }

    /// Peek at a frame at the given depth (0 = current).
    pub fn get_frame(&self, depth: usize) -> Option<&CtrlFrame> {
        let idx = self.ctrl_stack.len().checked_sub(1 + depth)?;
        Some(&self.ctrl_stack[idx])
    }

    /// Get the function-level frame (bottom of control stack).
    pub fn function_frame(&self) -> Option<&CtrlFrame> {
        self.ctrl_stack.first()
    }

    /// Take all collected diagnostics out of the checker.
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
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
    fn test_concrete_func_ref_not_subtype_of_structref() {
        let symbols = make_symbols_with_types(vec![func_type(0, Some("$f"))]);
        assert!(!types_compatible_with_symbols(
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
