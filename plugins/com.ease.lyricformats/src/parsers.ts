// Pure lyric-format parsers — no `tur:*` / `ease` imports in this module so
// it is unit-testable under plain Node (`pnpm test` -> scripts/selftest.ts).
//
// The three grammars (LRC, SRT, WebVTT) are implemented by a small
// hand-written lexer/parser: character scanning only, zero regex literals.
// (Earlier takes, kept in git history for reference: a regex-based port of
// the app's old Rust lrc-nom parser, then delegation to the `lrc-kit` /
// `@plussub/srt-vtt-parser` npm libs.)
//
// Semantics:
//   - LRC: info tags (`[ti:…]`, `[offset:…]`, …) pass through as metadata
//     and are never applied (`[offset:+500]` stays a verbatim string).
//     Multi-timestamp lines expand to one entry per timestamp; the first
//     tag decides the line kind, one info tag per line. Enhanced-LRC word
//     timings (`<mm:ss.xx> word`, plus the Foobar2000 `[mm:ss.xx] word`
//     shape) are stripped from the text. `[:]` is a comment; unknown tags
//     and untagged lines are skipped (lenient). Fractions scale by digit
//     count (`.5` = `.50` = `.500` = 500 ms, longer runs truncate). No
//     timed lines -> null ("not mine").
//   - SRT/WebVTT: cue start times; a parseable cue end becomes the
//     per-line `durationMs`. VTT NOTE/STYLE/REGION blocks are skipped, as
//     are optional cue id lines (SRT index / VTT identifier). Multi-line
//     cue text joins with a space; inline markup and `&nbsp;` are
//     stripped. Timestamps tolerate 2 or 3 fields, the SRT comma or the
//     VTT dot, and 1-3 fraction digits. No cues -> null.

// ---------------------------------------------------------------------------
// Contract types (host ↔ plugin) — canonical copies live in
// `../../infra/host-ops.ts` (bound on `LyricParseSig`); re-exported here for
// the parser implementations below.
// ---------------------------------------------------------------------------

import type {
    LyricLine,
    LyricMetadata,
    LyricParseResult,
} from "../../infra/host-ops";

export type { LyricLine, LyricMetadata, LyricParseResult };

export type Parser = (text: string) => LyricParseResult | null;

// ---------------------------------------------------------------------------
// Character utilities — the lexer toolkit (regex-free by design)
// ---------------------------------------------------------------------------

function isDigits(s: string): boolean {
    if (s === "") return false;
    for (let i = 0; i < s.length; i += 1) {
        const c = s.charCodeAt(i);
        if (c < 0x30 || c > 0x39) return false;
    }
    return true;
}

/** Split on `\n`, `\r\n` or a lone `\r` (every line terminator in the wild). */
function splitLines(text: string): string[] {
    const out: string[] = [];
    for (const chunk of text.split("\n")) {
        if (chunk.includes("\r")) {
            for (const piece of chunk.split("\r")) out.push(piece);
        } else {
            out.push(chunk);
        }
    }
    return out;
}

/** Drop a leading UTF-8 BOM — it would poison LRC's first-line tags and
 *  defeat the `WEBVTT` header sniffing. */
function stripBom(text: string): string {
    return text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
}

/** Collapse whitespace runs to single spaces and trim both ends. */
function collapseSpaces(text: string): string {
    let out = "";
    let pending = false;
    for (const c of text) {
        if (c === " " || c === "\t" || c === "\n" || c === "\r") {
            pending = out !== "";
        } else {
            out += pending ? " " + c : c;
            pending = false;
        }
    }
    return out;
}

/**
 * Parse one `[hh:]mm:ss[,.]frac` timestamp into whole milliseconds.
 * Accepts the SRT comma and the VTT/LRC dot, 2 or 3 fields, and scales the
 * fraction by its digit count. Returns null when not well-formed — callers
 * use that both to reject and to classify (`parseLrc` distinguishes time
 * tags from info tags this way).
 */
