// Plugin-private RPC sigs — the single source of this plugin's view↔backend
// op contracts. Both `backend.ts` (viewRpc registrations) and `view.ts`
// (`ease.rpc.call`) import from here, so the args/result shapes are declared
// exactly once and neither side re-declares or casts them. The sigs bind the
// op names to their shapes; the runtime value is just `{ op }`.

import type { EaseRpcOp } from "tur:rpc";

/** `webdav:test` outcome — drives the connect form's status text. */
export type TestOutcome = "SUCCESS" | "UNAUTHORIZED" | "TIMEOUT" | "OTHER_ERROR";

/** Credentials to test (collected by the view's form). */
export interface WebdavTestArgs {
    storageId?: string;
    addr: string;
    username: string;
    password?: string;
    isAnonymous?: boolean;
}

/** Connect-form fields (create when `storageId` is absent, update when set). */
export interface WebdavConnectArgs {
    storageId?: string;
    addr: string;
    alias?: string;
    username?: string;
    password?: string;
    isAnonymous?: boolean;
}

export const WebdavTestSig: EaseRpcOp<
    "webdav:test",
    WebdavTestArgs,
    { result: TestOutcome }
> = { op: "webdav:test" };

export const WebdavConnectSig: EaseRpcOp<
    "webdav:connect",
    WebdavConnectArgs,
    { storageId: string; created: boolean }
> = { op: "webdav:connect" };
