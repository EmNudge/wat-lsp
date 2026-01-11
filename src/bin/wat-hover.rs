//! wat-hover: A CLI tool for inspecting hover information in WAT files
//!
//! This tool allows you to see what hover popup would be shown at a specific
//! position in a WAT file, useful for debugging and testing the LSP hover feature.

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use tower_lsp::lsp_types::Position;

use wat_lsp_rust::hover::provide_hover;
use wat_lsp_rust::parser::parse_document;
use wat_lsp_rust::tree_sitter_bindings::create_parser;
use wat_lsp_rust::utils::{
    determine_context_from_line, determine_instruction_context, get_line_at_position,
    get_word_at_position, node_at_position,
};

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
enum OutputFormat {
    /// Human-readable text output (default)
    #[default]
    Text,
    /// JSON output for machine processing
    Json,
}

/// A CLI tool for inspecting hover information in WAT files
#[derive(Parser, Debug)]
#[command(name = "wat-hover")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// WAT file to analyze. Use '-' to read from stdin.
    #[arg(required = true)]
    file: PathBuf,

    /// Line number (1-indexed)
    #[arg(short, long)]
    line: u32,

    /// Column number (1-indexed)
    #[arg(short, long)]
    column: u32,

    /// Output format
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,

    /// Show the source line with a caret pointing to the position
    #[arg(short = 's', long)]
    show_source: bool,

    /// Show debug information about symbols and context
    #[arg(short = 'd', long)]
    debug: bool,
}

#[derive(serde::Serialize)]
struct HoverOutput {
    file: String,
    line: u32,
    column: u32,
    hover: Option<HoverContent>,
}

#[derive(serde::Serialize)]
struct HoverContent {
    content: String,
    kind: String,
}

