//! Stack state tracking for semantic validation.
//!
//! This module provides the StackState struct used to track value types
//! during instruction list traversal for stack underflow and type checking.

use crate::symbols::ValueType;

/// Tracks stack state during instruction list traversal for stack underflow and type checking
#[derive(Debug, Clone)]
pub struct StackState {
    /// Stack of value types (bottom to top)
    types: Vec<ValueType>,
    /// True after unconditional branches or unreachable - subsequent code is dead
    /// and we shouldn't report errors for it
    uncertain: bool,
}

impl StackState {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            uncertain: false,
        }
    }

    /// Try to consume n values from the stack
    /// Returns Err with the actual available count if underflow occurs
    pub fn consume(&mut self, n: usize) -> Result<(), usize> {
        if self.uncertain {
            // After unconditional control flow, we can't know the stack state
            return Ok(());
        }
        if self.types.len() >= n {
            self.types.truncate(self.types.len() - n);
            Ok(())
        } else {
            let available = self.types.len();
            self.types.clear();
            Err(available)
        }
    }

    /// Push types onto the stack
    pub fn produce(&mut self, types: Vec<ValueType>) {
        if !self.uncertain {
            self.types.extend(types);
        }
    }

    /// Mark stack as uncertain (after unconditional branch/unreachable)
    pub fn mark_uncertain(&mut self) {
        self.uncertain = true;
    }

    /// Check if stack is in uncertain state
    pub fn is_uncertain(&self) -> bool {
        self.uncertain
    }

    /// Get the current stack types (for final validation)
    pub fn get_types(&self) -> &[ValueType] {
        &self.types
    }
}

impl Default for StackState {
    fn default() -> Self {
        Self::new()
    }
}
