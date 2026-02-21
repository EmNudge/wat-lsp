// External scanner for nested block comments and annotations in WAT.
//
// Tree-sitter's stateless lexer cannot track nesting depth, so we handle
// block comments (which nest per the WAT spec) and annotations (which
// contain strings with ')' characters) with a custom scanner.
//
// The three external tokens are:
//   0 - COMMENT_BLOCK       (used in extras)
//   1 - COMMENT_BLOCK_ANNOT (used inside annotation_part)
//   2 - ANNOTATION          (entire annotation including parens)

#include "tree_sitter/parser.h"

enum TokenType {
  COMMENT_BLOCK,
  COMMENT_BLOCK_ANNOT,
  ANNOTATION,
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

/// Scan an annotation starting after the '(' has been consumed.
/// The lexer should be positioned at '@'.
/// Tracks paren depth, skipping strings and comments.
/// Returns true when the matching ')' is found.
static bool scan_annotation(TSLexer *lexer) {
  // We expect '@' next
  if (lexer->lookahead != '@') return false;
  lexer->advance(lexer, false);

  // Paren depth starts at 1 (the opening '(' was already consumed)
  int depth = 1;

  while (depth > 0 && !lexer->eof(lexer)) {
    switch (lexer->lookahead) {
      case '"':
        // Skip string literal (handles \\, \", and other escapes)
        lexer->advance(lexer, false);
        while (!lexer->eof(lexer) && lexer->lookahead != '"') {
          if (lexer->lookahead == '\\') {
            lexer->advance(lexer, false); // skip backslash
            if (!lexer->eof(lexer)) {
              lexer->advance(lexer, false); // skip escaped char
            }
          } else {
            lexer->advance(lexer, false);
          }
        }
        if (!lexer->eof(lexer)) {
          lexer->advance(lexer, false); // skip closing '"'
        }
        break;

      case '(':
        lexer->advance(lexer, false);
        if (lexer->lookahead == ';') {
          // Block comment — skip nested (;...;)
          lexer->advance(lexer, false);
          int comment_depth = 1;
          while (comment_depth > 0 && !lexer->eof(lexer)) {
            if (lexer->lookahead == '(') {
              lexer->advance(lexer, false);
              if (lexer->lookahead == ';') {
                comment_depth++;
                lexer->advance(lexer, false);
              }
            } else if (lexer->lookahead == ';') {
              lexer->advance(lexer, false);
              if (lexer->lookahead == ')') {
                comment_depth--;
                lexer->advance(lexer, false);
              }
            } else {
              lexer->advance(lexer, false);
            }
          }
        } else {
          // Nested paren
          depth++;
        }
        break;

      case ';':
        lexer->advance(lexer, false);
        if (lexer->lookahead == ';') {
          // Line comment — skip to end of line
          while (!lexer->eof(lexer) && lexer->lookahead != '\n') {
            lexer->advance(lexer, false);
          }
        }
        // If it was ';' followed by ')', that's just ';' then ')' — ')' handled next iteration
        break;

      case ')':
        depth--;
        lexer->advance(lexer, false);
        if (depth == 0) {
          return true;
        }
        break;

      default:
        lexer->advance(lexer, false);
        break;
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
  if (!valid_symbols[COMMENT_BLOCK] && !valid_symbols[COMMENT_BLOCK_ANNOT] && !valid_symbols[ANNOTATION]) {
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

  // Peek at next char to distinguish (;...) block comment vs (@...) annotation
  lexer->advance(lexer, false);

  if (lexer->lookahead == '@' && valid_symbols[ANNOTATION]) {
    // Annotation: (@name ...)
    if (!scan_annotation(lexer)) return false;
    lexer->mark_end(lexer);
    lexer->result_symbol = ANNOTATION;
    return true;
  }

  if (lexer->lookahead == ';' && (valid_symbols[COMMENT_BLOCK] || valid_symbols[COMMENT_BLOCK_ANNOT])) {
    // Block comment: (; ... ;)
    // scan_block_comment expects lexer at '(' but we already consumed it.
    // Consume the ';' and then scan the body directly.
    lexer->advance(lexer, false);

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
            lexer->mark_end(lexer);
            if (valid_symbols[COMMENT_BLOCK_ANNOT]) {
              lexer->result_symbol = COMMENT_BLOCK_ANNOT;
            } else {
              lexer->result_symbol = COMMENT_BLOCK;
            }
            return true;
          }
          lexer->advance(lexer, false);
        }
      } else {
        lexer->advance(lexer, false);
      }
    }
  }

  return false;
}
