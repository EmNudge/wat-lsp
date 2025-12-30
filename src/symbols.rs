use std::collections::HashMap;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
    I32,
    I64,
    F32,
    F64,
    Funcref,
    Externref,
    Unknown,
}

impl ValueType {
    #[allow(dead_code)] // May be useful for future features
    pub fn parse(s: &str) -> Self {
        match s {
            "i32" => ValueType::I32,
            "i64" => ValueType::I64,
            "f32" => ValueType::F32,
            "f64" => ValueType::F64,
            "funcref" => ValueType::Funcref,
            "externref" => ValueType::Externref,
            _ => ValueType::Unknown,
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            ValueType::I32 => "i32",
            ValueType::I64 => "i64",
            ValueType::F32 => "f32",
            ValueType::F64 => "f64",
            ValueType::Funcref => "funcref",
            ValueType::Externref => "externref",
            ValueType::Unknown => "unknown",
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
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: Option<String>,
    pub param_type: ValueType,
    #[allow(dead_code)] // Useful for signature help with parameter positions
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct BlockLabel {
    pub label: String,
    pub block_type: String, // "block", "loop", "if"
    pub line: u32,
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
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: Option<String>,
    #[allow(dead_code)] // Useful for numeric type references
    pub index: usize,
    pub parameters: Vec<ValueType>,
    pub results: Vec<ValueType>,
    #[allow(dead_code)] // Useful for go-to-definition
    pub line: u32,
}

#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    pub functions: Vec<Function>,
    pub globals: Vec<Global>,
    pub tables: Vec<Table>,
    pub types: Vec<TypeDef>,

    // Maps for quick lookup by name
    pub function_map: HashMap<String, usize>,
    pub global_map: HashMap<String, usize>,
    pub table_map: HashMap<String, usize>,
    pub type_map: HashMap<String, usize>,
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

    pub fn get_type_by_name(&self, name: &str) -> Option<&TypeDef> {
        self.type_map.get(name).and_then(|&idx| self.types.get(idx))
    }
}
