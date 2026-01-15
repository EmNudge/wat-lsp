use std::collections::HashMap;

use crate::core::types::Range;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
    I32,
    I64,
    F32,
    F64,
    V128,
    I8,
    I16,
    Funcref,
    Externref,
    Structref,
    Arrayref,
    I31ref,
    Anyref,
    Eqref,
    Nullref,
    NullFuncref,
    NullExternref,
    Ref(u32),     // Typed reference to a type index
    RefNull(u32), // Nullable typed reference
    Unknown,
}

impl std::fmt::Display for ValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueType::I32 => write!(f, "i32"),
            ValueType::I64 => write!(f, "i64"),
            ValueType::F32 => write!(f, "f32"),
            ValueType::F64 => write!(f, "f64"),
            ValueType::V128 => write!(f, "v128"),
            ValueType::I8 => write!(f, "i8"),
            ValueType::I16 => write!(f, "i16"),
            ValueType::Funcref => write!(f, "funcref"),
            ValueType::Externref => write!(f, "externref"),
            ValueType::Structref => write!(f, "structref"),
            ValueType::Arrayref => write!(f, "arrayref"),
            ValueType::I31ref => write!(f, "i31ref"),
            ValueType::Anyref => write!(f, "anyref"),
            ValueType::Eqref => write!(f, "eqref"),
            ValueType::Nullref => write!(f, "nullref"),
            ValueType::NullFuncref => write!(f, "nullfuncref"),
            ValueType::NullExternref => write!(f, "nullexternref"),
            ValueType::Ref(idx) => write!(f, "(ref {})", idx),
            ValueType::RefNull(idx) => write!(f, "(ref null {})", idx),
            ValueType::Unknown => write!(f, "unknown"),
        }
    }
}

impl ValueType {
    /// Try to parse a string into a ValueType.
    /// Returns None for unrecognized type strings.
    /// This is the canonical type parsing function used by all parsers.
    pub fn try_parse(s: &str) -> Option<Self> {
        match s {
            "i32" => Some(ValueType::I32),
            "i64" => Some(ValueType::I64),
            "f32" => Some(ValueType::F32),
            "f64" => Some(ValueType::F64),
            "v128" => Some(ValueType::V128),
            "i8" => Some(ValueType::I8),
            "i16" => Some(ValueType::I16),
            "funcref" => Some(ValueType::Funcref),
            "externref" => Some(ValueType::Externref),
            "structref" => Some(ValueType::Structref),
            "arrayref" => Some(ValueType::Arrayref),
            "i31ref" => Some(ValueType::I31ref),
            "anyref" => Some(ValueType::Anyref),
            "eqref" => Some(ValueType::Eqref),
            "nullref" => Some(ValueType::Nullref),
            "nullfuncref" => Some(ValueType::NullFuncref),
            "nullexternref" => Some(ValueType::NullExternref),
            _ => None,
        }
    }

    /// Parse a string into a ValueType, returning Unknown for unrecognized strings.
    #[allow(dead_code)] // May be useful for future features
    pub fn parse(s: &str) -> Self {
        Self::try_parse(s).unwrap_or(ValueType::Unknown)
    }
}

