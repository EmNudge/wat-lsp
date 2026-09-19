use std::env;
use std::fs;
use std::path::Path;

// Shared, testable markdown-docs parser and code generator. `include!`d here for
// codegen and from `tests/instruction_docs_codegen.rs` for validation so both
// agree on one parsed instruction set.
include!("build_support/instruction_docs.rs");

fn main() {
    // Instruction documentation generation
    println!("cargo:rerun-if-changed=packages/docs/instructions.md");
    // Annotation documentation generation
    println!("cargo:rerun-if-changed=packages/docs/annotations.md");
    println!("cargo:rerun-if-changed=build_support/instruction_docs.rs");

    let out_dir = env::var_os("OUT_DIR").unwrap();

    // Generate instruction docs
    let dest_path = Path::new(&out_dir).join("instruction_docs.rs");
    let generated_code = generate_docs_table("packages/docs/instructions.md", "INSTRUCTION_DOCS");
    fs::write(&dest_path, generated_code).expect("Failed to write generated code");

    // Generate annotation docs
    let annotation_dest_path = Path::new(&out_dir).join("annotation_docs.rs");
    let annotation_generated =
        generate_docs_table("packages/docs/annotations.md", "ANNOTATION_DOCS");
    fs::write(&annotation_dest_path, annotation_generated)
        .expect("Failed to write annotation docs");

    // Tree-sitter grammar compilation - only for native targets (not WASM)
    let target = env::var("TARGET").unwrap_or_default();
    if !target.contains("wasm") {
        compile_tree_sitter_grammar();
    }

    // The WASM build embeds a precompiled grammar via `include_bytes!` in
    // `src/ts_facade.rs`. That file is produced out-of-band (`tree-sitter build
    // --wasm`) and is not checked in, so on a clean checkout `include_bytes!`
    // fails with an opaque "No such file or directory" error. When the `wasm`
    // feature is enabled, verify the artifact up front and, if it is missing,
    // fail with the exact command needed to generate it.
    if env::var_os("CARGO_FEATURE_WASM").is_some() {
        check_wasm_grammar_artifact();
    }
}

fn check_wasm_grammar_artifact() {
    const GRAMMAR_WASM: &str = "grammars/tree-sitter-wat/tree-sitter-wat.wasm";
    println!("cargo:rerun-if-changed={GRAMMAR_WASM}");
    println!("cargo:rerun-if-changed=grammars/tree-sitter-wat/grammar.js");

    if Path::new(GRAMMAR_WASM).exists() {
        return;
    }

    const REQUIRED_TREE_SITTER: &str = "tree-sitter-cli@0.27.0";
    panic!(
        "\nMissing WASM grammar artifact: {GRAMMAR_WASM}\n\
         \n\
         The `wasm` feature embeds this precompiled grammar via `include_bytes!`, \
         but it is not checked in and must be generated on a clean checkout.\n\
         \n\
         Generate it with the pinned tree-sitter CLI:\n\
         \n    npm install -g {REQUIRED_TREE_SITTER}\
         \n    (cd grammars/tree-sitter-wat && tree-sitter generate && tree-sitter build --wasm)\n"
    );
}

fn compile_tree_sitter_grammar() {
    use std::process::Command;

    let grammar_dir = "grammars/tree-sitter-wat";
    let grammar_path = format!("{}/grammar.js", grammar_dir);

    println!("cargo:rerun-if-changed={}", grammar_path);
    println!("cargo:rerun-if-changed={}/src/scanner.c", grammar_dir);

    // Generate the parser from grammar.js using tree-sitter CLI
    // On Windows, npm installs binaries as .cmd files
    let tree_sitter_cmd = if cfg!(target_os = "windows") {
        "tree-sitter.cmd"
    } else {
        "tree-sitter"
    };

    // The generated parser (src/parser.c) is not committed, so it must be
    // regenerated on every clean checkout. Pin the CLI version to match CI
    // (.github/workflows) so generated output stays reproducible.
    const REQUIRED_TREE_SITTER: &str = "tree-sitter-cli@0.27.0";
    let status = Command::new(tree_sitter_cmd)
        .args(["generate"])
        .current_dir(grammar_dir)
        .status()
        .unwrap_or_else(|e| {
            panic!(
                "Failed to run `{tree_sitter_cmd} generate` ({e}).\n\
                 The tree-sitter parser is generated at build time and is not \
                 checked in.\n\
                 Install the pinned CLI and rebuild: npm install -g {REQUIRED_TREE_SITTER}"
            )
        });

    if !status.success() {
        panic!(
            "`{tree_sitter_cmd} generate` failed in {grammar_dir}. \
             Ensure {REQUIRED_TREE_SITTER} is installed and grammar.js is valid."
        );
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

/// Read `path`, parse it with the shared docs parser, and render the generated
/// Rust table. Fails the build with a clear, actionable message when the file is
/// missing or its contents are malformed (duplicate/empty entries), instead of
/// silently producing wrong or empty output.
fn generate_docs_table(path: &str, var_name: &str) -> String {
    let content = fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "Failed to read required docs file `{path}` ({e}).\n\
             This file is parsed at build time to generate `{var_name}`; it must \
             exist on a clean checkout."
        )
    });

    let entries = parse_docs(&content).unwrap_or_else(|e| {
        panic!("Invalid docs input in `{path}`: {e}");
    });

    generate_rust_code(&entries, var_name)
}
