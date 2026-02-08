// Example WAT programs for the playground
// Uses Vite's import.meta.glob to load examples from the examples/ directory

export interface ExampleDefinition {
  id: string;
  label: string;
  filename: string;
  code: string;
}

// Import all .wat files from examples/ as raw strings
const exampleModules = import.meta.glob<string>('./examples/*.wat', {
  eager: true,
  query: '?raw',
  import: 'default',
});

// Import invalid examples from examples/invalid
const invalidExampleModules = import.meta.glob<string>(
  './examples/invalid/*.wat',
  {
    eager: true,
    query: '?raw',
    import: 'default',
  }
);

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
const validExamples = Object.entries(exampleModules).map(([path, code]) => {
  const filename = path.split('/').pop() || '';
  const id = filenameToId(path);
  const label = extractLabel(code, id);
  return { id, label, filename, code };
});

// Build invalid examples with "Failure - " prefix
const invalidExamples = Object.entries(invalidExampleModules).map(
  ([path, code]) => {
    const filename = path.split('/').pop() || '';
    const id = `invalid_${filenameToId(path)}`;
    let label = extractLabel(code, id);
    // Replace "Invalid: " prefix with "Failure - " or add "Failure - " prefix
    if (label.startsWith('Invalid: ')) {
      label = 'Failure - ' + label.slice('Invalid: '.length);
    } else {
      label = 'Failure - ' + label;
    }
    return { id, label, filename, code };
  }
);

export const examples: ExampleDefinition[] = [...validExamples, ...invalidExamples].sort(
  (a, b) => a.label.localeCompare(b.label)
);

// Helper to get example by ID
export function getExampleById(id: string): ExampleDefinition | undefined {
  return examples.find((ex) => ex.id === id);
}

// Helper to get the default example
export function getDefaultExample(): ExampleDefinition {
  // Prefer 'hello' as default if available, otherwise first example
  return getExampleById('hello') || examples[0];
}
