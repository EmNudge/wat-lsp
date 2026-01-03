import * as vscode from 'vscode';
import Parser from 'tree-sitter';
import * as path from 'path';
import * as fs from 'fs';

// Token types that VS Code understands
// These map to standard TextMate scopes via semanticTokenScopes in package.json
export const tokenTypes = [
  'comment',           // 0
  'string',            // 1
  'number',            // 2
  'type',              // 3 - for types like i32, f64
  'function',          // 4 - for function definitions
  'variable',          // 5 - for variables
  'keyword',           // 6 - for module keywords
  'operator',          // 7 - for instruction actions (the part after the dot)
  'namespace',         // 8 - for instruction prefixes (the part before the dot)
  'parameter',         // 9 - for parameter names
  'property',          // 10 - for local variables
];

// Custom token type for control flow - mapped via semanticTokenScopes
const TOKEN_KEYWORD_CONTROL = 6; // We'll use modifiers to distinguish

export const tokenModifiers = [
  'declaration',    // 0
  'definition',     // 1
  'controlFlow',    // 2 - for control flow keywords
  'defaultLibrary', // 3 - for builtin types
];

export const legend = new vscode.SemanticTokensLegend(tokenTypes, tokenModifiers);

// Token type indices
const TOKEN_COMMENT = 0;
const TOKEN_STRING = 1;
const TOKEN_NUMBER = 2;
const TOKEN_TYPE = 3;
const TOKEN_FUNCTION = 4;
const TOKEN_VARIABLE = 5;
const TOKEN_KEYWORD = 6;
const TOKEN_OPERATOR = 7;
const TOKEN_NAMESPACE = 8;
const TOKEN_PARAMETER = 9;
const TOKEN_PROPERTY = 10;

// Modifier bit flags
const MOD_DECLARATION = 1 << 0;
const MOD_DEFINITION = 1 << 1;
const MOD_CONTROL_FLOW = 1 << 2;
const MOD_DEFAULT_LIBRARY = 1 << 3;

// Mapping from tree-sitter capture names to token info
interface TokenInfo {
  type: number;
  modifiers: number;
  split?: boolean; // Whether to split namespace.action
}

const captureToTokenInfo: Record<string, TokenInfo> = {
  // Comments and strings
  'comment': { type: TOKEN_COMMENT, modifiers: 0 },
  'string': { type: TOKEN_STRING, modifiers: 0 },

  // Numbers
  'number': { type: TOKEN_NUMBER, modifiers: 0 },

  // Types
  'type.builtin': { type: TOKEN_TYPE, modifiers: MOD_DEFAULT_LIBRARY },
  'type.definition': { type: TOKEN_TYPE, modifiers: MOD_DEFINITION },

  // Keywords
  'keyword': { type: TOKEN_KEYWORD, modifiers: 0 },
  'keyword.control': { type: TOKEN_KEYWORD, modifiers: MOD_CONTROL_FLOW },

  // Instructions - split into namespace.action
  'function.instruction': { type: TOKEN_OPERATOR, modifiers: 0, split: true },

  // Function definitions
  'function.definition': { type: TOKEN_FUNCTION, modifiers: MOD_DEFINITION },

  // Variables
  'variable': { type: TOKEN_VARIABLE, modifiers: 0 },
  'variable.definition': { type: TOKEN_VARIABLE, modifiers: MOD_DEFINITION },
  'variable.parameter': { type: TOKEN_PARAMETER, modifiers: 0 },
  'variable.local': { type: TOKEN_PROPERTY, modifiers: 0 },
};

interface Token {
  line: number;
  startCharacter: number;
  length: number;
  tokenType: number;
  tokenModifiers: number;
}

export class WatSemanticTokensProvider implements vscode.DocumentSemanticTokensProvider {
  private parser: Parser;
  private language: any = null;
  private queryString: string = '';

  constructor() {
    this.parser = new Parser();
    this.loadLanguage();
  }

