// Example WAT programs for the playground
// Uses Vite's import.meta.glob to load examples from docs/examples

export interface ExampleDefinition {
  id: string;
  label: string;
  code: string;
}

// Import all .wat files from docs/examples as raw strings
const exampleModules = import.meta.glob<string>('../../docs/examples/*.wat', {
  eager: true,
  query: '?raw',
  import: 'default',
});

// Extract a human-readable label from the file content
// Looks for a comment on the first line like: ";; Exception Handling Proposal"
function extractLabel(code: string, filename: string): string {
  const firstLine = code.split('\n')[0];
  const match = firstLine.match(/^;;\s*(.+)/);
  if (match) {
    return match[1].trim();
  }
  // Fallback: convert filename to title case
  return filename
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

// Convert filename to id (e.g., "numeric_ops.wat" -> "numeric_ops")
function filenameToId(path: string): string {
  const filename = path.split('/').pop() || '';
  return filename.replace('.wat', '');
}

// Build the examples array from imported modules
export const examples: ExampleDefinition[] = Object.entries(exampleModules)
  .map(([path, code]) => {
    const id = filenameToId(path);
    const label = extractLabel(code, id);
    return { id, label, code };
  })
  .sort((a, b) => a.label.localeCompare(b.label));

// Helper to get example by ID
export function getExampleById(id: string): ExampleDefinition | undefined {
  return examples.find((ex) => ex.id === id);
}

// Helper to get the default example
export function getDefaultExample(): ExampleDefinition {
  // Prefer 'hello' as default if available, otherwise first example
  return getExampleById('hello') || examples[0];
}
