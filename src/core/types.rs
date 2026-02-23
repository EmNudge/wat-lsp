//! Protocol-independent types for positions and ranges.
//!
//! These types have no dependencies on tower-lsp or wasm-bindgen,
//! allowing them to be used in both native and WASM builds.

#[cfg(feature = "native")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "native")]
use tower_lsp::lsp_types as lsp;

/// Completion item kind (matches LSP CompletionItemKind values)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "native", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum CompletionItemKind {
    Text = 1,
    Method = 2,
    Function = 3,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Property = 10,
    Unit = 11,
    Value = 12,
    Enum = 13,
    Keyword = 14,
    Snippet = 15,
    Color = 16,
    File = 17,
    Reference = 18,
    Folder = 19,
    EnumMember = 20,
    Constant = 21,
    Struct = 22,
    Event = 23,
    Operator = 24,
    TypeParameter = 25,
}

/// Insert text format for completion items
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "native", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum InsertTextFormat {
    PlainText = 1,
    Snippet = 2,
}

/// A completion item for code suggestions
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "native", derive(Serialize, Deserialize))]
pub struct CompletionItem {
    /// The label shown in the completion list
    pub label: String,
    /// The kind of completion item (function, variable, etc.)
    pub kind: Option<CompletionItemKind>,
    /// A short description shown next to the label
    pub detail: Option<String>,
    /// The text to insert when this item is selected
    pub insert_text: Option<String>,
    /// The format of the insert text (plain or snippet)
    pub insert_text_format: Option<InsertTextFormat>,
    /// Longer documentation about this item
    pub documentation: Option<String>,
}

impl CompletionItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Default::default()
        }
    }

    pub fn with_kind(mut self, kind: CompletionItemKind) -> Self {
        self.kind = Some(kind);
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_insert_text(mut self, text: impl Into<String>) -> Self {
        self.insert_text = Some(text.into());
        self
    }

    pub fn with_insert_text_format(mut self, format: InsertTextFormat) -> Self {
        self.insert_text_format = Some(format);
        self
    }

    pub fn with_documentation(mut self, doc: impl Into<String>) -> Self {
        self.documentation = Some(doc.into());
        self
    }
}

/// A position in a text document (0-indexed line and character)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "native", derive(Serialize, Deserialize))]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

/// A range in a text document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "native", derive(Serialize, Deserialize))]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// Create a range from line/column coordinates
    pub fn from_coords(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Self {
        Self {
            start: Position::new(start_line, start_char),
            end: Position::new(end_line, end_char),
        }
    }
}

/// Hover result containing markdown content
#[derive(Debug, Clone)]
#[cfg_attr(feature = "native", derive(Serialize, Deserialize))]
pub struct HoverResult {
    /// Markdown-formatted content to display
    pub contents: String,
    /// Optional range to highlight
    pub range: Option<Range>,
}

impl HoverResult {
    pub fn new(contents: String) -> Self {
        Self {
            contents,
            range: None,
        }
    }
}

/// Definition result - the range where a symbol is defined
pub type DefinitionResult = Option<Range>;

/// References result - list of ranges where a symbol is referenced
pub type ReferencesResult = Vec<Range>;

/// Diagnostic severity (matches LSP DiagnosticSeverity values)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "native", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum DiagnosticSeverity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

/// A diagnostic message (error, warning, etc.)
#[derive(Debug, Clone)]
#[cfg_attr(feature = "native", derive(Serialize, Deserialize))]
pub struct Diagnostic {
    /// The range where the diagnostic applies
    pub range: Range,
    /// The severity of the diagnostic
    pub severity: DiagnosticSeverity,
    /// The diagnostic message
    pub message: String,
    /// Optional diagnostic code
    pub code: Option<String>,
    /// Optional source identifier (e.g., "wat-lsp")
    pub source: Option<String>,
}

