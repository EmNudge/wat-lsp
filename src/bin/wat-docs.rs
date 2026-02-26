//! wat-docs: A CLI tool for searching and browsing WAT instruction documentation
//!
//! This tool provides access to WebAssembly instruction documentation and is designed
//! to integrate with tools like fzf for interactive searching.
//!
//! # Examples
//!
//! List all instructions (pipe to fzf):
//! ```sh
//! wat-docs list | fzf --preview 'wat-docs show {}'
//! ```
//!
//! Show documentation for a specific instruction:
//! ```sh
//! wat-docs show i32.add
//! ```
//!
//! Search for instructions by name or content:
//! ```sh
//! wat-docs search memory
//! ```

use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use wat_lsp_rust::docs;
use wat_lsp_rust::tree_sitter_bindings::create_parser;

#[derive(Parser, Debug)]
#[command(name = "wat-docs")]
#[command(
    author,
    version,
    about = "Search and browse WAT instruction documentation"
)]
#[command(
    long_about = "A CLI tool for searching and browsing WebAssembly Text format instruction documentation.\n\n\
    Designed to work with fzf for interactive searching:\n\n    \
    wat-docs list | fzf --preview 'wat-docs show {}'"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List all instruction names (one per line, suitable for fzf)
    List {
        /// Filter instruction names containing this pattern
        #[arg(short, long)]
        filter: Option<String>,
    },

    /// Show documentation for a specific instruction
    Show {
        /// The instruction name (e.g., i32.add, memory.grow)
        instruction: String,

        /// Output raw markdown without formatting or highlighting
        #[arg(short, long)]
        raw: bool,

        /// Disable syntax highlighting (use plain text)
        #[arg(long)]
        no_color: bool,

        /// Force color output even when not a TTY (useful for piping to less -R)
        #[arg(long)]
        color: bool,
    },

    /// Search instructions by name or documentation content
    Search {
        /// Search pattern (case-insensitive substring match)
        pattern: String,

        /// Only search instruction names, not documentation content
        #[arg(short, long)]
        names_only: bool,

        /// Show full documentation for each match
        #[arg(short, long)]
        full: bool,

        /// Disable syntax highlighting
        #[arg(long)]
        no_color: bool,

        /// Force color output even when not a TTY
        #[arg(long)]
        color: bool,
    },

    /// Output commands for shell integration (fzf aliases, etc.)
    #[command(name = "shell-setup")]
    ShellSetup {
        /// Shell type
        #[arg(value_enum)]
        shell: ShellType,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ShellType {
    Bash,
    Zsh,
    Fish,
}

/// ANSI escape codes for formatting
mod ansi {
    pub(super) const RESET: &str = "\x1b[0m";
    pub(super) const BOLD: &str = "\x1b[1m";
    pub(super) const DIM: &str = "\x1b[2m";

    // Colors
    pub(super) const GREEN: &str = "\x1b[32m";
    pub(super) const YELLOW: &str = "\x1b[33m";
    pub(super) const BLUE: &str = "\x1b[34m";
    pub(super) const MAGENTA: &str = "\x1b[35m";
    pub(super) const CYAN: &str = "\x1b[36m";

    // Bright colors
    pub(super) const BRIGHT_RED: &str = "\x1b[91m";
    pub(super) const BRIGHT_CYAN: &str = "\x1b[96m";
}

struct Highlighter {
    use_color: bool,
}

impl Highlighter {
    fn new(use_color: bool) -> Self {
        Self { use_color }
    }

    /// Render markdown documentation with syntax highlighting
    fn render_doc(&self, title: &str, content: &str) -> String {
        if !self.use_color {
            return format!("## {}\n\n{}", title, content);
        }

        let mut output = String::new();

        // Render title
        output.push_str(&format!(
            "{}{}## {}{}",
            ansi::BOLD,
            ansi::CYAN,
            title,
            ansi::RESET
        ));
        output.push_str("\n\n");

        // Process content line by line, handling code blocks
        let mut in_code_block = false;
        let mut code_block_content = String::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("```") {
                if in_code_block {
                    // End of code block - highlight and output
                    output.push_str(&self.highlight_wat_code(&code_block_content));
                    output.push_str(&format!("{}{}{}\n", ansi::DIM, "```", ansi::RESET));
                    code_block_content.clear();
                    in_code_block = false;
                } else {
                    // Start of code block
                    in_code_block = true;
                    output.push_str(&format!("{}{}{}\n", ansi::DIM, trimmed, ansi::RESET));
                }
            } else if in_code_block {
                code_block_content.push_str(line);
                code_block_content.push('\n');
            } else if let Some(rest) = trimmed.strip_prefix("Signature:") {
                // Highlight signature line
                output.push_str(&format!(
                    "{}{}Signature:{}{}",
                    ansi::BOLD,
                    ansi::YELLOW,
                    ansi::RESET,
                    rest
                ));
                output.push('\n');
            } else if trimmed.starts_with("Example:") {
                output.push_str(&format!(
                    "{}{}Example:{}",
                    ansi::BOLD,
                    ansi::GREEN,
                    ansi::RESET
                ));
                output.push('\n');
            } else {
                output.push_str(line);
                output.push('\n');
            }
        }

        output
    }

    /// Highlight WAT code using tree-sitter for accurate parsing
    fn highlight_wat_code(&self, code: &str) -> String {
        let mut parser = create_parser();

        let tree = match parser.parse(code, None) {
            Some(t) => t,
            None => return code.to_string(), // Fallback to plain text
        };

        let root = tree.root_node();

        // Collect colored spans
        let mut spans: Vec<(usize, usize, &str, &str)> = Vec::new(); // (start, end, color, reset)

        self.collect_highlight_spans(&root, &mut spans);

        // Sort spans by start position
        spans.sort_by_key(|s| s.0);

        // Build output with colors
        let mut output = String::new();
        let mut last_end = 0;

        for (start, end, color, _) in &spans {
            // Add any unhighlighted text before this span
            if *start > last_end {
                output.push_str(&code[last_end..*start]);
            }

            // Add highlighted text
            if *start < code.len() && *end <= code.len() {
                output.push_str(color);
                output.push_str(&code[*start..*end]);
                output.push_str(ansi::RESET);
            }

            last_end = *end;
        }

        // Add any remaining text
        if last_end < code.len() {
            output.push_str(&code[last_end..]);
        }

        output
    }

    fn collect_highlight_spans<'a>(
        &self,
        node: &tree_sitter::Node,
        spans: &mut Vec<(usize, usize, &'a str, &'a str)>,
    ) {
        let kind = node.kind();
        let start = node.start_byte();
        let end = node.end_byte();

        // Determine color based on node type
        let color: Option<&str> = match kind {
            // Comments - dim gray/italic
            "comment_line" | "comment_block" => Some(ansi::DIM),

            // Keywords - bold magenta
            "module" | "func" | "param" | "result" | "local" | "global" | "memory" | "table"
            | "export" | "import" | "type" | "elem" | "data" | "start" | "mut" | "offset"
            | "item" => Some(ansi::MAGENTA),

            // Control flow keywords - bold blue
            "block" | "loop" | "if" | "then" | "else" | "end" | "br" | "br_if" | "br_table"
            | "return" | "call" | "call_indirect" | "unreachable" | "nop" | "drop" | "select"
            | "try" | "catch" | "catch_all" | "throw" | "rethrow" | "delegate" => Some(ansi::BLUE),

            // Value types - cyan
            "value_type" | "value_type_num_type" | "value_type_ref_type" => Some(ansi::CYAN),

            // Numeric types - cyan
            "num_type_i32" | "num_type_i64" | "num_type_f32" | "num_type_f64" | "num_type_v128" => {
                Some(ansi::CYAN)
            }

            // Reference types
            "ref_type" | "heap_type" => Some(ansi::CYAN),

            // Identifiers/variables ($name) - yellow
            "identifier" => Some(ansi::YELLOW),

            // Numbers - bright red
            "nat" | "int" | "float" | "dec_nat" | "hex_nat" | "dec_float" | "hex_float"
            | "align_offset_value" => Some(ansi::BRIGHT_RED),

            // Strings - green
            "string" => Some(ansi::GREEN),

            // Instructions - bright cyan for the instruction name
            "instr_plain" | "op" => {
                // For instructions, we want to color just the opcode
                if node.child_count() == 0 {
                    Some(ansi::BRIGHT_CYAN)
                } else {
                    None // Let children be colored
                }
            }

            // Plain instruction names (opcodes)
            s if s.starts_with("op_") => Some(ansi::BRIGHT_CYAN),

            // Type names
            "i32" | "i64" | "f32" | "f64" | "v128" | "funcref" | "externref" | "anyref"
            | "eqref" | "i31ref" | "structref" | "arrayref" | "nullref" | "nullfuncref"
            | "nullexternref" => Some(ansi::CYAN),

            _ => None,
        };

        if let Some(c) = color {
            // Only add leaf nodes or specific parent nodes
            if node.child_count() == 0
                || matches!(
                    kind,
                    "comment_line" | "comment_block" | "string" | "identifier"
                )
            {
                spans.push((start, end, c, ansi::RESET));
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_highlight_spans(&child, spans);
        }
    }

    /// Render a separator line
    fn render_separator(&self) -> String {
        if self.use_color {
            format!("\n{}---{}\n\n", ansi::DIM, ansi::RESET)
        } else {
            "\n---\n\n".to_string()
        }
    }
}