  private async loadLanguage() {
    try {
      const extensionPath = path.join(__dirname, '../..');
      const grammarPaths = [
        path.join(extensionPath, 'node_modules', 'tree-sitter-wat'),
        path.join(extensionPath, '..', 'tree-sitter-wasm'),
      ];

      let languageModule = null;
      let grammarPath: string | null = null;

      for (const testPath of grammarPaths) {
        try {
          languageModule = require(testPath);
          grammarPath = testPath;
          break;
        } catch (e) {
          const highlightsPath = path.join(testPath, 'queries', 'highlights.scm');
          if (fs.existsSync(highlightsPath)) {
            grammarPath = testPath;
            this.queryString = fs.readFileSync(highlightsPath, 'utf8');
          }
        }
      }

      if (languageModule) {
        this.language = languageModule;
        this.parser.setLanguage(this.language);
        console.log('WAT tree-sitter language loaded successfully');

        if (!this.queryString && grammarPath) {
          const highlightsPath = path.join(grammarPath, 'queries', 'highlights.scm');
          if (fs.existsSync(highlightsPath)) {
            this.queryString = fs.readFileSync(highlightsPath, 'utf8');
          }
        }
      } else {
        console.warn('Could not load tree-sitter-wat native module.');
      }
    } catch (error) {
      console.error('Error loading tree-sitter language:', error);
    }
  }

  public async provideDocumentSemanticTokens(
    document: vscode.TextDocument,
    token: vscode.CancellationToken
  ): Promise<vscode.SemanticTokens | null> {
    if (!this.language) {
      await new Promise(resolve => setTimeout(resolve, 100));
      if (!this.language) {
        return null;
      }
    }

    const text = document.getText();
    const tree = this.parser.parse(text);

    if (!tree) {
      return null;
    }

    const tokens: Token[] = [];

    if (this.queryString && this.language) {
      try {
        const query = this.language.query(this.queryString);
        const captures = query.captures(tree.rootNode);

        for (const capture of captures) {
          const node = capture.node;
          const captureName = capture.name;

          const tokenInfo = captureToTokenInfo[captureName];
          if (!tokenInfo) {
            continue;
          }

          const startPos = document.positionAt(node.startIndex);
          const endPos = document.positionAt(node.endIndex);

          // Handle instruction splitting (namespace.action pattern)
          if (tokenInfo.split && startPos.line === endPos.line) {
            const nodeText = node.text;
            const dotIndex = nodeText.indexOf('.');

            if (dotIndex > 0) {
              // Namespace part (e.g., "local", "i32") - use namespace token type
              tokens.push({
                line: startPos.line,
                startCharacter: startPos.character,
                length: dotIndex,
                tokenType: TOKEN_NAMESPACE,
                tokenModifiers: 0,
              });
              // Action part (e.g., "get", "add") - use operator token type (will be colored like functions)
              tokens.push({
                line: startPos.line,
                startCharacter: startPos.character + dotIndex + 1,
                length: nodeText.length - dotIndex - 1,
                tokenType: TOKEN_OPERATOR,
                tokenModifiers: 0,
              });
            } else {
              // No dot - single instruction like "nop", "drop"
              // Use keyword with controlFlow modifier to match keyword.control.wat scope
              tokens.push({
                line: startPos.line,
                startCharacter: startPos.character,
                length: node.endIndex - node.startIndex,
                tokenType: TOKEN_KEYWORD,
                tokenModifiers: MOD_CONTROL_FLOW,
              });
            }
            continue;
          }

          // Regular token handling
          if (startPos.line === endPos.line) {
            tokens.push({
              line: startPos.line,
              startCharacter: startPos.character,
              length: node.endIndex - node.startIndex,
              tokenType: tokenInfo.type,
              tokenModifiers: tokenInfo.modifiers,
            });
          } else {
            // Multi-line tokens
            for (let line = startPos.line; line <= endPos.line; line++) {
              const lineStart = line === startPos.line ? startPos.character : 0;
              const lineEnd = line === endPos.line ? endPos.character : document.lineAt(line).text.length;

              if (lineEnd > lineStart) {
                tokens.push({
                  line: line,
                  startCharacter: lineStart,
                  length: lineEnd - lineStart,
                  tokenType: tokenInfo.type,
                  tokenModifiers: tokenInfo.modifiers,
                });
              }
            }
          }
        }
      } catch (error) {
        console.error('Error running tree-sitter query:', error);
      }
    }

    // Sort tokens by position
    tokens.sort((a, b) => {
      if (a.line !== b.line) {
        return a.line - b.line;
      }
      return a.startCharacter - b.startCharacter;
    });

    // Build semantic tokens
    const builder = new vscode.SemanticTokensBuilder(legend);
    for (const tok of tokens) {
      builder.push(
        new vscode.Range(
          new vscode.Position(tok.line, tok.startCharacter),
          new vscode.Position(tok.line, tok.startCharacter + tok.length)
        ),
        tokenTypes[tok.tokenType],
        tok.tokenModifiers ? tokenModifiers.filter((_, i) => tok.tokenModifiers & (1 << i)) : []
      );
    }

    return builder.build();
  }
}
