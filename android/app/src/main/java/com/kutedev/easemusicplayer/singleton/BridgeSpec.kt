@file:Suppress("FunctionName")

package com.kutedev.easemusicplayer.singleton

import kotlinx.serialization.KSerializer
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.serializer

/**
 * Which opaque handle a [BridgeSpec]'s method expects.
 *
 * - [BACKEND] — the principal backend handle (data-layer methods).
 * - [PLAYER] — the cantode player handle (transport ops).
 * - [PLAYER_CONTEXT] — the cantode player-context handle.
 * - [NONE] — method takes no handle (e.g. `player.contextNew`).
 */
enum class HandleKind { BACKEND, PLAYER, PLAYER_CONTEXT, NONE }

/**
 * Typed declaration of a bridge method: its wire name, the type of
 * argument it accepts (or [Unit] for arg-less methods), the type of
 * payload it returns, and the handle kind it expects.
 *
 * Catalog entries live in [BridgeMethods]. Callers go through
 * [Bridge.call]; they never construct specs ad-hoc.
 *
 * The type parameters pin the call-site contract: a caller who writes
 * `bridge.call(BridgeMethods.Playlist.GET, id)` cannot accidentally pass
 * a `MusicId`, and cannot accidentally decode the result as a `Music` —
 * both errors are compile-time.
 */
sealed class BridgeSpec {
    abstract val name: String
    abstract val handleKind: HandleKind

    /** Method that takes no argument. */
    class NoArg<R>(
        override val name: String,
        internal val retSerializer: KSerializer<R>,
        override val handleKind: HandleKind = HandleKind.BACKEND,
    ) : BridgeSpec()

    /** Method with a single typed argument. */
    class WithArg<A, R>(
        override val name: String,
        internal val argSerializer: KSerializer<A>,
        internal val retSerializer: KSerializer<R>,
        override val handleKind: HandleKind = HandleKind.BACKEND,
    ) : BridgeSpec()
}

/** Reified factory for arg-less specs (avoids repeating `serializer()` at each call site). */
inline fun <reified R> bridgeSpecNoArg(
    name: String,
    handleKind: HandleKind = HandleKind.BACKEND,
): BridgeSpec.NoArg<R> = BridgeSpec.NoArg(name, serializer(), handleKind)

/** Reified factory for typed-arg specs. */
inline fun <reified A, reified R> bridgeSpecArg(
    name: String,
    handleKind: HandleKind = HandleKind.BACKEND,
): BridgeSpec.WithArg<A, R> = BridgeSpec.WithArg(name, serializer(), serializer(), handleKind)

/** True iff [s] is (or is the nullable variant of) `Unit.serializer()`. */
internal fun KSerializer<*>.isUnitSerializer(): Boolean =
    descriptor.serialName.removeSuffix("?") == "kotlin.Unit"