fn main() -> ExitCode {
    let args = Args::parse();

    match args.command {
        Command::List { filter } => cmd_list(filter.as_deref()),
        Command::Show {
            instruction,
            raw,
            no_color,
            color,
        } => cmd_show(&instruction, raw, no_color, color),
        Command::Search {
            pattern,
            names_only,
            full,
            no_color,
            color,
        } => cmd_search(&pattern, names_only, full, no_color, color),
        Command::ShellSetup { shell } => cmd_shell_setup(shell),
    }
}

fn cmd_list(filter: Option<&str>) -> ExitCode {
    let names = match filter {
        Some(pattern) => docs::search_instruction_names(pattern),
        None => docs::instruction_names(),
    };

    for name in names {
        println!("{}", name);
    }

    ExitCode::SUCCESS
}

fn should_use_color(no_color_flag: bool, force_color: bool) -> bool {
    // --color forces color on
    if force_color {
        return true;
    }
    // --no-color forces color off
    if no_color_flag {
        return false;
    }
    // Check NO_COLOR env var (https://no-color.org/)
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }
    // Check if stdout is a terminal
    io::stdout().is_terminal()
}

fn cmd_show(instruction: &str, raw: bool, no_color: bool, force_color: bool) -> ExitCode {
    match docs::get_instruction_doc(instruction) {
        Some(doc) => {
            if raw {
                println!("{}", doc);
            } else {
                let highlighter = Highlighter::new(should_use_color(no_color, force_color));
                print!("{}", highlighter.render_doc(instruction, doc));
                io::stdout().flush().ok();
            }
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("Unknown instruction: {}", instruction);
            eprintln!();

            // Suggest similar instructions
            let suggestions = docs::search_instruction_names(instruction);
            if !suggestions.is_empty() {
                eprintln!("Did you mean one of these?");
                for name in suggestions.iter().take(5) {
                    eprintln!("  {}", name);
                }
            }

            ExitCode::from(1)
        }
    }
}

