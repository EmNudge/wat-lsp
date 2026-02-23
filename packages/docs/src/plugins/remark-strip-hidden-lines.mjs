/**
 * Remark plugin that strips hidden context lines from WAT code blocks.
 *
 * Lines starting with `# ` (or bare `#`) inside ```wat code blocks are used for
 * validation context (like Rust doc-tests) but should not appear in rendered docs.
 */
import { visit } from 'unist-util-visit';

export function remarkStripHiddenLines() {
  return (tree) => {
    visit(tree, 'code', (node) => {
      if (node.lang === 'wat') {
        node.value = node.value
          .split('\n')
          .filter((line) => !line.startsWith('# ') && line !== '#')
          .join('\n');
      }
    });
  };
}
