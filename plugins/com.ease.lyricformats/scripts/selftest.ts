// Node-runnable selftest for src/parsers.ts (no tur runtime involved —
// `parsers.ts` is deliberately dependency-pure besides the format libs).
// Run: pnpm test   (node executes .ts via native type stripping)

import assert from "node:assert/strict";
import { parseLrc, parseSubtitle } from "../src/parsers.ts";

let passed = 0;

function test(name: string, fn: () => void) {
    fn();
    passed += 1;
    console.log(`  ok  ${name}`);
}

// ---------------------------------------------------------------------------
// LRC
// ---------------------------------------------------------------------------

test("LRC: lines, sort, metadata; offset stored NOT applied", () => {
    const r = parseLrc(
        "[ti:Ease Test]\n[ar:artist]\n[offset:+500]\n[00:03.00]b\n[00:01.00]a\n",
    )!;
    assert.equal(r.lines.length, 2);
    assert.equal(r.lines[0]!.timeMs, 1000);
    assert.equal(r.lines[0]!.text, "a");
    assert.equal(r.lines[1]!.timeMs, 3000);
    assert.equal(r.metadata!.title, "Ease Test");
    assert.equal(r.metadata!.artist, "artist");
    assert.equal(r.metadata!.offset, "+500"); // verbatim, unapplied
});

test("LRC: multi-timestamp line expands; fraction digit-count scales", () => {
    const r = parseLrc("[00:10.5][01:00.50]chorus\n")!;
    assert.equal(r.lines.length, 2);
    assert.equal(r.lines[0]!.timeMs, 10_500); // .5 = 500 ms
    assert.equal(r.lines[1]!.timeMs, 60_500); // .50 = 500 ms
});

test("LRC: enhanced word tags stripped from text", () => {
    const r = parseLrc("[00:01.00] <00:01.20> hel<00:01.40>lo world\n")!;
    assert.equal(r.lines.length, 1);
    assert.equal(r.lines[0]!.text, "hello world");
});

test("LRC: `[:]` comment and unknown tags ignored", () => {
    const r = parseLrc("[:]\n[xx:yy]\n[00:01.00]a\n")!;
    assert.equal(r.lines.length, 1);
    assert.equal(r.metadata, undefined);
});

test("LRC: BOM tolerated", () => {
    const r = parseLrc("\uFEFF[00:01.00]a\n")!;
    assert.equal(r.lines[0]!.text, "a");
});

test("LRC: no timed lines -> null (not mine)", () => {
    assert.equal(parseLrc("just some plain text\nmore\n"), null);
    assert.equal(parseLrc("[ti:only metadata]\n"), null);
    assert.equal(parseLrc(""), null);
});

test("LRC: metadata au/by/length mapping", () => {
    const r = parseLrc("[au:someone]\n[by:other]\n[length:03:30]\n[00:01.00]a\n")!;
    assert.equal(r.metadata!.lyricist, "someone");
    assert.equal(r.metadata!.author, "other");
    assert.equal(r.metadata!.length, "03:30");
});

test("LRC: foobar-style inline [mm:ss] stripped from text", () => {
    const r = parseLrc("[00:01.00]a [00:02.00]b\n")!;
    assert.equal(r.lines[0]!.text, "a b");
});

test("LRC: literal '<3' survives; short fractions scale", () => {
    const r = parseLrc("[00:01.5]love <3\n")!; // .5 -> 500 ms
    assert.equal(r.lines[0]!.timeMs, 1500);
    assert.equal(r.lines[0]!.text, "love <3");
});

// ---------------------------------------------------------------------------
// SRT / WebVTT
// ---------------------------------------------------------------------------

test("SRT: cues parsed, durationMs from cue end, markup + multi-line collapsed", () => {
    const r = parseSubtitle(
        "1\n00:00:01,000 --> 00:00:03,000\n<i>first</i>\nline two\n\n2\n00:00:05,000 --> 00:00:07,500\nsecond\n",
    )!;
    assert.equal(r.lines.length, 2);
    assert.equal(r.lines[0]!.timeMs, 1000);
    assert.equal(r.lines[0]!.durationMs, 2000);
    assert.equal(r.lines[0]!.text, "first line two");
    assert.equal(r.lines[1]!.timeMs, 5000);
    assert.equal(r.lines[1]!.durationMs, 2500);
});

test("VTT: header/NOTE skipped, dot-ms timings, markup stripped", () => {
    const r = parseSubtitle(
        "WEBVTT\n\nNOTE\nthis is a comment\n\n00:00:01.000 --> 00:00:02.000\n<v Speaker>hello</v>\n\n00:00:04.000 --> 00:00:06.000\n&nbsp;world&nbsp;\n",
    )!;
    assert.equal(r.lines.length, 2);
    assert.equal(r.lines[0]!.text, "hello");
    assert.equal(r.lines[1]!.timeMs, 4000);
    assert.equal(r.lines[1]!.text, "world");
});

test("VTT: BOM stripped before format sniffing", () => {
    const r = parseSubtitle("\uFEFFWEBVTT\n\n00:00:01.000 --> 00:00:02.000\nhi\n")!;
    assert.equal(r.lines[0]!.text, "hi");
});

test("VTT: two-field timestamps (no hours) and settings tail tolerated", () => {
    const r = parseSubtitle("WEBVTT\n\n00:04.000 --> 00:06.500 align:start line:90%\nhi\n")!;
    assert.equal(r.lines[0]!.timeMs, 4000);
    assert.equal(r.lines[0]!.durationMs, 2500);
});

test("SRT: dot milliseconds tolerated; unparseable end drops durationMs", () => {
    const r = parseSubtitle("1\n00:00:01.250 --> oops\nx\n")!;
    assert.equal(r.lines[0]!.timeMs, 1250);
    assert.equal(r.lines[0]!.durationMs, undefined);
});

test("SRT: cues without index lines", () => {
    const r = parseSubtitle(
        "00:00:01,000 --> 00:00:02,000\na\n\n00:00:03,000 --> 00:00:04,000\nb\n",
    )!;
    assert.equal(r.lines.length, 2);
});

test("subtitles: no cues -> null (not mine)", () => {
    assert.equal(parseSubtitle("random text\nwithout cues\n"), null);
    assert.equal(parseSubtitle(""), null);
});

console.log(`\n${passed} tests passed`);