impl Diagnostic {
    /// Create a new error diagnostic
    pub fn error(range: Range, message: impl Into<String>) -> Self {
        Self {
            range,
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            code: None,
            source: Some("wat-lsp".to_string()),
        }
    }

    /// Create a new warning diagnostic
    pub fn warning(range: Range, message: impl Into<String>) -> Self {
        Self {
            range,
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
            code: None,
            source: Some("wat-lsp".to_string()),
        }
    }

    /// Create a new hint diagnostic
    pub fn hint(range: Range, message: impl Into<String>) -> Self {
        Self {
            range,
            severity: DiagnosticSeverity::Hint,
            message: message.into(),
            code: None,
            source: Some("wat-lsp".to_string()),
        }
    }

    /// Set the diagnostic code
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

/// Symbol kind for document symbols (matches LSP SymbolKind values)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SymbolKindCore {
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Array = 18,
    Object = 19,
    Key = 20,
    Event = 24,
    Interface = 11,
}

/// A protocol-independent document symbol for outline views
#[derive(Debug, Clone)]
pub struct DocumentSymbolInfo {
    pub name: std::string::String,
    pub detail: Option<std::string::String>,
    pub kind: SymbolKindCore,
    pub range: Range,
    pub children: Option<Vec<DocumentSymbolInfo>>,
}

// Conversion implementations for native builds (tower-lsp types)
#[cfg(feature = "native")]
impl From<lsp::Position> for Position {
    fn from(p: lsp::Position) -> Self {
        Self {
            line: p.line,
            character: p.character,
        }
    }
}

#[cfg(feature = "native")]
impl From<Position> for lsp::Position {
    fn from(p: Position) -> Self {
        Self {
            line: p.line,
            character: p.character,
        }
    }
}

#[cfg(feature = "native")]
impl From<lsp::Range> for Range {
    fn from(r: lsp::Range) -> Self {
        Self {
            start: r.start.into(),
            end: r.end.into(),
        }
    }
}

#[cfg(feature = "native")]
impl From<Range> for lsp::Range {
    fn from(r: Range) -> Self {
        Self {
            start: r.start.into(),
            end: r.end.into(),
        }
    }
}

#[cfg(feature = "native")]
impl From<CompletionItemKind> for lsp::CompletionItemKind {
    fn from(kind: CompletionItemKind) -> Self {
        match kind {
            CompletionItemKind::Text => lsp::CompletionItemKind::TEXT,
            CompletionItemKind::Method => lsp::CompletionItemKind::METHOD,
            CompletionItemKind::Function => lsp::CompletionItemKind::FUNCTION,
            CompletionItemKind::Constructor => lsp::CompletionItemKind::CONSTRUCTOR,
            CompletionItemKind::Field => lsp::CompletionItemKind::FIELD,
            CompletionItemKind::Variable => lsp::CompletionItemKind::VARIABLE,
            CompletionItemKind::Class => lsp::CompletionItemKind::CLASS,
            CompletionItemKind::Interface => lsp::CompletionItemKind::INTERFACE,
            CompletionItemKind::Module => lsp::CompletionItemKind::MODULE,
            CompletionItemKind::Property => lsp::CompletionItemKind::PROPERTY,
            CompletionItemKind::Unit => lsp::CompletionItemKind::UNIT,
            CompletionItemKind::Value => lsp::CompletionItemKind::VALUE,
            CompletionItemKind::Enum => lsp::CompletionItemKind::ENUM,
            CompletionItemKind::Keyword => lsp::CompletionItemKind::KEYWORD,
            CompletionItemKind::Snippet => lsp::CompletionItemKind::SNIPPET,
            CompletionItemKind::Color => lsp::CompletionItemKind::COLOR,
            CompletionItemKind::File => lsp::CompletionItemKind::FILE,
            CompletionItemKind::Reference => lsp::CompletionItemKind::REFERENCE,
            CompletionItemKind::Folder => lsp::CompletionItemKind::FOLDER,
            CompletionItemKind::EnumMember => lsp::CompletionItemKind::ENUM_MEMBER,
            CompletionItemKind::Constant => lsp::CompletionItemKind::CONSTANT,
            CompletionItemKind::Struct => lsp::CompletionItemKind::STRUCT,
            CompletionItemKind::Event => lsp::CompletionItemKind::EVENT,
            CompletionItemKind::Operator => lsp::CompletionItemKind::OPERATOR,
            CompletionItemKind::TypeParameter => lsp::CompletionItemKind::TYPE_PARAMETER,
        }
    }
}

