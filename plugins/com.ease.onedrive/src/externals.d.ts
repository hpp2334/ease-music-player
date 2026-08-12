// Ambient declarations for host-provided JS modules that are NOT covered by
// the published `@tur-ng/*` packages. `tur:std` / `tur:net` types come from
// the tsconfig `paths` mapping to `@tur-ng/std` / `@tur-ng/net`; rspack leaves
// all `tur:*` / `ease` imports as externals for the engine to resolve.
//
// The unified `ease` module (storage / secret / oauth / themes namespaces)
// is declared in `plugins/infra/ease.d.ts` at the repo root; tsconfig
// includes it via the `include` glob.

declare module "tur:rpc" {
    export function registerHandler(op: string, fn: (args: any) => any): void;
    export function pushChunk(streamId: number, bytes: Uint8Array): void;
    export function endStream(streamId: number): void;
    export function errorStream(streamId: number, message: string): void;
}