fn main() -> ExitCode {
    let args = Args::parse();

    // Read the file
    let (filename, source) = if args.file.as_os_str() == "-" {
        let mut buf = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut buf) {
            eprintln!("stdin: Failed to read: {}", e);
            return ExitCode::from(2);
        }
        ("<stdin>".to_string(), buf)
    } else {
        match fs::read_to_string(&args.file) {
            Ok(content) => (args.file.display().to_string(), content),
            Err(e) => {
                eprintln!("{}: Failed to read: {}", args.file.display(), e);
                return ExitCode::from(2);
            }
        }
    };

    // Parse the document
    let mut parser = create_parser();
    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => {
            eprintln!("{}: Failed to parse file", filename);
            return ExitCode::from(1);
        }
    };

    // Extract symbols
    let symbols = match parse_document(&source) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: Failed to extract symbols: {:?}", filename, e);
            return ExitCode::from(1);
        }
    };

    // Convert to 0-indexed position
    let position = Position {
        line: args.line.saturating_sub(1),
        character: args.column.saturating_sub(1),
    };

    // Debug output
    if args.debug {
        eprintln!("=== Debug Info ===");
        eprintln!(
            "Position: line {}, col {}",
            position.line, position.character
        );

        // Show word at position
        if let Some(word) = get_word_at_position(&source, position.into()) {
            eprintln!("Word at position: {:?}", word);
        }

        // Show context detection
        if let Some(node) = node_at_position(&tree, &source, position.into()) {
            eprintln!("Node kind: {:?}", node.kind());
            eprintln!("Node text: {:?}", &source[node.byte_range()]);
            // Walk up tree
            let mut current = node;
            eprintln!("Tree walk:");
            loop {
                eprintln!(
                    "  {} -> {:?}",
                    current.kind(),
                    &source[current.byte_range()]
                        .chars()
                        .take(50)
                        .collect::<String>()
                );
                if let Some(parent) = current.parent() {
                    current = parent;
                } else {
                    break;
                }
            }
            let context = determine_instruction_context(node, &source);
            eprintln!("AST Context: {:?}", context);
        }
        if let Some(line) = get_line_at_position(&source, position.line as usize) {
            let context = determine_context_from_line(line);
            eprintln!("Line-based Context: {:?}", context);
            eprintln!("Line content: {:?}", line.trim());
        }

        eprintln!("\nTables ({}):", symbols.tables.len());
        for table in &symbols.tables {
            eprintln!("  {:?}", table.name);
        }
        eprintln!("Memories ({}):", symbols.memories.len());
        for mem in &symbols.memories {
            eprintln!("  {:?}", mem.name);
        }
        eprintln!("Data segments ({}):", symbols.data_segments.len());
        for data in &symbols.data_segments {
            eprintln!(
                "  {:?}: {:?}",
                data.name,
                &data.content.chars().take(20).collect::<String>()
            );
        }
        eprintln!("Elem segments ({}):", symbols.elem_segments.len());
        for elem in &symbols.elem_segments {
            eprintln!("  {:?}", elem.name);
        }
        eprintln!("Tags ({}):", symbols.tags.len());
        for tag in &symbols.tags {
            eprintln!("  {:?}", tag.name);
        }
        eprintln!("==================");
    }

    // Get hover information
    let hover = provide_hover(&source, &symbols, &tree, position);

    // Show source line if requested
    if args.show_source {
        let lines: Vec<&str> = source.lines().collect();
        if let Some(line_content) = lines.get(position.line as usize) {
            eprintln!();
            eprintln!("  {}:{}", args.line, line_content);
            eprintln!(
                "  {}{}^",
                " ".repeat(args.line.to_string().len() + 1),
                " ".repeat(position.character as usize)
            );
            eprintln!();
        }
    }

    // Output results
    match args.format {
        OutputFormat::Text => {
            if let Some(h) = hover {
                match h.contents {
                    tower_lsp::lsp_types::HoverContents::Markup(markup) => {
                        println!("{}", markup.value);
                    }
                    tower_lsp::lsp_types::HoverContents::Scalar(s) => match s {
                        tower_lsp::lsp_types::MarkedString::String(text) => {
                            println!("{}", text);
                        }
                        tower_lsp::lsp_types::MarkedString::LanguageString(ls) => {
                            println!("```{}\n{}\n```", ls.language, ls.value);
                        }
                    },
                    tower_lsp::lsp_types::HoverContents::Array(arr) => {
                        for item in arr {
                            match item {
                                tower_lsp::lsp_types::MarkedString::String(text) => {
                                    println!("{}", text);
                                }
                                tower_lsp::lsp_types::MarkedString::LanguageString(ls) => {
                                    println!("```{}\n{}\n```", ls.language, ls.value);
                                }
                            }
                        }
                    }
                }
            } else {
                println!(
                    "No hover information at {}:{}:{}",
                    filename, args.line, args.column
                );
            }
        }
        OutputFormat::Json => {
            let output = HoverOutput {
                file: filename,
                line: args.line,
                column: args.column,
                hover: hover.map(|h| match h.contents {
                    tower_lsp::lsp_types::HoverContents::Markup(markup) => HoverContent {
                        content: markup.value,
                        kind: format!("{:?}", markup.kind),
                    },
                    tower_lsp::lsp_types::HoverContents::Scalar(s) => match s {
                        tower_lsp::lsp_types::MarkedString::String(text) => HoverContent {
                            content: text,
                            kind: "PlainText".to_string(),
                        },
                        tower_lsp::lsp_types::MarkedString::LanguageString(ls) => HoverContent {
                            content: ls.value,
                            kind: format!("Language({})", ls.language),
                        },
                    },
                    tower_lsp::lsp_types::HoverContents::Array(arr) => {
                        let combined: Vec<String> = arr
                            .into_iter()
                            .map(|item| match item {
                                tower_lsp::lsp_types::MarkedString::String(text) => text,
                                tower_lsp::lsp_types::MarkedString::LanguageString(ls) => ls.value,
                            })
                            .collect();
                        HoverContent {
                            content: combined.join("\n\n"),
                            kind: "Array".to_string(),
                        }
                    }
                }),
            };
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
    }

    ExitCode::SUCCESS
}
