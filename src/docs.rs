//! Documentation access module for WAT instruction and annotation documentation
//!
//! This module provides programmatic access to the instruction and annotation
//! documentation that is generated at build time from `packages/docs/instructions.md`
//! and `packages/docs/annotations.md`.

mod generated {
    include!(concat!(env!("OUT_DIR"), "/instruction_docs.rs"));
}

mod generated_annotations {
    include!(concat!(env!("OUT_DIR"), "/annotation_docs.rs"));
}

/// Binary-search lookup in a sorted `(name, doc)` array.
fn lookup(table: &'static [(&str, &str)], name: &str) -> Option<&'static str> {
    table
        .binary_search_by_key(&name, |(k, _)| k)
        .ok()
        .map(|i| table[i].1)
}

/// Get documentation for a specific instruction
pub fn get_instruction_doc(name: &str) -> Option<&'static str> {
    lookup(&generated::INSTRUCTION_DOCS, name)
}

/// Get all instruction names (native-only: used by wat-docs binary)
#[cfg(feature = "native")]
pub fn instruction_names() -> Vec<&'static str> {
    generated::INSTRUCTION_DOCS
        .iter()
        .map(|(k, _)| *k)
        .collect()
}

/// Get all instruction documentation (native-only)
#[cfg(feature = "native")]
pub fn all_instructions() -> &'static [(&'static str, &'static str)] {
    &generated::INSTRUCTION_DOCS
}

/// Search for instructions matching a pattern (case-insensitive substring match)
#[cfg(feature = "native")]
pub fn search_instructions(pattern: &str) -> Vec<(&'static str, &'static str)> {
    let pattern_lower = pattern.to_lowercase();
    generated::INSTRUCTION_DOCS
        .iter()
        .filter(|(name, doc)| {
            name.to_lowercase().contains(&pattern_lower)
                || doc.to_lowercase().contains(&pattern_lower)
        })
        .copied()
        .collect()
}

/// Search for instructions where the name matches a pattern
#[cfg(feature = "native")]
pub fn search_instruction_names(pattern: &str) -> Vec<&'static str> {
    let pattern_lower = pattern.to_lowercase();
    generated::INSTRUCTION_DOCS
        .iter()
        .filter(|(name, _)| name.to_lowercase().contains(&pattern_lower))
        .map(|(k, _)| *k)
        .collect()
}

// ============================================================================
// Annotation Documentation
// ============================================================================

/// Get documentation for a specific annotation (without @ prefix)
pub fn get_annotation_doc(name: &str) -> Option<&'static str> {
    lookup(&generated_annotations::ANNOTATION_DOCS, name)
}

/// Get all annotation names (native-only: used by wat-docs binary)
#[cfg(feature = "native")]
pub fn annotation_names() -> Vec<&'static str> {
    generated_annotations::ANNOTATION_DOCS
        .iter()
        .map(|(k, _)| *k)
        .collect()
}

