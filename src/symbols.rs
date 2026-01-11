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
    #[allow(dead_code)] // May be useful for future features
    pub fn parse(s: &str) -> Self {
        match s {
            "i32" => ValueType::I32,
            "i64" => ValueType::I64,
            "f32" => ValueType::F32,
            "f64" => ValueType::F64,
            "v128" => ValueType::V128,
            "i8" => ValueType::I8,
            "i16" => ValueType::I16,
            "funcref" => ValueType::Funcref,
            "externref" => ValueType::Externref,
            "structref" => ValueType::Structref,
            "arrayref" => ValueType::Arrayref,
            "i31ref" => ValueType::I31ref,
            "anyref" => ValueType::Anyref,
            "eqref" => ValueType::Eqref,
            "nullref" => ValueType::Nullref,
            "nullfuncref" => ValueType::NullFuncref,
            "nullexternref" => ValueType::NullExternref,
            _ => ValueType::Unknown,
        }
    }

    pub fn to_str(&self) -> String {
        match self {
            ValueType::I32 => "i32".to_string(),
            ValueType::I64 => "i64".to_string(),
            ValueType::F32 => "f32".to_string(),
            ValueType::F64 => "f64".to_string(),
            ValueType::V128 => "v128".to_string(),
            ValueType::I8 => "i8".to_string(),
            ValueType::I16 => "i16".to_string(),
            ValueType::Funcref => "funcref".to_string(),
            ValueType::Externref => "externref".to_string(),
            ValueType::Structref => "structref".to_string(),
            ValueType::Arrayref => "arrayref".to_string(),
            ValueType::I31ref => "i31ref".to_string(),
            ValueType::Anyref => "anyref".to_string(),
            ValueType::Eqref => "eqref".to_string(),
            ValueType::Nullref => "nullref".to_string(),
            ValueType::NullFuncref => "nullfuncref".to_string(),
            ValueType::NullExternref => "nullexternref".to_string(),
            ValueType::Ref(idx) => format!("(ref {})", idx),
            ValueType::RefNull(idx) => format!("(ref null {})", idx),
            ValueType::Unknown => "unknown".to_string(),
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
    pub limits: (u32, Option<u32>), // (min, max)
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

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_function(&mut self, func: Function) {
        let index = self.functions.len();
        if let Some(ref name) = func.name {
            self.function_map.insert(name.clone(), index);
        }
        self.functions.push(func);
    }

    pub fn add_global(&mut self, global: Global) {
        let index = self.globals.len();
        if let Some(ref name) = global.name {
            self.global_map.insert(name.clone(), index);
        }
        self.globals.push(global);
    }

    pub fn add_table(&mut self, table: Table) {
        let index = self.tables.len();
        if let Some(ref name) = table.name {
            self.table_map.insert(name.clone(), index);
        }
        self.tables.push(table);
    }

    pub fn add_memory(&mut self, memory: Memory) {
        let index = self.memories.len();
        if let Some(ref name) = memory.name {
            self.memory_map.insert(name.clone(), index);
        }
        self.memories.push(memory);
    }

    pub fn add_type(&mut self, type_def: TypeDef) {
        let index = self.types.len();
        if let Some(ref name) = type_def.name {
            self.type_map.insert(name.clone(), index);
        }
        self.types.push(type_def);
    }

    pub fn get_function_by_name(&self, name: &str) -> Option<&Function> {
        self.function_map
            .get(name)
            .and_then(|&idx| self.functions.get(idx))
    }

    pub fn get_function_by_index(&self, index: usize) -> Option<&Function> {
        self.functions.get(index)
    }

    pub fn get_global_by_name(&self, name: &str) -> Option<&Global> {
        self.global_map
            .get(name)
            .and_then(|&idx| self.globals.get(idx))
    }

    pub fn get_global_by_index(&self, index: usize) -> Option<&Global> {
        self.globals.get(index)
    }

    pub fn get_table_by_name(&self, name: &str) -> Option<&Table> {
        self.table_map
            .get(name)
            .and_then(|&idx| self.tables.get(idx))
    }

    pub fn get_table_by_index(&self, index: usize) -> Option<&Table> {
        self.tables.get(index)
    }

    pub fn get_memory_by_name(&self, name: &str) -> Option<&Memory> {
        self.memory_map
            .get(name)
            .and_then(|&idx| self.memories.get(idx))
    }

    pub fn get_memory_by_index(&self, index: usize) -> Option<&Memory> {
        self.memories.get(index)
    }

    pub fn get_type_by_name(&self, name: &str) -> Option<&TypeDef> {
        self.type_map.get(name).and_then(|&idx| self.types.get(idx))
    }

    pub fn get_type_by_index(&self, index: usize) -> Option<&TypeDef> {
        self.types.get(index)
    }

    pub fn add_tag(&mut self, tag: Tag) {
        let index = self.tags.len();
        if let Some(ref name) = tag.name {
            self.tag_map.insert(name.clone(), index);
        }
        self.tags.push(tag);
    }

    pub fn get_tag_by_name(&self, name: &str) -> Option<&Tag> {
        self.tag_map.get(name).and_then(|&idx| self.tags.get(idx))
    }

    pub fn get_tag_by_index(&self, index: usize) -> Option<&Tag> {
        self.tags.get(index)
    }

    pub fn add_data(&mut self, data: DataSegment) {
        let index = self.data_segments.len();
        if let Some(ref name) = data.name {
            self.data_map.insert(name.clone(), index);
        }
        self.data_segments.push(data);
    }

    pub fn get_data_by_name(&self, name: &str) -> Option<&DataSegment> {
        self.data_map
            .get(name)
            .and_then(|&idx| self.data_segments.get(idx))
    }

    pub fn get_data_by_index(&self, index: usize) -> Option<&DataSegment> {
        self.data_segments.get(index)
    }

    pub fn add_elem(&mut self, elem: ElemSegment) {
        let index = self.elem_segments.len();
        if let Some(ref name) = elem.name {
            self.elem_map.insert(name.clone(), index);
        }
        self.elem_segments.push(elem);
    }

    pub fn get_elem_by_name(&self, name: &str) -> Option<&ElemSegment> {
        self.elem_map
            .get(name)
            .and_then(|&idx| self.elem_segments.get(idx))
    }

    pub fn get_elem_by_index(&self, index: usize) -> Option<&ElemSegment> {
        self.elem_segments.get(index)
    }
}
