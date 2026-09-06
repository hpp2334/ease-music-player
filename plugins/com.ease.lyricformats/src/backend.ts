// Lyric-format parsers — a headless JS plugin serving the app's entire
// lyric parsing (there is no built-in Rust parser). One `lyric:parse`
// host-RPC handler serves every `contributions.lyricParsers` entry of
// this manifest; `parserId` (the contribution id) rides the payload and
// routes to the format implementation:
//
// hostRpc — the Rust host invokes this (services/lyrics/dispatch.rs):
//   - lyric:parse  { pluginId, parserId, fileName, size, contentBase64 }
//     -> { lines: [{ timeMs, durationMs?, text }], metadata? } | null
//     `null` = "not mine / unrecognized content" — the host falls through
//     to the next parser claiming the extension.
//
// Formats (implementations in ./parsers.ts — a hand-written, regex-free
// lexer/parser, no parsing dependencies):
//   - lrc      LRC (metadata tags pass through unapplied; enhanced-LRC
//              word timings stripped from the text)
//   - subtitle SRT + WebVTT (cue start times, plus per-line durationMs
//              from the cue end)
//
// All results are pre-sorted by time; the host clamps, rounds, caps and
// sorts again (defensive on both sides).

// TextEncoder/TextDecoder polyfill FIRST — npm deps below may rely on them.
import "../../infra/string-polyfill";
import "../../infra/text-polyfill";
import { hostRpc } from "tur:rpc";
import { Base64 } from "js-base64";
import { parseLrc, parseSubtitle } from "./parsers";
import type { LyricParseResult, Parser } from "./parsers";

interface LyricParseArgs {
    pluginId: string;
    parserId: string;
    fileName: string;
    size: number;
    contentBase64: string;
}

const PARSERS: Record<string, Parser> = {
    lrc: parseLrc,
    subtitle: parseSubtitle,
};

export function start(): void {
    hostRpc.registerHandler("lyric:parse", (args: LyricParseArgs) => {
        const parser = PARSERS[args.parserId];
        if (!parser) return null;
        const bytes = Base64.toUint8Array(args.contentBase64);
        const text = new TextDecoder().decode(bytes);
        // A parse error thrown here rejects the RPC call host-side (logged,
        // next parser tried); `null` means "unrecognized content" with the
        // same fall-through.
        const result: LyricParseResult | null = parser(text);
        return result;
    });
}
