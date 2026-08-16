// Ambient declarations for the host-provided `tur:rpc` module, which is NOT
// covered by the published `@tur-ng/*` packages. rspack leaves `tur:*` /
// `ease` imports as externals for the engine to resolve at runtime.

declare module "tur:rpc" {
    export function registerHandler(op: string, fn: (args: any) => any): void;
    /** Subscribe to a host-fired plugin event (fire-and-forget, event bus
     * channel 1 — see ease-tur-rpc's EVENT_CHANNEL_ID). */
    export function onEvent(type: string, fn: (payload: any) => void): void;
    export function pushChunk(streamId: number, bytes: Uint8Array): void;
    export function endStream(streamId: number): void;
    export function errorStream(streamId: number, message: string): void;
}