function parseTimestamp(str: string): number | null {
    const parts = str.trim().split(":");
    if (parts.length < 2 || parts.length > 3) return null;
    const head = parts.slice(0, parts.length - 1); // [hh?,] mm
    const tail = parts[parts.length - 1]!; // ss[,.]frac
    const sep = Math.max(tail.lastIndexOf(","), tail.lastIndexOf("."));
    if (sep < 0) return null;
    const sec = tail.slice(0, sep);
    const frac = tail.slice(sep + 1);
    if (!isDigits(sec) || !isDigits(frac)) return null;
    for (const p of head) {
        if (!isDigits(p)) return null;
    }
    let ms = Number(sec) * 1000;
    const f = frac.slice(0, 3);
    ms += Number(f) * (f.length === 1 ? 100 : f.length === 2 ? 10 : 1);
    ms += Number(head[head.length - 1]) * 60_000;
    if (head.length === 2) ms += Number(head[0]) * 3_600_000;
    return ms;
}

// ---------------------------------------------------------------------------
// LRC
// ---------------------------------------------------------------------------

// LRC info-tag key -> contract metadata key (`length` / `offset` pass
// through verbatim — the host keeps them as strings, exactly like the old
// Rust parser did).
const LRC_META_KEYS: Record<string, keyof LyricMetadata> = {
    ar: "artist",
    al: "album",
    ti: "title",
    au: "lyricist",
    by: "author",
};

/** Lexer: consecutive `[inner]` groups from the start of a line. */
function takeLeadingTags(line: string): { tags: string[]; rest: string } {
    const tags: string[] = [];
    let rest = line;
    while (rest.startsWith("[")) {
        const close = rest.indexOf("]");
        if (close < 0) break; // unclosed bracket — not a tag
        tags.push(rest.slice(1, close));
        rest = rest.slice(close + 1);
    }
    return { tags, rest };
}

/**
 * Strip enhanced-LRC word timings from line text: the A2 extension
 * `<mm:ss.xx> word` and the Foobar2000 `[mm:ss.xx] word` shape. Bracket
 * groups whose content is not a timestamp pass through verbatim.
 */
function stripWordTags(text: string): string {
    let out = "";
    let i = 0;
    while (i < text.length) {
        const c = text[i]!;
        if (c === "<" || c === "[") {
            const close = c === "<" ? ">" : "]";
            const end = text.indexOf(close, i + 1);
            if (end > i + 1 && parseTimestamp(text.slice(i + 1, end)) !== null) {
                i = end + 1;
                continue;
            }
        }
        out += c;
        i += 1;
    }
    return out;
}

export function parseLrc(text: string): LyricParseResult | null {
    const meta: Record<string, string> = {};
    const lines: LyricLine[] = [];
    for (const rawLine of splitLines(stripBom(text))) {
        const line = rawLine.trim();
        if (line === "") continue;
        const { tags, rest } = takeLeadingTags(line);
        if (tags.length === 0) continue; // untagged line — lenient skip
        const first = tags[0]!;
        if (parseTimestamp(first) !== null) {
            // Timed line: every leading tag is a timestamp candidate, each
            // seeding one entry that carries the word-tag-stripped text.
            const lineText = stripWordTags(rest).trim();
            if (lineText === "") continue;
            for (const tag of tags) {
                const ms = parseTimestamp(tag);
                if (ms !== null) lines.push({ timeMs: ms, text: lineText });
            }
            continue;
        }
        // Info line: `[key:value]` — the first tag decides, one per line.
        const colon = first.indexOf(":");
        if (colon < 0) continue; // e.g. `[00]` — not valid info either
        const key = first.slice(0, colon).trim();
        const value = first.slice(colon + 1).trim();
        if (key === "" && value === "") continue; // `[:]` comment
        const mapped = LRC_META_KEYS[key];
        if (mapped) meta[mapped] = value;
        else if (key === "length" || key === "offset") meta[key] = value;
        // Unrecognized tag — ignored.
    }
    if (lines.length === 0) return null; // nothing timed parsed — "not mine"
    lines.sort((a, b) => a.timeMs - b.timeMs);
    const result: LyricParseResult = { lines };
    if (Object.keys(meta).length > 0) result.metadata = meta as LyricMetadata;
    return result;
}

// ---------------------------------------------------------------------------
// SRT / WebVTT
// ---------------------------------------------------------------------------

function isArrowLine(line: string): boolean {
    return line.includes("-->");
}

