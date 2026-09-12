// Host event sigs — the typed catalog of events the app fires at plugin
// backends (`hostRpc.onEvent(sig, …)`). Each sig binds the wire event type
// string to its payload shape, so plugins subscribe without hand-declaring
// payload types; extract the shape elsewhere via
// `EaseRpcSigArg<typeof MusicPlaySig>`.
//
// SOURCE OF TRUTH: the payload shapes mirror `PluginEvent.toJsonElement()` in
// `android/app/src/main/java/com/kutedev/easemusicplayer/singleton/PluginEvent.kt`
// — update both sides together. `pluginId` never appears here: the host fans
// each event out to the per-plugin bus of every plugin whose manifest
// declares the event `type`.
//
// This module is plain bundled TS (imported relatively by plugins, like the
// polyfills): the runtime value of a sig is just `{ type }`, so no host
// provisioning is involved.

import type { EaseRpcSig } from "tur:rpc";

/**
 * `music:play` — fired after a track is freshly loaded and playback starts
 * (`player.loadMusic` + `player.play`). Never fires for resuming an
 * already-loaded track — that is `MusicResumeSig`.
 */
export const MusicPlaySig: EaseRpcSig<
    "music:play",
    {
        musicId: number;
        title: string;
        /** Wall-clock milliseconds (System.currentTimeMillis()). */
        ts: number;
    }
> = { type: "music:play" };

/** `music:pause` — fired when the user (or sleep timer) pauses playback. */
export const MusicPauseSig: EaseRpcSig<
    "music:pause",
    {
        /** Absent when no track is loaded (the Kotlin side omits the key). */
        musicId?: number;
        ts: number;
        positionMs: number;
    }
> = { type: "music:pause" };

/**
 * `music:resume` — fired when paused playback resumes on the already-loaded
 * track (the counterpart of `MusicPauseSig`).
 */
export const MusicResumeSig: EaseRpcSig<
    "music:resume",
    {
        /** Absent when no track is loaded (the Kotlin side omits the key). */
        musicId?: number;
        ts: number;
        positionMs: number;
    }
> = { type: "music:resume" };

/** `music:stop` — fired when playback is stopped (current music cleared). */
export const MusicStopSig: EaseRpcSig<"music:stop", { ts: number }> = {
    type: "music:stop",
};

/**
 * `music:complete` — fired when the current track finishes naturally
 * (STATE_ENDED).
 */
export const MusicCompleteSig: EaseRpcSig<
    "music:complete",
    {
        musicId: number;
        title: string;
        ts: number;
    }
> = { type: "music:complete" };