/// Get all annotation documentation (native-only)
#[cfg(feature = "native")]
pub fn all_annotations() -> &'static [(&'static str, &'static str)] {
    &generated_annotations::ANNOTATION_DOCS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_instruction_doc_exists() {
        // Test that common instructions have documentation
        assert!(get_instruction_doc("i32.add").is_some());
        assert!(get_instruction_doc("i32.const").is_some());
        assert!(get_instruction_doc("local.get").is_some());
        assert!(get_instruction_doc("call").is_some());
        assert!(get_instruction_doc("block").is_some());
    }

    #[test]
    fn test_get_instruction_doc_not_exists() {
        // Test that non-existent instructions return None
        assert!(get_instruction_doc("not.an.instruction").is_none());
        assert!(get_instruction_doc("").is_none());
        assert!(get_instruction_doc("foobar").is_none());
    }

    #[test]
    fn test_get_instruction_doc_content() {
        // Test that documentation contains expected content
        let doc = get_instruction_doc("i32.add").unwrap();
        assert!(doc.to_lowercase().contains("add"));
        assert!(doc.contains("i32"));
    }

    #[test]
    fn test_instruction_names_not_empty() {
        let names = instruction_names();
        assert!(!names.is_empty());
        // Should have many instructions
        assert!(names.len() > 100);
    }

    #[test]
    fn test_instruction_names_sorted() {
        let names = instruction_names();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn test_instruction_names_contains_common() {
        let names = instruction_names();
        assert!(names.contains(&"i32.add"));
        assert!(names.contains(&"i64.mul"));
        assert!(names.contains(&"f32.neg"));
        assert!(names.contains(&"memory.grow"));
    }

    #[test]
    fn test_all_instructions_not_empty() {
        let all = all_instructions();
        assert!(!all.is_empty());
        assert!(all.len() > 100);
    }

    #[test]
    fn test_all_instructions_matches_names() {
        let all = all_instructions();
        let names = instruction_names();
        assert_eq!(all.len(), names.len());
    }

    #[test]
    fn test_search_instructions_by_name() {
        let results = search_instructions("i32.add");
        assert!(!results.is_empty());
        // Should find i32.add
        assert!(results.iter().any(|(name, _)| *name == "i32.add"));
    }

    #[test]
    fn test_search_instructions_by_content() {
        // Search for something that appears in documentation but not instruction names
        let results = search_instructions("memory");
        assert!(!results.is_empty());
        // Should find memory-related instructions
        assert!(results.iter().any(|(name, _)| name.contains("memory")));
    }

    #[test]
    fn test_search_instructions_case_insensitive() {
        let results_lower = search_instructions("memory");
        let results_upper = search_instructions("MEMORY");
        let results_mixed = search_instructions("MeMoRy");

        assert_eq!(results_lower.len(), results_upper.len());
        assert_eq!(results_lower.len(), results_mixed.len());
    }

    #[test]
    fn test_search_instructions_sorted() {
        let results = search_instructions("i32");
        let names: Vec<_> = results.iter().map(|(n, _)| *n).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn test_search_instructions_no_match() {
        let results = search_instructions("xyznotfound123");
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_instruction_names_basic() {
        let results = search_instruction_names("i32");
        assert!(!results.is_empty());
        // All results should contain "i32"
        for name in &results {
            assert!(name.to_lowercase().contains("i32"));
        }
    }

    #[test]
    fn test_search_instruction_names_case_insensitive() {
        let results_lower = search_instruction_names("add");
        let results_upper = search_instruction_names("ADD");
        assert_eq!(results_lower, results_upper);
    }

    #[test]
    fn test_search_instruction_names_sorted() {
        let results = search_instruction_names("load");
        let mut sorted = results.clone();
        sorted.sort();
        assert_eq!(results, sorted);
    }

    #[test]
    fn test_search_instruction_names_no_match() {
        let results = search_instruction_names("xyznotfound123");
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_instruction_names_partial() {
        // Search for partial instruction names
        let results = search_instruction_names(".add");
        assert!(results.contains(&"i32.add"));
        assert!(results.contains(&"i64.add"));
        assert!(results.contains(&"f32.add"));
        assert!(results.contains(&"f64.add"));
    }

    // Annotation documentation tests

    #[test]
    fn test_get_annotation_doc_exists() {
        // Test that common annotations have documentation
        assert!(get_annotation_doc("name").is_some());
        assert!(get_annotation_doc("producers").is_some());
        assert!(get_annotation_doc("custom").is_some());
    }

    #[test]
    fn test_get_annotation_doc_not_exists() {
        // Test that non-existent annotations return None
        assert!(get_annotation_doc("not_an_annotation").is_none());
        assert!(get_annotation_doc("").is_none());
    }

    #[test]
    fn test_get_annotation_doc_content() {
        // Test that documentation contains expected content
        let doc = get_annotation_doc("name").unwrap();
        assert!(doc.to_lowercase().contains("name"));
    }

    #[test]
    fn test_annotation_names_not_empty() {
        let names = annotation_names();
        assert!(!names.is_empty());
    }

    #[test]
    fn test_annotation_names_sorted() {
        let names = annotation_names();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn test_all_annotations_not_empty() {
        let all = all_annotations();
        assert!(!all.is_empty());
    }
}
