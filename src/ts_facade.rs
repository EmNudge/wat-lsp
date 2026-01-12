//! Unified tree-sitter abstraction for both native and WASM builds.
//!
//! This module provides a platform-agnostic interface to tree-sitter.
//! - Native builds use `tree-sitter` crate directly
//! - WASM builds use `web-tree-sitter-sg` crate directly
//!
//! The abstraction provides unified types that work identically on both platforms.

// ============================================================================
// Native Implementation (using tree-sitter crate)
// ============================================================================

#[cfg(feature = "native")]
mod native {
    pub use tree_sitter::{
        InputEdit, Language, Parser, Point, Query, QueryError, Tree, TreeCursor,
    };

    // Re-export Node with the same API
    pub type Node<'a> = tree_sitter::Node<'a>;

    /// Get the WAT language
    pub fn wat_language() -> Language {
        crate::tree_sitter_bindings::wat_language()
    }

    /// Create a parser configured for WAT
    pub fn create_parser() -> Parser {
        crate::tree_sitter_bindings::create_parser()
    }
}

// ============================================================================
// WASM Implementation (using web-tree-sitter-sg crate)
// ============================================================================

#[cfg(feature = "wasm")]
mod wasm {
    use std::ops::Range;
    use wasm_bindgen::prelude::*;
    use web_tree_sitter_sg::{
        Language as WtsLanguage, Parser as WtsParser, Query as WtsQuery, SyntaxNode,
        Tree as WtsTree, TreeSitter,
    };

    // Include the tree-sitter-wat.wasm file at compile time
    const WAT_GRAMMAR_WASM: &[u8] =
        include_bytes!("../grammars/tree-sitter-wat/tree-sitter-wat.wasm");

    /// Initialize tree-sitter runtime (must be called before using parser)
    pub async fn init() -> Result<(), JsValue> {
        TreeSitter::init().await.map_err(|e| e.into())
    }

    /// Language wrapper
    #[derive(Clone)]
    pub struct Language(WtsLanguage);

    impl Language {
        /// Create a query from this language
        pub fn query(&self, source: &str) -> Result<Query, String> {
            let js_source = js_sys::JsString::from(source);
            self.0
                .query(&js_source)
                .map(Query)
                .map_err(|e| format!("Query error: {:?}", e))
        }
    }

    /// Get the WAT language (async because it loads from WASM)
    pub async fn wat_language() -> Result<Language, String> {
        // Convert bytes to Uint8Array
        let array = js_sys::Uint8Array::new_with_length(WAT_GRAMMAR_WASM.len() as u32);
        array.copy_from(WAT_GRAMMAR_WASM);

        let lang = WtsLanguage::load_bytes(&array)
            .await
            .map_err(|e| format!("Failed to load WAT grammar: {:?}", e))?;
        Ok(Language(lang))
    }

    /// Query wrapper for running tree-sitter queries
    pub struct Query(WtsQuery);

    impl Query {
        /// Get the capture names defined in this query
        pub fn capture_names(&self) -> Vec<String> {
            let names = self.0.capture_names();
            names.iter().filter_map(|v| v.as_string()).collect()
        }

        /// Run captures on a node, returning all captures
        pub fn captures(&self, node: &Node) -> Vec<QueryCapture> {
            use wasm_bindgen::JsCast;
            let captures = self.0.captures(&node.0, None, None);

            let mut result = Vec::new();
            for v in captures.iter() {
                let obj = match v.dyn_ref::<js_sys::Object>() {
                    Some(o) => o,
                    None => continue,
                };

                let name = match js_sys::Reflect::get(obj, &"name".into())
                    .ok()
                    .and_then(|n| n.as_string())
                {
                    Some(s) => s,
                    None => continue,
                };

                let node_val = match js_sys::Reflect::get(obj, &"node".into()).ok() {
                    Some(n) => n,
                    None => continue,
                };

                // Use unchecked_into since the JS object is a valid SyntaxNode
                // but dyn_ref fails due to wasm-bindgen type mismatch
                let node: SyntaxNode = node_val.unchecked_into();

                result.push(QueryCapture {
                    name,
                    node: Node(node),
                });
            }

            result
        }
    }

    /// A single capture from a query
    #[derive(Clone)]
    pub struct QueryCapture {
        name: String,
        node: Node,
    }

    impl QueryCapture {
        /// Get the capture name (e.g., "comment", "keyword", "string")
        pub fn name(&self) -> String {
            self.name.clone()
        }

        /// Get the captured node
        pub fn node(&self) -> Node {
            self.node.clone()
        }
    }

    /// Parser wrapper
    pub struct Parser(WtsParser);

    impl Parser {
        pub fn new() -> Result<Self, String> {
            WtsParser::new().map(Parser).map_err(|e| format!("{:?}", e))
        }

        pub fn set_language(&mut self, language: &Language) -> Result<(), String> {
            self.0
                .set_language(Some(&language.0))
                .map_err(|e| format!("{:?}", e))
        }

