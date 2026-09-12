// Ambient declarations for the host-provided `tur:rpc` module, which is NOT
// covered by the published `@tur-ng/*` packages. rspack leaves `tur:*` /
// `ease` imports as externals for the engine to resolve at runtime.
//
// Registration is split by WHO may call the op — the dispatcher routes
// strictly by the request envelope's scope, so an op registered in the wrong
// namespace is a "no host/view handler" error at the first call, never a
// silent half-serve.
//
// Ops and events are identified by SIG objects, not bare strings: a sig binds
// the wire name to its args/payload (and result) shapes, so plugin code never
// hand-declares arg types — the callback parameters infer from the sig.
// Host-contract sigs live in `plugins/infra/host-ops.ts` (ops) and
// `plugins/infra/events.ts` (events); plugin-private view ops get their own
// sigs in the plugin. The runtime ALSO accepts the legacy bare-string form —
// already-installed bundles built against the string API keep working.

declare module "tur:rpc" {
    /**
     * A host-event signal: binds the wire event `type` string to its payload
     * shape. The runtime value is just `{ type }` — `__payload` is a phantom
     * type marker that never exists at runtime.
     */
    export interface EaseRpcSig<Type extends string, Payload> {
        readonly type: Type;
        /** Phantom payload marker — never present at runtime. */
        readonly __payload?: Payload;
    }

    /** Extract the payload type bound on an event sig. */
    export type EaseRpcSigArg<S> = S extends EaseRpcSig<string, infer P>
        ? P
        : never;

    /**
     * An op signal: binds the wire `op` string to its args and result
     * shapes (for streams, the Result slot is the stream meta shape — see
     * {@link StreamSource}). The runtime value is just `{ op }` — `__args` /
     * `__result` are phantom type markers that never exist at runtime.
     */
    export interface EaseRpcOp<
        Op extends string,
        Args,
        Result = unknown,
    > {
        readonly op: Op;
        /** Phantom args marker — never present at runtime. */
        readonly __args?: Args;
        /** Phantom result marker — never present at runtime. */
        readonly __result?: Result;
    }

    /** Extract the args type bound on an op sig. */
    export type EaseRpcOpArg<S> = S extends EaseRpcOp<string, infer A, any>
        ? A
        : never;

    /** Extract the result type bound on an op sig. */
    export type EaseRpcOpResult<S> = S extends EaseRpcOp<string, any, infer R>
        ? R
        : never;

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
        /** Register a request/response handler for the sig's op, served when
         *  the host calls it (`RpcClient::call_host`). The callback's return
         *  value (or rejection error) is replied as JSON — its type is the
         *  sig's bound Result. */
        registerHandler<S extends EaseRpcOp<string, any, any>>(
            sig: S,
            fn: (
                args: EaseRpcOpArg<S>,
            ) => EaseRpcOpResult<S> | PromiseLike<EaseRpcOpResult<S>>,
        ): void;

        /** Register a streaming handler for the sig's op, served when the host
         *  calls `open_stream` (streams are host-only — views cannot open
         *  them). `open(args)` resolves to a {@link StreamSource} whose `meta`
         *  is typed by the sig's Result: it is replied to the host, then
         *  `body` is pumped chunk-by-chunk with host-granted credits —
         *  backpressure and cancellation are the dispatcher's job, plugin code
         *  never touches a stream id. An error thrown by `open` fails the call
         *  itself (single error path). */
        registerStream<S extends EaseRpcOp<string, any, any>>(
            sig: S,
            open: (args: EaseRpcOpArg<S>) => Promise<StreamSource<EaseRpcOpResult<S>>>,
        ): void;

        /** Subscribe to a host-fired plugin event (fire-and-forget, event
         *  bus channel 1 — see ease-tur-rpc's EVENT_CHANNEL_ID). The payload
         *  type is the sig's bound payload. */
        onEvent<S extends EaseRpcSig<string, unknown>>(
            sig: S,
            fn: (payload: EaseRpcSigArg<S>) => void,
        ): void;
    };

    /**
     * Ops this plugin's own VIEW may invoke via `ease.rpc.call` (add/edit
     * forms, disconnect buttons). Views are JSON request/response only — no
     * streams, no events — so there is no registerStream/onEvent here. An op
     * callable from both sides is simply registered in both namespaces.
     */
    export const viewRpc: {
        /** Register a request/response handler for the sig's op, served when
         *  the plugin's view calls `ease.rpc.call(sig, args)`. */
        registerHandler<S extends EaseRpcOp<string, any, any>>(
            sig: S,
            fn: (
                args: EaseRpcOpArg<S>,
            ) => EaseRpcOpResult<S> | PromiseLike<EaseRpcOpResult<S>>,
        ): void;
    };

    /**
     * What a `hostRpc.registerStream` opener resolves to. `M` is the stream's
     * meta shape — typed by the op sig's Result slot (loose by default).
     */
    export interface StreamSource<M = Record<string, unknown>> {
        /** Replied to the host before chunks flow, e.g.
         * `{ totalLength?, name?, contentType?, dataOffset? }` (the storage
         * contract lives in `ease-js-storage` / `plugins/infra/host-ops.ts`). */
        meta: M;
        /** Pull-driven chunk source — e.g.
         * `(await requestStream(...).promise).body`, passed through
         * unwrapped. */
        body: AsyncIterable<Uint8Array>;
        /** Release the underlying resource — wire it to `task.cancel()` on the
         *  `requestStream` Task. Called exactly once on every pump exit
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
