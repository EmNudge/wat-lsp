import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { examples, getDefaultExample } from '../../examples';
import init, { parseWat } from 'js-wasm-tools';
import { extractWatTypes, generateHostCode } from '../codegen';
import { runHostCode } from '../runner/hostRunner';
import type { ConsoleLine } from '../runner/hostRunner';

export interface OpenFile {
    id: string;
    filename: string;
    code: string;
    isDirty: boolean;
    hostCode: string;
    hostDirty: boolean;
}

export const usePlaygroundStore = defineStore('playground', () => {
    const openFiles = ref<OpenFile[]>([]);
    const activeFileId = ref<string | null>(null);
    const lspReady = ref(false);
    const lspStatus = ref('LSP Loading...');
    const consoleOutput = ref<{ message: string; type: string; timestamp: string }[]>([]);
    const diagnostics = ref<any[]>([]);
    const symbols = ref<any[]>([]);
    const hoverInfo = ref<{ line: number; col: number; result: any } | null>(null);

    const wasmBytes = ref<Uint8Array | null>(null);
    const wasmModule = ref<WebAssembly.Module | null>(null);
    const wasmInstance = ref<WebAssembly.Instance | null>(null);
    const isCompiling = ref(false);
    const isRunning = ref(false);
    const recentlyUsedIds = ref<string[]>([]);

    let wasmToolsReady = false;

    async function ensureWasmTools() {
        if (!wasmToolsReady) {
            await init();
            wasmToolsReady = true;
        }
    }

    const currentFile = computed(() =>
        openFiles.value.find(f => f.id === activeFileId.value)
    );

    // Initial load
    const defaultEx = getDefaultExample();
    const defaultInfo = extractWatTypes(defaultEx.code);
    openFiles.value = [{
        id: defaultEx.id,
        filename: defaultEx.filename,
        code: defaultEx.code,
        isDirty: false,
        hostCode: generateHostCode(defaultInfo, defaultEx.filename),
        hostDirty: false
    }];
    activeFileId.value = defaultEx.id;
    recentlyUsedIds.value = [defaultEx.id];

    function log(message: string, type: string = 'log') {
        consoleOutput.value.push({
            message,
            type,
            timestamp: new Date().toLocaleTimeString()
        });
    }

    function clearConsole() {
        consoleOutput.value = [];
    }

    function openFile(id: string) {
        const existing = openFiles.value.find(f => f.id === id);
        if (existing) {
            activeFileId.value = id;
            return;
        }

        const example = examples.find(ex => ex.id === id);
        if (example) {
            const info = extractWatTypes(example.code);
            const hostCode = generateHostCode(info, example.filename);

            const current = currentFile.value;
            if (current && !current.isDirty) {
                // Replace current unedited tab
                const index = openFiles.value.findIndex(f => f.id === current.id);
                if (index !== -1) {
                    openFiles.value[index] = {
                        id: example.id,
                        filename: example.filename,
                        code: example.code,
                        isDirty: false,
                        hostCode,
                        hostDirty: false
                    };
                    activeFileId.value = id;
                    return;
                }
            }

            // Otherwise add new tab
            openFiles.value.push({
                id: example.id,
                filename: example.filename,
                code: example.code,
                isDirty: false,
                hostCode,
                hostDirty: false
            });
            activeFileId.value = id;
        }

        // Add to recently used
        recentlyUsedIds.value = [id, ...recentlyUsedIds.value.filter(x => x !== id)];
    }

    function closeFile(id: string) {
        const index = openFiles.value.findIndex(f => f.id === id);
        if (index === -1) return;

        openFiles.value.splice(index, 1);

        if (activeFileId.value === id) {
            if (openFiles.value.length > 0) {
                activeFileId.value = openFiles.value[Math.max(0, index - 1)].id;
            } else {
                activeFileId.value = null;
            }
        }
    }

    function updateCode(newCode: string) {
        if (currentFile.value) {
            const original = examples.find(ex => ex.id === currentFile.value?.id);
            currentFile.value.code = newCode;
            if (original) {
                currentFile.value.isDirty = newCode !== original.code;
            }
        }
    }

    const moduleImports = ref<WebAssembly.ModuleImportDescriptor[]>([]);
    const moduleExports = ref<WebAssembly.ModuleExportDescriptor[]>([]);

    function generateHostForFile(file: OpenFile) {
        const info = extractWatTypes(file.code);
        file.hostCode = generateHostCode(info, file.filename);
        file.hostDirty = false;
    }

    function generateHost() {
        if (!currentFile.value) return;
        generateHostForFile(currentFile.value);
    }

    function updateHostCode(code: string) {
        if (currentFile.value) {
            currentFile.value.hostCode = code;
            currentFile.value.hostDirty = true;
        }
    }

    async function compile() {
        if (!currentFile.value) return false;
        isCompiling.value = true;
        log(`Starting compilation of ${currentFile.value.filename}...`, 'info');

        try {
            await ensureWasmTools();
            const bytes = parseWat(currentFile.value.code);
            wasmBytes.value = bytes;

            log(`Compiled successfully (${bytes.byteLength} bytes)`, 'success');
            const compiledModule = await WebAssembly.compile(bytes.buffer as ArrayBuffer);
            wasmModule.value = compiledModule;

            // Extract imports/exports
            moduleImports.value = WebAssembly.Module.imports(compiledModule);
            moduleExports.value = WebAssembly.Module.exports(compiledModule);

            // Auto-generate host code if not manually edited
            if (!currentFile.value.hostDirty) {
                generateHost();
            }

            return true;
        } catch (e: any) {
            log(`Compilation failed: ${e.message}`, 'error');
            return false;
        } finally {
            isCompiling.value = false;
        }
    }

    async function runHost() {
        if (!wasmBytes.value || !currentFile.value?.hostCode) return;
        isRunning.value = true;
        log('Running host code...', 'info');

        try {
            const lines: ConsoleLine[] = await runHostCode(wasmBytes.value, currentFile.value.hostCode);
            for (const line of lines) {
                const msg = line.args.join(' ');
                const typeMap: Record<string, string> = { log: 'log', info: 'info', warn: 'warn', error: 'error' };
                log(msg, typeMap[line.level] ?? 'log');
            }
            if (lines.length === 0) {
                log('Execution completed (no output)', 'info');
            }
        } catch (e: any) {
            log(`Runtime error: ${e.message}`, 'error');
        } finally {
            isRunning.value = false;
        }
    }

    function setSymbols(newSymbols: any[]) {
        symbols.value = newSymbols;
    }

    function setHoverInfo(info: { line: number; col: number; result: any } | null) {
        hoverInfo.value = info;
    }

    return {
        openFiles,
        activeFileId,
        currentFile,
        lspReady,
        lspStatus,
        consoleOutput,
        diagnostics,
        symbols,
        hoverInfo,
        moduleImports,
        moduleExports,
        wasmBytes,
        wasmModule,
        wasmInstance,
        isCompiling,
        isRunning,
        recentlyUsedIds,
        log,
        clearConsole,
        openFile,
        closeFile,
        updateCode,
        updateHostCode,
        generateHost,
        compile,
        runHost,
        setSymbols,
        setHoverInfo
    };
});