#[cfg(feature = "native")]
impl From<InsertTextFormat> for lsp::InsertTextFormat {
    fn from(format: InsertTextFormat) -> Self {
        match format {
            InsertTextFormat::PlainText => lsp::InsertTextFormat::PLAIN_TEXT,
            InsertTextFormat::Snippet => lsp::InsertTextFormat::SNIPPET,
        }
    }
}

#[cfg(feature = "native")]
impl From<CompletionItem> for lsp::CompletionItem {
    fn from(item: CompletionItem) -> Self {
        lsp::CompletionItem {
            label: item.label,
            kind: item.kind.map(|k| k.into()),
            detail: item.detail,
            insert_text: item.insert_text,
            insert_text_format: item.insert_text_format.map(|f| f.into()),
            documentation: item.documentation.map(lsp::Documentation::String),
            ..Default::default()
        }
    }
}

#[cfg(feature = "native")]
impl From<DiagnosticSeverity> for lsp::DiagnosticSeverity {
    fn from(severity: DiagnosticSeverity) -> Self {
        match severity {
            DiagnosticSeverity::Error => lsp::DiagnosticSeverity::ERROR,
            DiagnosticSeverity::Warning => lsp::DiagnosticSeverity::WARNING,
            DiagnosticSeverity::Information => lsp::DiagnosticSeverity::INFORMATION,
            DiagnosticSeverity::Hint => lsp::DiagnosticSeverity::HINT,
        }
    }
}

#[cfg(feature = "native")]
impl From<Diagnostic> for lsp::Diagnostic {
    fn from(diag: Diagnostic) -> Self {
        lsp::Diagnostic {
            range: diag.range.into(),
            severity: Some(diag.severity.into()),
            code: diag.code.map(lsp::NumberOrString::String),
            code_description: None,
            source: diag.source,
            message: diag.message,
            related_information: None,
            tags: None,
            data: None,
        }
    }
}

#[cfg(feature = "native")]
impl From<SymbolKindCore> for lsp::SymbolKind {
    fn from(kind: SymbolKindCore) -> Self {
        match kind {
            SymbolKindCore::Function => lsp::SymbolKind::FUNCTION,
            SymbolKindCore::Variable => lsp::SymbolKind::VARIABLE,
            SymbolKindCore::Constant => lsp::SymbolKind::CONSTANT,
            SymbolKindCore::String => lsp::SymbolKind::STRING,
            SymbolKindCore::Array => lsp::SymbolKind::ARRAY,
            SymbolKindCore::Object => lsp::SymbolKind::OBJECT,
            SymbolKindCore::Key => lsp::SymbolKind::KEY,
            SymbolKindCore::Event => lsp::SymbolKind::EVENT,
            SymbolKindCore::Interface => lsp::SymbolKind::INTERFACE,
        }
    }
}

#[cfg(feature = "native")]
impl From<DocumentSymbolInfo> for lsp::DocumentSymbol {
    #[allow(deprecated)]
    fn from(sym: DocumentSymbolInfo) -> Self {
        lsp::DocumentSymbol {
            name: sym.name,
            detail: sym.detail,
            kind: sym.kind.into(),
            tags: None,
            deprecated: None,
            range: sym.range.into(),
            selection_range: sym.range.into(),
            children: sym
                .children
                .map(|c| c.into_iter().map(Into::into).collect()),
        }
    }
}