/// Convert from wast crate's ValType to our ValueType.
/// Centralized conversion used by wast_parser.
impl From<&wast::core::ValType<'_>> for ValueType {
    fn from(val_type: &wast::core::ValType) -> Self {
        match val_type {
            wast::core::ValType::I32 => ValueType::I32,
            wast::core::ValType::I64 => ValueType::I64,
            wast::core::ValType::F32 => ValueType::F32,
            wast::core::ValType::F64 => ValueType::F64,
            wast::core::ValType::V128 => ValueType::V128,
            wast::core::ValType::Ref(ref_type) => match &ref_type.heap {
                wast::core::HeapType::Abstract { ty, .. } => match ty {
                    wast::core::AbstractHeapType::Func => ValueType::Funcref,
                    wast::core::AbstractHeapType::Extern => ValueType::Externref,
                    wast::core::AbstractHeapType::Struct => ValueType::Structref,
                    wast::core::AbstractHeapType::Array => ValueType::Arrayref,
                    wast::core::AbstractHeapType::I31 => ValueType::I31ref,
                    wast::core::AbstractHeapType::Any => ValueType::Anyref,
                    wast::core::AbstractHeapType::Eq => ValueType::Eqref,
                    wast::core::AbstractHeapType::None => ValueType::Nullref,
                    wast::core::AbstractHeapType::NoFunc => ValueType::NullFuncref,
                    wast::core::AbstractHeapType::NoExtern => ValueType::NullExternref,
                    _ => ValueType::Unknown,
                },
                wast::core::HeapType::Concrete(idx) => match idx {
                    wast::token::Index::Num(n, _) => {
                        if ref_type.nullable {
                            ValueType::RefNull(*n)
                        } else {
                            ValueType::Ref(*n)
                        }
                    }
                    wast::token::Index::Id(_) => ValueType::Unknown,
                },
                wast::core::HeapType::Exact(_) => ValueType::Unknown,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: Option<String>,
    pub var_type: ValueType,
    #[allow(dead_code)] // Useful for future validation features
    pub is_mutable: bool,
    #[allow(dead_code)] // Useful for constant folding optimization
    pub initial_value: Option<String>,
    #[allow(dead_code)] // Useful for go-to-definition
    pub index: usize,
    #[allow(dead_code)] // Useful for go-to-definition
    pub range: Option<Range>,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: Option<String>,
    pub param_type: ValueType,
    #[allow(dead_code)] // Useful for signature help with parameter positions
    pub index: usize,
    #[allow(dead_code)] // Useful for go-to-definition
    pub range: Option<Range>,
}

#[derive(Debug, Clone)]
pub struct BlockLabel {
    pub label: String,
    pub block_type: String, // "block", "loop", "if", "try", "try_table"
    pub line: u32,
    #[allow(dead_code)] // Useful for go-to-definition
    pub range: Option<Range>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Option<String>,
    #[allow(dead_code)] // Useful for numeric function references
    pub index: usize,
    pub parameters: Vec<Parameter>,
    pub results: Vec<ValueType>,
    pub locals: Vec<Variable>,
    pub blocks: Vec<BlockLabel>,
    pub line: u32,
    #[allow(dead_code)] // Useful for go-to-definition and range queries
    pub end_line: u32, // Line where function ends
    #[allow(dead_code)] // Useful for precise AST navigation
    pub start_byte: usize, // Byte offset where function starts
    #[allow(dead_code)] // Useful for precise AST navigation
    pub end_byte: usize, // Byte offset where function ends
    #[allow(dead_code)] // Useful for go-to-definition
    pub range: Option<Range>,
}

#[derive(Debug, Clone)]
pub struct Global {
    pub name: Option<String>,
    #[allow(dead_code)] // Useful for numeric global references
    pub index: usize,
    pub var_type: ValueType,
    pub is_mutable: bool,
    pub initial_value: Option<String>,
    #[allow(dead_code)] // Useful for go-to-definition
    pub line: u32,
    #[allow(dead_code)] // Useful for go-to-definition
    pub range: Option<Range>,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub name: Option<String>,
    #[allow(dead_code)] // Useful for numeric table references
    pub index: usize,
    pub ref_type: ValueType,
    pub limits: (u32, Option<u32>), // (min, max)
    #[allow(dead_code)] // Useful for go-to-definition
    pub line: u32,
    #[allow(dead_code)] // Useful for go-to-definition
    pub range: Option<Range>,
}

#[derive(Debug, Clone)]
pub struct Memory {
    pub name: Option<String>,
    #[allow(dead_code)] // Useful for numeric memory references
    pub index: usize,
    #[allow(dead_code)] // Useful for memory operations and diagnostics
    pub limits: (u64, Option<u64>), // (min, max) - u64 for memory64 support
    #[allow(dead_code)] // Useful for memory64 validation
    pub is_memory64: bool, // true if memory uses i64 address space
    #[allow(dead_code)] // Useful for atomic operation validation
    pub shared: bool, // true if memory is shared (for threads)
    #[allow(dead_code)] // Useful for go-to-definition
    pub line: u32,
    #[allow(dead_code)] // Useful for go-to-definition
    pub range: Option<Range>,
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    Func {
        params: Vec<ValueType>,
        results: Vec<ValueType>,
    },
    Struct {
        fields: Vec<(Option<String>, ValueType, bool)>, // (name, type, mutable)
    },
    Array {
        element_type: ValueType,
        mutable: bool,
    },
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: Option<String>,
    #[allow(dead_code)] // Useful for numeric type references
    pub index: usize,
    pub kind: TypeKind,
    #[allow(dead_code)] // Useful for subtype validation
    pub supertype: Option<u32>, // Parent type index for subtyping
    #[allow(dead_code)] // Useful for subtype validation
    pub is_final: bool, // Whether this type can be subtyped
    #[allow(dead_code)] // Useful for rec group handling
    pub rec_group_id: Option<usize>, // Which rec group this type belongs to
    #[allow(dead_code)] // Useful for go-to-definition
    pub line: u32,
    #[allow(dead_code)] // Useful for go-to-definition
    pub range: Option<Range>,
}

#[derive(Debug, Clone)]
pub struct Tag {
    pub name: Option<String>,
    #[allow(dead_code)] // Useful for numeric tag references
    pub index: usize,
    pub params: Vec<ValueType>,
    #[allow(dead_code)] // Useful for go-to-definition
    pub line: u32,
    #[allow(dead_code)] // Useful for go-to-definition
    pub range: Option<Range>,
}

#[derive(Debug, Clone)]
pub struct DataSegment {
    pub name: Option<String>,
    #[allow(dead_code)] // Useful for numeric data references
    pub index: usize,
    pub content: String,    // The string content (for display)
    pub byte_length: usize, // Length in bytes
    #[allow(dead_code)] // Useful for active segments
    pub is_passive: bool, // true for passive segments (those with names)
    #[allow(dead_code)] // Useful for go-to-definition
    pub line: u32,
    #[allow(dead_code)] // Useful for go-to-definition
    pub range: Option<Range>,
}

#[derive(Debug, Clone)]
pub struct ElemSegment {
    pub name: Option<String>,
    #[allow(dead_code)] // Useful for numeric elem references
    pub index: usize,
    pub func_names: Vec<String>, // Function names/indices in this elem
    #[allow(dead_code)] // Useful for active segments
    pub table_name: Option<String>, // Target table if specified
    #[allow(dead_code)] // Useful for go-to-definition
    pub line: u32,
    #[allow(dead_code)] // Useful for go-to-definition
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    pub functions: Vec<Function>,
    pub globals: Vec<Global>,
    pub tables: Vec<Table>,
    pub memories: Vec<Memory>,
    pub types: Vec<TypeDef>,
    pub tags: Vec<Tag>,
    pub data_segments: Vec<DataSegment>,
    pub elem_segments: Vec<ElemSegment>,

    // Maps for quick lookup by name
    pub function_map: HashMap<String, usize>,
    pub global_map: HashMap<String, usize>,
    pub table_map: HashMap<String, usize>,
    pub memory_map: HashMap<String, usize>,
    pub type_map: HashMap<String, usize>,
    pub tag_map: HashMap<String, usize>,
    pub data_map: HashMap<String, usize>,
    pub elem_map: HashMap<String, usize>,
}

/// Macro to generate add/get methods for symbol types.
/// Eliminates boilerplate for the 8 symbol type method triplets.
macro_rules! impl_symbol_accessors {
    ($add_name:ident, $get_by_name:ident, $get_by_index:ident,
     $type:ty, $vec:ident, $map:ident) => {
        pub fn $add_name(&mut self, item: $type) {
            let index = self.$vec.len();
            if let Some(ref name) = item.name {
                self.$map.insert(name.clone(), index);
            }
            self.$vec.push(item);
        }

        pub fn $get_by_name(&self, name: &str) -> Option<&$type> {
            self.$map.get(name).and_then(|&idx| self.$vec.get(idx))
        }

        pub fn $get_by_index(&self, index: usize) -> Option<&$type> {
            self.$vec.get(index)
        }
    };
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    impl_symbol_accessors!(
        add_function,
        get_function_by_name,
        get_function_by_index,
        Function,
        functions,
        function_map
    );
    impl_symbol_accessors!(
        add_global,
        get_global_by_name,
        get_global_by_index,
        Global,
        globals,
        global_map
    );
    impl_symbol_accessors!(
        add_table,
        get_table_by_name,
        get_table_by_index,
        Table,
        tables,
        table_map
    );
    impl_symbol_accessors!(
        add_memory,
        get_memory_by_name,
        get_memory_by_index,
        Memory,
        memories,
        memory_map
    );
    impl_symbol_accessors!(
        add_type,
        get_type_by_name,
        get_type_by_index,
        TypeDef,
        types,
        type_map
    );
    impl_symbol_accessors!(
        add_tag,
        get_tag_by_name,
        get_tag_by_index,
        Tag,
        tags,
        tag_map
    );
    impl_symbol_accessors!(
        add_data,
        get_data_by_name,
        get_data_by_index,
        DataSegment,
        data_segments,
        data_map
    );
    impl_symbol_accessors!(
        add_elem,
        get_elem_by_name,
        get_elem_by_index,
        ElemSegment,
        elem_segments,
        elem_map
    );
}
