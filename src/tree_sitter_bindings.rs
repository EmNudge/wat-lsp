use tree_sitter::{Language, Parser};

// External C function from compiled grammar
extern "C" {
    fn tree_sitter_wat() -> Language;
}

/// Get the WAT language grammar
pub fn wat_language() -> Language {
    unsafe { tree_sitter_wat() }
}

/// Create a new parser configured for WAT
pub fn create_parser() -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&wat_language())
        .expect("Failed to load WAT grammar");
    parser
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = create_parser();
        // Parser should be created successfully
        drop(parser);
    }

    #[test]
    fn test_basic_parsing() {
        let mut parser = create_parser();
        let source = "(module (func $add (param i32 i32) (result i32)))";
        let tree = parser.parse(source, None).expect("Parse failed");

        assert!(!tree.root_node().has_error(), "Parse tree has errors");
        assert_eq!(tree.root_node().kind(), "ROOT");

        // Verify the tree has module as a child
        let mut cursor = tree.root_node().walk();
        let children: Vec<_> = tree.root_node().children(&mut cursor).collect();
        assert!(!children.is_empty(), "ROOT should have children");
    }
}
