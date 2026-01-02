import * as path from 'path';
import * as fs from 'fs';
import { workspace, ExtensionContext, window, commands, languages } from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind
} from 'vscode-languageclient/node';
import { WatSemanticTokensProvider, legend } from './semanticTokensProvider';

let client: LanguageClient | undefined;
let serverPath: string;

function getServerPath(context: ExtensionContext): string {
  const config = workspace.getConfiguration('watLsp');
  let customPath = config.get<string>('serverPath', '');

  if (customPath) {
    return customPath;
  }

  // Use bundled server binary based on platform and architecture
  const serverDir = path.join(context.extensionPath, 'server');

  const platform = process.platform;
  const arch = process.arch;

  // For local development, check for simple binary name first
  const simpleName = platform === 'win32' ? 'wat-lsp-rust.exe' : 'wat-lsp-rust';
  const simplePath = path.join(serverDir, simpleName);
  if (fs.existsSync(simplePath)) {
    return simplePath;
  }

  // Fall back to platform-specific names (for published extension)
  let serverExecutable: string;
  if (platform === 'win32') {
    serverExecutable = 'wat-lsp-rust-win32-x64.exe';
  } else if (platform === 'darwin') {
    serverExecutable = arch === 'arm64'
      ? 'wat-lsp-rust-darwin-arm64'
      : 'wat-lsp-rust-darwin-x64';
  } else {
    // Linux and other Unix-like systems
    serverExecutable = 'wat-lsp-rust-linux-x64';
  }

  return path.join(serverDir, serverExecutable);
}

async function startClient(context: ExtensionContext): Promise<void> {
  if (client) {
    return; // Already running
  }

  const serverOptions: ServerOptions = {
    command: serverPath,
    args: [],
    transport: TransportKind.stdio,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', pattern: '**/*.wat' },
      { scheme: 'file', pattern: '**/*.wast' },
      { scheme: 'file', language: 'wat' },
      { scheme: 'file', language: 'wasm' },
    ],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher('**/*.{wat,wast}')
    }
  };

  // Register semantic tokens provider for tree-sitter syntax highlighting
  const semanticTokensProvider = new WatSemanticTokensProvider();
  context.subscriptions.push(
    languages.registerDocumentSemanticTokensProvider(
      [
        { language: 'wat' },
        { pattern: '**/*.wat' },
        { pattern: '**/*.wast' },
      ],
      semanticTokensProvider,
      legend
    )
  );

  // Create and start the language client
  client = new LanguageClient(
    'watLsp',
    'WAT Language Server',
    serverOptions,
    clientOptions
  );

  try {
    await client.start();
  } catch (err: any) {
    client = undefined;
    window.showErrorMessage(
      `Failed to start WAT Language Server: ${err.message}\n\n` +
      `Server path: ${serverPath}\n\n` +
      `Make sure the server binary exists and is executable. ` +
      `You may need to run: cargo build --release`
    );
  }
}

async function stopClient(): Promise<void> {
  if (!client) {
    return;
  }
  await client.stop();
  client = undefined;
}

export function activate(context: ExtensionContext) {
  console.log('WAT LSP extension activating...');

  // Register toggle command FIRST to ensure it's always available
  const toggleCommand = commands.registerCommand('watLsp.toggle', async () => {
    const config = workspace.getConfiguration('watLsp');
    const currentlyEnabled = config.get<boolean>('enabled', true);
    await config.update('enabled', !currentlyEnabled);
    window.showInformationMessage(`WAT LSP: ${!currentlyEnabled ? 'Enabled' : 'Disabled'}`);
  });
  context.subscriptions.push(toggleCommand);
  console.log('WAT LSP toggle command registered');

  try {
    serverPath = getServerPath(context);
    console.log(`WAT LSP server path: ${serverPath}`);

    // Watch for config changes
    context.subscriptions.push(
      workspace.onDidChangeConfiguration(async (e) => {
        if (e.affectsConfiguration('watLsp.enabled')) {
          const enabled = workspace.getConfiguration('watLsp').get<boolean>('enabled', true);
          if (enabled && !client) {
            await startClient(context);
          } else if (!enabled && client) {
            await stopClient();
          }
        }
      })
    );

    // Start the client if enabled
    const config = workspace.getConfiguration('watLsp');
    if (config.get<boolean>('enabled', true)) {
      startClient(context);
    }
  } catch (err: any) {
    console.error('WAT LSP activation error:', err);
    window.showErrorMessage(`WAT LSP failed to activate: ${err.message}`);
  }

  console.log('WAT LSP extension activated');
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
