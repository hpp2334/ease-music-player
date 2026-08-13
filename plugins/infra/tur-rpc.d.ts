// Ambient declarations for the host-provided `tur:rpc` module, which is NOT
// covered by the published `@tur-ng/*` packages. rspack leaves `tur:*` /
// `ease` imports as externals for the engine to resolve at runtime.

declare module "tur:rpc" {
    export function registerHandler(op: string, fn: (args: any) => any): void;
    export function pushChunk(streamId: number, bytes: Uint8Array): void;
    export function endStream(streamId: number): void;
    export function errorStream(streamId: number, message: string): void;
}
