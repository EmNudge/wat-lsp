import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { examples, getDefaultExample } from '../../examples';
import wabtInit from 'wabt';

export interface OpenFile {
    id: string;
    filename: string;
    code: string;
    isDirty: boolean;
}

export const usePlaygroundStore = defineStore('playground', () => {
    const openFiles = ref<OpenFile[]>([]);
    const activeFileId = ref<string | null>(null);
    const lspReady = ref(false);
    const lspStatus = ref('LSP Loading...');
    const consoleOutput = ref<{ message: string; type: string; timestamp: string }[]>([]);
    const diagnostics = ref<any[]>([]);
    const symbols = ref<any[]>([]);

    const wasmBytes = ref<Uint8Array | null>(null);
    const wasmModule = ref<WebAssembly.Module | null>(null);
    const wasmInstance = ref<WebAssembly.Instance | null>(null);
    const isCompiling = ref(false);
    const recentlyUsedIds = ref<string[]>([]);

    let wabt: any = null;

    async function initWabt() {
        if (!wabt) {
            wabt = await (wabtInit as any)();
        }
        return wabt;
    }

    const currentFile = computed(() =>
        openFiles.value.find(f => f.id === activeFileId.value)
    );

    // Initial load
    const defaultEx = getDefaultExample();
    openFiles.value = [{
        id: defaultEx.id,
        filename: defaultEx.filename,
        code: defaultEx.code,
        isDirty: false
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
            const current = currentFile.value;
            if (current && !current.isDirty) {
                // Replace current unedited tab
                const index = openFiles.value.findIndex(f => f.id === current.id);
                if (index !== -1) {
                    openFiles.value[index] = {
                        id: example.id,
                        filename: example.filename,
                        code: example.code,
                        isDirty: false
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
                isDirty: false
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

    async function compile() {
        if (!currentFile.value) return false;
        isCompiling.value = true;
        log(`Starting compilation of ${currentFile.value.filename}...`, 'info');

        try {
            const wabt = await initWabt();
            const module = wabt.parseWat(currentFile.value.filename, currentFile.value.code, {
                bulk_memory: true,
                exceptions: true,
                gc: true,
                multi_value: true,
                mutable_globals: true,
                reference_types: true,
                saturating_float_to_int: true,
                sign_extension: true,
                simd: true,
                tail_call: true,
            });

            module.validate();
            const result = module.toBinary({ log: false, write_debug_names: true });
            wasmBytes.value = result.buffer;

            if (wasmBytes.value) {
                log(`Compiled successfully (${wasmBytes.value.byteLength} bytes)`, 'success');
                const compiledModule = await WebAssembly.compile(wasmBytes.value.buffer as ArrayBuffer);
                wasmModule.value = compiledModule;

                // Extract imports/exports
                moduleImports.value = WebAssembly.Module.imports(compiledModule);
                moduleExports.value = WebAssembly.Module.exports(compiledModule);
            }
            module.destroy();
            return true;
        } catch (e: any) {
            log(`Compilation failed: ${e.message}`, 'error');
            return false;
        } finally {
            isCompiling.value = false;
        }
    }

    function setSymbols(newSymbols: any[]) {
        symbols.value = newSymbols;
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
        moduleImports,
        moduleExports,
        wasmBytes,
        wasmModule,
        wasmInstance,
        isCompiling,
        recentlyUsedIds,
        log,
        clearConsole,
        openFile,
        closeFile,
        updateCode,
        compile,
        setSymbols
    };
});
