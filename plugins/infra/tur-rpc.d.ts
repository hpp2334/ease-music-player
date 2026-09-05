// Ambient declarations for the host-provided `tur:rpc` module, which is NOT
// covered by the published `@tur-ng/*` packages. rspack leaves `tur:*` /
// `ease` imports as externals for the engine to resolve at runtime.
//
// Registration is split by WHO may call the op — the dispatcher routes
// strictly by the request envelope's scope, so an op registered in the wrong
// namespace is a "no host/view handler" error at the first call, never a
// silent half-serve. Args types are declared at each call site via the
// generics — annotate the callback parameter with the op's concrete args
// shape.

declare module "tur:rpc" {
    /**
     * Everything the HOST may invoke on this backend — ops are contract
     * literals (identical names for every provider), identity riding the
     * payload (`pluginId` + `storageId`/`oauthId`):
     * - the storage contract — `storage:list` (handler) and `storage:get`
     *   (stream) called by the Rust storage service;
     * - the instance lifecycle — `storage:removeInstance`;
     * - the OAuth flow — `oauth:url` / `oauth:exchange` (the flow token
     *   `oauthId` comes from the host-minted `ease.oauth.new()`);
     * - host-fired events — `music:play` etc.
     */
    export const hostRpc: {
        /** Register a request/response handler for `op`, served when the
         *  host calls it (`RpcClient::call_host`). The callback's return
         *  value (or rejection error) is replied as JSON. */
        registerHandler<A>(op: string, fn: (args: A) => unknown): void;

        /** Register a streaming handler for `op`, served when the host calls
         *  `open_stream` (streams are host-only — views cannot open them).
         *  `open(args)` resolves to a {@link StreamSource}: its `meta` is
         *  replied to the host, then `body` is pumped chunk-by-chunk with
         *  host-granted credits — backpressure and cancellation are the
         *  dispatcher's job, plugin code never touches a stream id. An
         *  error thrown by `open` fails the call itself (single error
         *  path). */
        registerStream<A>(op: string, open: (args: A) => Promise<StreamSource>): void;

        /** Subscribe to a host-fired plugin event (fire-and-forget, event
         *  bus channel 1 — see ease-tur-rpc's EVENT_CHANNEL_ID). */
        onEvent<P>(type: string, fn: (payload: P) => void): void;
    };

    /**
     * Ops this plugin's own VIEW may invoke via `ease.rpc.call` (add/edit
     * forms, disconnect buttons). Views are JSON request/response only — no
     * streams, no events — so there is no registerStream/onEvent here. An op
     * callable from both sides is simply registered in both namespaces.
     */
    export const viewRpc: {
        /** Register a request/response handler for `op`, served when the
         *  plugin's view calls `ease.rpc.call(op, args)`. */
        registerHandler<A>(op: string, fn: (args: A) => unknown): void;
    };

    /** What a `hostRpc.registerStream` opener resolves to. */
    export interface StreamSource {
        /** Replied to the host before chunks flow, e.g.
         *  `{ totalLength?, name?, contentType?, dataOffset? }` (the storage
         *  contract lives in `ease-js-storage`). */
        meta: Record<string, unknown>;
        /** Pull-driven chunk source — e.g.
         *  `(await requestStream(...).promise).body`, passed through
         *  unwrapped. */
        body: AsyncIterable<Uint8Array>;
        /** Release the underlying resource — wire it to `task.cancel()` on
         *  the `requestStream` Task. Called exactly once on every pump exit
         *  (normal end, host cancel, error); a no-op on completion per
         *  `Task.cancel()` semantics. */
        release?: () => void;
        /** Optional mid-body error mapper — lets the plugin apply host-known
         *  error marking (e.g. the `TIMEOUT:`/`UNAUTHORIZED:` prefixes) to
         *  failures raised by the body itself. The opener path marks by
         *  throwing. */
        mapError?: (e: unknown) => Error;
    }
}
