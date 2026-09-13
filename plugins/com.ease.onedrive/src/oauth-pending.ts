// OAuth pending-slot helpers — shared by view.ts (writes the slot before
// `ease.oauth.start`) and backend.ts (consumes it in the `oauth:exchange`
// handler). The host never sees any of this: business data keyed by the
// host-minted flow id lives entirely in the plugin's KV.

/** KV key for one in-flight OAuth flow's pending data. */
export function pendingKey(oauthId: string): string {
    return `oauth:${oauthId}`;
}

export interface PendingFlow {
    /** Display alias the user typed in the connect form (null = default). */
    alias: string | null;
}

/** Read (without consuming) a flow's pending slot, or null if absent. */
export function readPending(db: Db, oauthId: string): PendingFlow | null {
    const raw = db.singleGet(pendingKey(oauthId));
    if (raw == null) return null;
    try {
        return JSON.parse(raw) as PendingFlow;
    } catch {
        return null;
    }
}

/** Consume a flow's pending slot: read + delete. Returns null if absent. */
export function takePending(db: Db, oauthId: string): PendingFlow | null {
    const pending = readPending(db, oauthId);
    db.singleDelete(pendingKey(oauthId));
    return pending;
}

// Minimal structural type for the `ease` db namespace (avoids importing the
// ambient `ease` module here, which would couple this helper to the host at
// type time — both view and backend instances satisfy this shape).
interface Db {
    singleGet(key: string): string | null;
    singleDelete(key: string): void;
}
