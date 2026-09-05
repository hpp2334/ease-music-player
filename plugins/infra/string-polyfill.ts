// ECMAScript Annex-B / legacy builtin polyfill for the tur (boa) runtime.
//
// boa skips deprecated Annex-B extras that real-world npm packages still
// call. Import this module FIRST from every bundle entry (side-effect
// import), before any dependency that might rely on these.
//
// Found the hard way: fast-xml-parser's `readTagExp` calls
// `String.prototype.substr` on every namespace-prefixed tag — with
// `removeNSPrefix: true` that is EVERY tag of a WebDAV multistatus body —
// and the resulting TypeError (swallowed by a `catch { return [] }` at the
// time) turned every WebDAV `storage:list` into a silent empty directory
// on device.

// --- String.prototype.substr (Annex B; absent in boa) ----------------------
//
// Spec semantics: NaN start → 0; negative start counts from the end;
// start >= length → ""; omitted length → to the end; NaN / negative / zero
// length → "". Installed via defineProperty so the lib's (deprecated) TS
// declaration is untouched.

if (typeof String.prototype.substr !== "function") {
    Object.defineProperty(String.prototype, "substr", {
        value: function substr(this: string, start: number, length?: number): string {
            const s = String(this);
            const size = s.length;
            let begin = Number(start);
            if (Number.isNaN(begin)) begin = 0;
            if (begin < 0) begin = Math.max(size + begin, 0);
            if (begin >= size) return "";
            let end = size;
            if (length !== undefined) {
                const len = Number(length);
                if (Number.isNaN(len) || len <= 0) return "";
                end = Math.min(begin + len, size);
            }
            return s.substring(begin, end);
        },
        writable: true,
        configurable: true,
        enumerable: false,
    });
}

export {};