/** First whitespace-delimited token — the cue-end timestamp; VTT cue
 *  settings (`align:start line:90%`) and SRT legacy coordinates follow. */
function firstToken(s: string): string {
    const t = s.trimStart();
    let end = t.length;
    for (const ws of [" ", "\t"]) {
        const k = t.indexOf(ws);
        if (k >= 0 && k < end) end = k;
    }
    return t.slice(0, end);
}

/** `from --> to [settings]` -> `{ from, to }`; `to` is null when the end
 *  timestamp is unparseable, `from` null rejects the whole cue. */
function parseTimingLine(line: string): { from: number; to: number | null } | null {
    const arrow = line.indexOf("-->");
    const from = parseTimestamp(line.slice(0, arrow));
    if (from === null) return null;
    const to = parseTimestamp(firstToken(line.slice(arrow + 3)));
    return { from, to };
}

function isVttBlockKeyword(line: string): boolean {
    const t = line.trim().toUpperCase();
    return (
        t === "NOTE" ||
        t === "STYLE" ||
        t === "REGION" ||
        t.startsWith("NOTE ") ||
        t.startsWith("STYLE ") ||
        t.startsWith("REGION ")
    );
}

/** Strip VTT/SRT inline markup (`<i>`, `<b>`, `<c.className>`, `<v Speaker>`…).
 *  A `<` with no closing `>` (e.g. the lyric `love <3`) passes through. */
function stripAngleTags(text: string): string {
    let out = "";
    let i = 0;
    while (i < text.length) {
        const c = text[i]!;
        if (c === "<") {
            const end = text.indexOf(">", i + 1);
            if (end > i + 1) {
                i = end + 1;
                continue;
            }
        }
        out += c;
        i += 1;
    }
    return out;
}

function stripCueTags(text: string): string {
    // `&nbsp;` -> plain space first, so entity spaces collapse with the rest.
    return collapseSpaces(stripAngleTags(text.split("&nbsp;").join(" ")));
}

export function parseSubtitle(text: string): LyricParseResult | null {
    const rows = splitLines(stripBom(text));
    const isVtt = rows.length > 0 && rows[0]!.startsWith("WEBVTT");
    const cues: Array<{ from: number; to: number | null; text: string }> = [];
    let i = 0;
    if (isVtt) {
        // Skip the header block (`WEBVTT` line + optional metadata lines)
        // up to the first blank line.
        while (i < rows.length && rows[i]!.trim() !== "") i += 1;
    }
    while (i < rows.length) {
        const line = rows[i]!;
        if (line.trim() === "") {
            i += 1;
            continue;
        }
        if (isVtt && isVttBlockKeyword(line)) {
            i += 1;
            while (i < rows.length && rows[i]!.trim() !== "") i += 1;
            continue;
        }
        // A cue starts at its timing line; an optional id line (SRT index /
        // VTT identifier) may precede it. Cue text never contains `-->`,
        // so the arrow is a reliable anchor.
        let timing: { from: number; to: number | null } | null = null;
        if (isArrowLine(line)) {
            timing = parseTimingLine(line);
        } else if (i + 1 < rows.length && isArrowLine(rows[i + 1]!)) {
            timing = parseTimingLine(rows[i + 1]!);
            i += 1;
        }
        if (timing === null) {
            i += 1; // resync: scan forward for the next cue
            continue;
        }
        i += 1;
        const textRows: string[] = [];
        while (i < rows.length && rows[i]!.trim() !== "" && !isArrowLine(rows[i]!)) {
            textRows.push(rows[i]!);
            i += 1;
        }
        cues.push({ from: timing.from, to: timing.to, text: textRows.join(" ") });
    }
    const lines: LyricLine[] = [];
    for (const cue of cues) {
        const lineText = stripCueTags(cue.text);
        if (lineText === "") continue;
        const entry: LyricLine = { timeMs: cue.from, text: lineText };
        // Cue end -> per-line duration (optional in the contract; the host
        // currently derives it from the next line's start and ignores ours,
        // but it is free to send and correct for sparse subtitles).
        if (cue.to !== null && cue.to > cue.from) entry.durationMs = cue.to - cue.from;
        lines.push(entry);
    }
    if (lines.length === 0) return null;
    lines.sort((a, b) => a.timeMs - b.timeMs);
    return { lines };
}
