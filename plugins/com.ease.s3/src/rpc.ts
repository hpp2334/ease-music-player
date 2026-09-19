// Plugin-private RPC sigs — the single source of this plugin's view↔backend
// op contracts (mirrors the WebDAV plugin's `rpc.ts`). Both `backend.ts`
// (viewRpc registrations) and `view.ts` (`ease.rpc.call`) import from here,
// so the args/result shapes are declared exactly once.

import type { EaseRpcOp } from "tur:rpc";

/** `s3:test` outcome — drives the connect form's status text. */
export type TestOutcome = "SUCCESS" | "UNAUTHORIZED" | "TIMEOUT" | "OTHER_ERROR";

/** Connection fields to test (collected by the view's form). */
export interface S3TestArgs {
    storageId?: string;
    endpoint: string;
    region?: string;
    bucket: string;
    accessKeyId: string;
    /** Blank on edit = "use the stored secret". */
    secretAccessKey?: string;
}

/** Connect-form fields (create when `storageId` is absent, update when set). */
export interface S3ConnectArgs {
    storageId?: string;
    endpoint: string;
    region?: string;
    bucket: string;
    alias?: string;
    accessKeyId: string;
    /** Blank on update keeps the stored secret; required on create. */
    secretAccessKey?: string;
}

export const S3TestSig: EaseRpcOp<
    "s3:test",
    S3TestArgs,
    { result: TestOutcome }
> = { op: "s3:test" };

export const S3ConnectSig: EaseRpcOp<
    "s3:connect",
    S3ConnectArgs,
    { storageId: string; created: boolean }
> = { op: "s3:connect" };
