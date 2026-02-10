//! Core type checker implementing the WebAssembly validation algorithm (spec §3.3).
//!
//! This module provides a `TypeChecker` that tracks typed values on a value stack
//! and control frames on a control stack. It replaces the untyped `StackState` to
//! detect both stack underflow AND type mismatches.

use crate::core::types::Diagnostic;
use crate::symbols::ValueType;
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
        // Ref(n) <: funcref (non-null func ref)
        (Ref(_), Funcref) => true,
        // RefNull(n) <: funcref (nullable func ref)
        (RefNull(_), Funcref) => true,
        _ => false,
    }
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
        let height = self.val_stack.len();
        self.ctrl_stack.push(CtrlFrame {
            opcode,
            start_types: start_types.clone(),
            end_types,
            height,
            unreachable: false,
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
