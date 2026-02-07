// External scanner for nested block comments in WAT.
//
// Tree-sitter's stateless lexer cannot track nesting depth, so we handle
// block comments (which nest per the WAT spec) with a custom scanner.
//
// The two external tokens are:
//   0 - COMMENT_BLOCK       (used in extras)
//   1 - COMMENT_BLOCK_ANNOT (used inside annotation_part)

#include "tree_sitter/parser.h"

enum TokenType {
  COMMENT_BLOCK,
  COMMENT_BLOCK_ANNOT,
};

void *tree_sitter_wat_external_scanner_create(void) {
  return NULL;
}

void tree_sitter_wat_external_scanner_destroy(void *payload) {
  (void)payload;
}

unsigned tree_sitter_wat_external_scanner_serialize(void *payload, char *buffer) {
  (void)payload;
  (void)buffer;
  return 0;
}

void tree_sitter_wat_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {
  (void)payload;
  (void)buffer;
  (void)length;
}

/// Scan a block comment starting from the current position.
/// Expects the lexer to be positioned at '(' with ';' following.
/// Returns true if a complete block comment was consumed.
static bool scan_block_comment(TSLexer *lexer) {
  // We should be looking at '('
  if (lexer->lookahead != '(') return false;
  lexer->advance(lexer, false);

  // Next must be ';'
  if (lexer->lookahead != ';') return false;
  lexer->advance(lexer, false);

  // Now scan the comment body, tracking nesting depth
  int depth = 1;
  while (depth > 0 && !lexer->eof(lexer)) {
    if (lexer->lookahead == '(') {
      lexer->advance(lexer, false);
      if (lexer->lookahead == ';') {
        depth++;
        lexer->advance(lexer, false);
      }
    } else if (lexer->lookahead == ';') {
      lexer->advance(lexer, false);
      if (lexer->lookahead == ')') {
        depth--;
        if (depth == 0) {
          lexer->advance(lexer, false);
          return true;
        }
        lexer->advance(lexer, false);
      }
    } else {
      lexer->advance(lexer, false);
    }
  }

  return false;
}

bool tree_sitter_wat_external_scanner_scan(
  void *payload,
  TSLexer *lexer,
  const bool *valid_symbols
) {
  (void)payload;

  // Only attempt if at least one of our symbols is valid
  if (!valid_symbols[COMMENT_BLOCK] && !valid_symbols[COMMENT_BLOCK_ANNOT]) {
    return false;
  }

  // Skip whitespace — tree-sitter does NOT auto-skip before external scanners
  while (lexer->lookahead == ' '  || lexer->lookahead == '\t' ||
         lexer->lookahead == '\n' || lexer->lookahead == '\r' ||
         lexer->lookahead == 0xFEFF) {
    lexer->advance(lexer, true);
  }

  if (lexer->lookahead != '(') return false;

  // Mark the start position
  lexer->mark_end(lexer);

  if (!scan_block_comment(lexer)) return false;

  lexer->mark_end(lexer);

  // Prefer COMMENT_BLOCK_ANNOT when both are valid (annotation context)
  if (valid_symbols[COMMENT_BLOCK_ANNOT]) {
    lexer->result_symbol = COMMENT_BLOCK_ANNOT;
  } else {
    lexer->result_symbol = COMMENT_BLOCK;
  }

  return true;
}
