// Ambient declarations for the host-provided JS modules this plugin imports.
// Resolved at runtime by the tur engine (rspack treats them as externals).

declare module "tur:net" {
    export interface RequestOptions {
        url: string;
        method?: string;
        headers?: Record<string, string>;
        body?: string | ArrayBuffer;
        responseType?: "text" | "bytes";
        username?: string;
        password?: string;
    }
    export interface Response {
        ok: boolean;
        status: number;
        statusText: string;
        headers: Record<string, string>;
        bodyText?: string;
        bodyBytes?: ArrayBuffer;
    }
    export interface StreamResponse {
        ok: boolean;
        status: number;
        statusText: string;
        headers: Record<string, string>;
        body: AsyncIterable<Uint8Array>;
    }
    export function request(opts: RequestOptions): Promise<Response>;
    export function requestStream(opts: RequestOptions): Promise<StreamResponse>;
}

declare module "tur:rpc" {
    export function registerHandler(op: string, fn: (args: any) => any): void;
    export function pushChunk(streamId: number, bytes: Uint8Array): void;
    export function endStream(streamId: number): void;
    export function errorStream(streamId: number, message: string): void;
}
