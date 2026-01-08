use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // EXISTING: Instruction documentation generation
    println!("cargo:rerun-if-changed=docs/instructions.md");

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("instruction_docs.rs");

    let docs_content =
        fs::read_to_string("docs/instructions.md").expect("Failed to read docs/instructions.md");

    let docs = parse_instruction_docs(&docs_content);
    let generated_code = generate_rust_code(&docs);

    fs::write(&dest_path, generated_code).expect("Failed to write generated code");

    // Tree-sitter grammar compilation - only for native targets (not WASM)
    let target = env::var("TARGET").unwrap_or_default();
    if !target.contains("wasm") {
        compile_tree_sitter_grammar();
    }
}

fn compile_tree_sitter_grammar() {
    use std::process::Command;

    let grammar_dir = "tree-sitter-wasm";
    let grammar_path = format!("{}/grammar.js", grammar_dir);

    println!("cargo:rerun-if-changed={}", grammar_path);

    // Generate the parser from grammar.js using tree-sitter CLI
    // On Windows, npm installs binaries as .cmd files
    let tree_sitter_cmd = if cfg!(target_os = "windows") {
        "tree-sitter.cmd"
    } else {
        "tree-sitter"
    };

    let status = Command::new(tree_sitter_cmd)
        .args(["generate"])
        .current_dir(grammar_dir)
        .status()
        .expect("Failed to run tree-sitter generate. Make sure tree-sitter-cli is installed.");

    if !status.success() {
        panic!("tree-sitter generate failed");
    }

    // Now compile the generated parser
    let mut build = cc::Build::new();
    build
        .file(format!("{}/src/parser.c", grammar_dir))
        .include(format!("{}/src", grammar_dir))
        .warnings(false); // Tree-sitter generates warnings

    // Check if scanner.c exists (some grammars have external scanners)
    let scanner_path = format!("{}/src/scanner.c", grammar_dir);
    if Path::new(&scanner_path).exists() {
        build.file(scanner_path);
    }

    build.compile("tree-sitter-wat");
}

fn parse_instruction_docs(content: &str) -> HashMap<String, String> {
    let mut docs = HashMap::new();
    // Normalize line endings to handle both Unix (\n) and Windows (\r\n)
    let normalized = content.replace("\r\n", "\n");

    // Process line by line instead of splitting on ---, since --- can appear inside code blocks
    let mut instruction_name: Option<String> = None;
    let mut doc_lines: Vec<&str> = Vec::new();
    let mut in_code_block = false;

    // Helper to save current instruction if we have one
    let save_instruction =
        |name: &mut Option<String>, lines: &mut Vec<&str>, docs: &mut HashMap<String, String>| {
            if let Some(n) = name.take() {
                // Trim trailing empty lines
                while lines.last() == Some(&"") {
                    lines.pop();
                }
                if !lines.is_empty() {
                    let doc = lines.join("\n");
                    docs.insert(n, doc);
                }
                lines.clear();
            }
        };

    for line in normalized.lines() {
        let trimmed = line.trim();

        // Check for code block boundaries
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            // Add code fence to doc if we're collecting
            if instruction_name.is_some() {
                doc_lines.push(trimmed);
            }
            continue;
        }

        // Inside code blocks, preserve original indentation
        if in_code_block {
            if instruction_name.is_some() {
                doc_lines.push(line);
            }
            continue;
        }

        // Outside code blocks - check for section markers and headers

        // Section separator - save current instruction and reset
        if trimmed == "---" {
            save_instruction(&mut instruction_name, &mut doc_lines, &mut docs);
            continue;
        }

        // New instruction header
        if let Some(stripped) = trimmed.strip_prefix("## ") {
            // Save previous instruction if any
            save_instruction(&mut instruction_name, &mut doc_lines, &mut docs);
            instruction_name = Some(stripped.trim().to_string());
            continue;
        }

        // Skip document title
        if trimmed.starts_with("# ") {
            continue;
        }

        // Add content line if we're collecting for an instruction
        if instruction_name.is_some() {
            // Include empty lines only after we've started collecting content
            if !trimmed.is_empty() || !doc_lines.is_empty() {
                doc_lines.push(trimmed);
            }
        }
    }

    // Don't forget the last instruction
    save_instruction(&mut instruction_name, &mut doc_lines, &mut docs);

    docs
}

fn generate_rust_code(docs: &HashMap<String, String>) -> String {
    let mut code = String::from(
        "// This file is automatically generated by build.rs\n\
         // Do not edit manually - edit docs/instructions.md instead\n\n\
         use std::collections::HashMap;\n\
         use once_cell::sync::Lazy;\n\n\
         pub static INSTRUCTION_DOCS: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {\n\
         \x20\x20\x20\x20let mut m = HashMap::new();\n\n",
    );

    let mut sorted_keys: Vec<_> = docs.keys().collect();
    sorted_keys.sort();

    for key in sorted_keys {
        let value = &docs[key];
        // Escape the documentation string for Rust
        let escaped = value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");

        code.push_str(&format!("    m.insert(\"{}\", \"{}\");\n", key, escaped));
    }

    code.push_str(
        "\n    m\n\
         });\n",
    );

    code
}
