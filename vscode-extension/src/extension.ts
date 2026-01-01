import * as path from 'path';
import { workspace, ExtensionContext, window } from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
  // Get the server path from settings or use the bundled binary
  const config = workspace.getConfiguration('watLsp');
  let serverPath = config.get<string>('serverPath', '');

  if (!serverPath) {
    // Use bundled server binary based on platform and architecture
    const serverDir = path.join(context.extensionPath, 'server');
    let serverExecutable: string;

    const platform = process.platform;
    const arch = process.arch;

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

    serverPath = path.join(serverDir, serverExecutable);
  }

  // Server options - launch the LSP server
  const serverOptions: ServerOptions = {
    command: serverPath,
    args: [],
    transport: TransportKind.stdio,
  };

  // Options for the language client
  const clientOptions: LanguageClientOptions = {
    // Register the server for WAT files
    // Use pattern-based selectors to work regardless of which extension owns the language ID
    documentSelector: [
      { scheme: 'file', pattern: '**/*.wat' },
      { scheme: 'file', pattern: '**/*.wast' },
      { scheme: 'file', language: 'wat' },  // In case we own it
      { scheme: 'file', language: 'wasm' }, // In case vscode-wasm uses this ID
    ],
    synchronize: {
      // Notify the server about file changes to .wat and .wast files
      fileEvents: workspace.createFileSystemWatcher('**/*.{wat,wast}')
    }
  };

  // Create and start the language client
  client = new LanguageClient(
    'watLsp',
    'WAT Language Server',
    serverOptions,
    clientOptions
  );

  // Start the client (this will also launch the server)
  client.start().catch(err => {
    window.showErrorMessage(
      `Failed to start WAT Language Server: ${err.message}\n\n` +
      `Server path: ${serverPath}\n\n` +
      `Make sure the server binary exists and is executable. ` +
      `You may need to run: cargo build --release`
    );
  });
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