fn cmd_search(
    pattern: &str,
    names_only: bool,
    full: bool,
    no_color: bool,
    force_color: bool,
) -> ExitCode {
    if names_only {
        let names = docs::search_instruction_names(pattern);
        if names.is_empty() {
            eprintln!("No instructions found matching '{}'", pattern);
            return ExitCode::from(1);
        }

        for name in names {
            println!("{}", name);
        }
    } else {
        let results = docs::search_instructions(pattern);
        if results.is_empty() {
            eprintln!("No instructions found matching '{}'", pattern);
            return ExitCode::from(1);
        }

        let highlighter = Highlighter::new(should_use_color(no_color, force_color));

        for (name, doc) in results {
            if full {
                print!("{}", highlighter.render_doc(name, doc));
                print!("{}", highlighter.render_separator());
            } else {
                // Show just the first line of documentation as a summary
                let summary = doc.lines().next().unwrap_or("");
                if highlighter.use_color {
                    println!("{}{}{}: {}", ansi::BOLD, name, ansi::RESET, summary);
                } else {
                    println!("{}: {}", name, summary);
                }
            }
        }
        io::stdout().flush().ok();
    }

    ExitCode::SUCCESS
}

fn cmd_shell_setup(shell: ShellType) -> ExitCode {
    match shell {
        ShellType::Bash | ShellType::Zsh => {
            println!(
                r#"# WAT documentation browser with fzf
# Add this to your .bashrc or .zshrc

wat-browse() {{
    local instruction
    instruction=$(wat-docs list | fzf --preview 'wat-docs show {{}} --color' --preview-window=right:60%:wrap --ansi)
    if [ -n "$instruction" ]; then
        wat-docs show "$instruction"
    fi
}}

# Search and browse
wat-search() {{
    local instruction
    instruction=$(wat-docs search "$1" --names-only | fzf --preview 'wat-docs show {{}} --color' --preview-window=right:60%:wrap --ansi)
    if [ -n "$instruction" ]; then
        wat-docs show "$instruction"
    fi
}}"#
            );
        }
        ShellType::Fish => {
            println!(
                r#"# WAT documentation browser with fzf
# Add this to your config.fish

function wat-browse
    set instruction (wat-docs list | fzf --preview 'wat-docs show {{}} --color' --preview-window=right:60%:wrap --ansi)
    if test -n "$instruction"
        wat-docs show "$instruction"
    end
end

function wat-search
    set instruction (wat-docs search $argv[1] --names-only | fzf --preview 'wat-docs show {{}} --color' --preview-window=right:60%:wrap --ansi)
    if test -n "$instruction"
        wat-docs show "$instruction"
    end
end"#
            );
        }
    }

    ExitCode::SUCCESS
}
