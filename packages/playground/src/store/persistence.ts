// Local storage persistence for playground editor contents

const STORAGE_PREFIX = 'wat-playground';

interface SavedEdit {
    sha: string;    // Hash of the original content (for cache invalidation)
    content: string; // User's edited content
}

/** Fast 53-bit hash (cyrb53) for content fingerprinting */
export function hashContent(str: string): string {
    let h1 = 0xdeadbeef, h2 = 0x41c6ce57;
    for (let i = 0; i < str.length; i++) {
        const ch = str.charCodeAt(i);
        h1 = Math.imul(h1 ^ ch, 2654435761);
        h2 = Math.imul(h2 ^ ch, 1597334677);
    }
    h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507);
    h1 ^= Math.imul(h2 ^ (h2 >>> 13), 3266489909);
    h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507);
    h2 ^= Math.imul(h1 ^ (h1 >>> 13), 3266489909);
    return (4294967296 * (2097151 & h2) + (h1 >>> 0)).toString(36);
}

function storageKey(fileId: string, type: 'wat' | 'host'): string {
    return `${STORAGE_PREFIX}:${fileId}:${type}`;
}

/** Save edited content to localStorage with the SHA of the original */
export function saveEdit(fileId: string, type: 'wat' | 'host', content: string, originalSha: string): void {
    try {
        const data: SavedEdit = { sha: originalSha, content };
        localStorage.setItem(storageKey(fileId, type), JSON.stringify(data));
    } catch {
        // localStorage full or unavailable
    }
}

/** Load saved content if the original SHA matches; returns null if no save or SHA mismatch */
export function loadEdit(fileId: string, type: 'wat' | 'host', originalSha: string): string | null {
    try {
        const raw = localStorage.getItem(storageKey(fileId, type));
        if (!raw) return null;
        const data: SavedEdit = JSON.parse(raw);
        if (data.sha !== originalSha) {
            // Original content has changed — discard stale edit
            localStorage.removeItem(storageKey(fileId, type));
            return null;
        }
        return data.content;
    } catch {
        return null;
    }
}

/** Clear saved edit for a file */
export function clearEdit(fileId: string, type: 'wat' | 'host'): void {
    try {
        localStorage.removeItem(storageKey(fileId, type));
    } catch {
        // silently fail
    }
}