        pub fn parse(&mut self, source: &str, _old_tree: Option<&Tree>) -> Option<Tree> {
            let js_string = js_sys::JsString::from(source);
            // Note: web-tree-sitter-sg doesn't support incremental parsing the same way
            self.0
                .parse_with_string(&js_string, None, None)
                .ok()
                .flatten()
                .map(Tree)
        }
    }

    /// Create a parser configured for WAT (async)
    /// Returns both the parser and the language for query creation
    pub async fn create_parser() -> Result<(Parser, Language), String> {
        let mut parser = Parser::new()?;
        let language = wat_language().await?;
        parser.set_language(&language)?;
        Ok((parser, language))
    }

    /// Tree wrapper
    pub struct Tree(WtsTree);

    impl Tree {
        pub fn root_node(&self) -> Node {
            Node(self.0.root_node())
        }
    }

    impl Clone for Tree {
        fn clone(&self) -> Self {
            // web-tree-sitter-sg Tree doesn't implement Clone directly
            // but we can get a copy through the root node's tree
            Tree(self.0.clone())
        }
    }

    /// Node wrapper that provides a tree-sitter-compatible API
    #[derive(Clone)]
    pub struct Node(SyntaxNode);

    impl Node {
        /// Get the node's type/kind as a string
        pub fn kind(&self) -> String {
            self.0.type_().into()
        }

        /// Get the byte range of this node
        pub fn byte_range(&self) -> Range<usize> {
            self.0.start_index() as usize..self.0.end_index() as usize
        }

        /// Get the start byte offset
        pub fn start_byte(&self) -> usize {
            self.0.start_index() as usize
        }

        /// Get the end byte offset
        pub fn end_byte(&self) -> usize {
            self.0.end_index() as usize
        }

        /// Get the start position (line, column)
        pub fn start_position(&self) -> Point {
            let pos = self.0.start_position();
            Point {
                row: pos.row() as usize,
                column: pos.column() as usize,
            }
        }

        /// Get the end position (line, column)
        pub fn end_position(&self) -> Point {
            let pos = self.0.end_position();
            Point {
                row: pos.row() as usize,
                column: pos.column() as usize,
            }
        }

        /// Get the parent node
        pub fn parent(&self) -> Option<Node> {
            self.0.parent().map(Node)
        }

        /// Check if node has an error
        pub fn has_error(&self) -> bool {
            self.0.has_error()
        }

        /// Check if this is a named node
        pub fn is_named(&self) -> bool {
            self.0.is_named()
        }

        /// Get child count
        pub fn child_count(&self) -> usize {
            self.0.child_count() as usize
        }

        /// Get a child by index
        pub fn child(&self, index: usize) -> Option<Node> {
            self.0.child(index as u32).map(Node)
        }

        /// Get all children
        pub fn children<'a>(
            &'a self,
            _cursor: &'a mut TreeCursor,
        ) -> impl Iterator<Item = Node> + 'a {
            (0..self.child_count()).filter_map(move |i| self.child(i))
        }

        /// Create a tree cursor starting at this node
        pub fn walk(&self) -> TreeCursor {
            TreeCursor(self.0.walk())
        }

        /// Get the range (start and end points) of this node
        pub fn range(&self) -> NodeRange {
            NodeRange {
                start_point: self.start_position(),
                end_point: self.end_position(),
                start_byte: self.start_byte(),
                end_byte: self.end_byte(),
            }
        }
    }

    /// Range struct matching tree-sitter's Range
    #[derive(Debug, Clone, Copy)]
    pub struct NodeRange {
        pub start_point: Point,
        pub end_point: Point,
        pub start_byte: usize,
        pub end_byte: usize,
    }

    /// Point struct for positions (matches tree_sitter::Point)
    #[derive(Debug, Clone, Copy, Default)]
    pub struct Point {
        pub row: usize,
        pub column: usize,
    }

    impl Point {
        pub fn new(row: usize, column: usize) -> Self {
            Self { row, column }
        }
    }

    /// TreeCursor wrapper
    pub struct TreeCursor(web_tree_sitter_sg::TreeCursor);

    impl TreeCursor {
        pub fn node(&self) -> Node {
            Node(self.0.current_node())
        }

        pub fn goto_first_child(&mut self) -> bool {
            self.0.goto_first_child()
        }

        pub fn goto_next_sibling(&mut self) -> bool {
            self.0.goto_next_sibling()
        }

        pub fn goto_parent(&mut self) -> bool {
            self.0.goto_parent()
        }
    }
}

// ============================================================================
// Re-exports based on feature
// ============================================================================

#[cfg(feature = "native")]
pub use native::*;

#[cfg(all(feature = "wasm", not(feature = "native")))]
pub use wasm::*;

// ============================================================================
// Tests (native only)
// ============================================================================

#[cfg(test)]
#[cfg(feature = "native")]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let _parser = create_parser();
    }

    #[test]
    fn test_basic_parsing() {
        let mut parser = create_parser();
        let source = "(module (func $add (param i32 i32) (result i32)))";
        let tree = parser.parse(source, None).expect("Parse failed");

        assert!(!tree.root_node().has_error(), "Parse tree has errors");
        assert_eq!(tree.root_node().kind(), "ROOT");
    }
}
