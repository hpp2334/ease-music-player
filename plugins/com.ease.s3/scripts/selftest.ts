// Node-runnable selftest for the pure modules (`src/sigv4.ts`,
// `src/s3path.ts`, `src/listing.ts` + the shared `infra/http-dates.ts` date
// parser listing depends on — no tur imports involved; listing's
// only local runtime imports, `./s3path.ts` and `../../infra/http-dates.ts`,
// are Node-resolvable). Run:
// pnpm test   (node executes .TS via native type stripping)
//
// Two layers of verification:
//   1. Known-good constants from the AWS SigV4 documentation (the empty
//      payload hash, the docs' worked signing-key derivation example).
//   2. An independent reference signer built on node:crypto (OpenSSL HMAC /
//      SHA-256) — cross-checks @noble/hashes and this plugin's assembly
//      against a second implementation for every request shape the backend
//      produces (list with/without pagination tokens, ranged download,
//      custom ports, CJK / space / `+` / `%` keys).

import assert from "node:assert/strict";
import { createHash, createHmac } from "node:crypto";
import { parseHttpDate, parseIsoDate, parseHttpOrIsoDate } from "../../infra/http-dates.ts";
import {
    EMPTY_PAYLOAD_SHA256,
    amzTimestamps,
    canonicalQueryString,
    deriveSigningKey,
    canonicalRequest,
    stringToSign,
    signRequest,
    type SigV4Request,
} from "../src/sigv4.ts";
import {
    normalizeEndpoint,
    s3Encode,
    s3Decode,
    dirToPrefix,
    pathToKey,
    keyToPath,
    canonicalUri,
} from "../src/s3path.ts";
import { parseListPage, pageToEntries, parseS3Error } from "../src/listing.ts";

let passed = 0;

function test(name: string, fn: () => void) {
    fn();
    passed += 1;
    console.log(`  ok  ${name}`);
}

// ---------------------------------------------------------------------------
// Reference signer (node:crypto) — independent of the plugin's code paths
// ---------------------------------------------------------------------------

const refHmac = (key: Buffer | string, data: string): Buffer =>
    createHmac("sha256", key).update(data, "utf8").digest();
const refSha256Hex = (data: string): string =>
    createHash("sha256").update(data, "utf8").digest("hex");

interface RefCreds {
    accessKey: string;
    secretKey: string;
}

function refSignature(p: SigV4Request, creds: RefCreds): string {
    // Canonical request assembled from scratch (sorted headers, space-folded
    // values) — NOT reusing the plugin's builder.
    const sorted = Object.entries(p.headers).sort((a, b) => (a[0] < b[0] ? -1 : 1));
    const headerLines = sorted.map(([k, v]) => `${k}:${v.replace(/\s+/g, " ").trim()}\n`).join("");
    const cr =
        p.method + "\n" +
        p.canonicalUri + "\n" +
        p.canonicalQuery + "\n" +
        headerLines + "\n" +
        sorted.map(([k]) => k).join(";") + "\n" +
        (p.payloadSha256 ?? EMPTY_PAYLOAD_SHA256);
    const sts =
        "AWS4-HMAC-SHA256\n" +
        p.amzDate + "\n" +
        `${p.dateStamp}/${p.region}/${p.service}/aws4_request\n` +
        refSha256Hex(cr);
    let k = refHmac("AWS4" + creds.secretKey, p.dateStamp);
    k = refHmac(k, p.region);
    k = refHmac(k, p.service);
    k = refHmac(k, "aws4_request");
    return refHmac(k, sts).toString("hex");
}

/** Cross-check one request: canonical request string, string-to-sign, and
 *  final signature must all agree with the reference signer + the assembled
 *  Authorization header must embed them. */
function crossCheck(name: string, p: SigV4Request, creds: RefCreds): void {
    test(name, () => {
        // 1. Canonical request string matches the reference assembly.
        const sorted = Object.entries(p.headers).sort((a, b) => (a[0] < b[0] ? -1 : 1));
        const refCr =
            p.method + "\n" +
            p.canonicalUri + "\n" +
            p.canonicalQuery + "\n" +
            sorted.map(([k, v]) => `${k}:${v.replace(/\s+/g, " ").trim()}\n`).join("") + "\n" +
            sorted.map(([k]) => k).join(";") + "\n" +
            (p.payloadSha256 ?? EMPTY_PAYLOAD_SHA256);
        assert.equal(canonicalRequest(p), refCr);

        // 2. String-to-sign embeds the reference hash of that request.
        const refSts =
            "AWS4-HMAC-SHA256\n" +
            p.amzDate + "\n" +
            `${p.dateStamp}/${p.region}/${p.service}/aws4_request\n` +
            refSha256Hex(refCr);
        assert.equal(stringToSign(p), refSts);

        // 3. Final signature equals the node:crypto-derived one.
        const signed = signRequest(p, creds);
        const expected = refSignature(p, creds);
        assert.ok(
            signed.authorization.includes(`Signature=${expected}`),
            `authorization mismatch:\n  got  ${signed.authorization}\n  want …Signature=${expected}`,
        );
        assert.equal(
            signed.signedHeaders,
            Object.keys(p.headers).sort().join(";"),
        );
    });
}

const CREDS: RefCreds = {
    accessKey: "AKIDEXAMPLE",
    secretKey: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
};

// ---------------------------------------------------------------------------
// sigv4: documented constants
// ---------------------------------------------------------------------------

test("empty-payload SHA-256 constant (AWS-documented)", () => {
    assert.equal(EMPTY_PAYLOAD_SHA256, refSha256Hex(""));
    assert.equal(
        EMPTY_PAYLOAD_SHA256,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
});

test("signing-key derivation matches the AWS docs worked example", () => {
    // docs: secret wJalrXUtnFEMI/…, 20150830/us-east-1/iam ->
    // c4afb1cc5771d871763a393e44b703571b55cc28424d1a5e86da6ed3c154a4b9
    const key = deriveSigningKey(CREDS.secretKey, "20150830", "us-east-1", "iam");
    assert.equal(
        Buffer.from(key).toString("hex"),
        "c4afb1cc5771d871763a393e44b703571b55cc28424d1a5e86da6ed3c154a4b9",
    );
});

test("amzTimestamps: UTC formatting", () => {
    const t = amzTimestamps(new Date("2015-08-30T12:36:00Z"));
    assert.equal(t.amzDate, "20150830T123600Z");
    assert.equal(t.dateStamp, "20150830");
});

// ---------------------------------------------------------------------------
// sigv4: canonical query
// ---------------------------------------------------------------------------

test("canonicalQueryString: sorts, strictly encodes, empty values", () => {
    assert.equal(
        canonicalQueryString([
            ["list-type", "2"],
            ["delimiter", "/"],
            ["prefix", ""],
        ]),
        "delimiter=%2F&list-type=2&prefix=",
    );
});

test("canonicalQueryString: token chars + space + CJK percent-encoded", () => {
    assert.equal(
        canonicalQueryString([
            ["prefix", "音 楽"],
            ["continuation-token", "1ueGcxLPRx1Tr/XYtHm5hCTd2uFTA="],
        ]),
        "continuation-token=1ueGcxLPRx1Tr%2FXYtHm5hCTd2uFTA%3D&prefix=%E9%9F%B3%20%E6%A5%BD",
    );
});

test("canonicalQueryString: duplicate keys sort by value", () => {
    assert.equal(
        canonicalQueryString([
            ["tag", "b"],
            ["tag", "a"],
        ]),
        "tag=a&tag=b",
    );
});

// ---------------------------------------------------------------------------
// sigv4: full request cross-checks (every shape the backend produces)
// ---------------------------------------------------------------------------

crossCheck(
    "sign: root list (bucket op, empty prefix)",
    {
        method: "GET",
        canonicalUri: "/example-bucket",
        canonicalQuery: "delimiter=%2F&list-type=2&max-keys=1000&prefix=",
        headers: {
            host: "example.amazonaws.com",
            "x-amz-content-sha256": EMPTY_PAYLOAD_SHA256,
            "x-amz-date": "20150830T123600Z",
        },
        amzDate: "20150830T123600Z",
        dateStamp: "20150830",
        region: "us-east-1",
        service: "s3",
    },
    CREDS,
);

crossCheck(
    "sign: paginated list with token + CJK prefix",
    {
        method: "GET",
        canonicalUri: "/my-bucket",
        canonicalQuery:
            "continuation-token=1ueGcxLPRx1Tr%2FXYtHm5hCTd2uFTA%3D&delimiter=%2F&list-type=2&max-keys=1000&prefix=%E9%9F%B3%E4%B9%90%2F",
        headers: {
            host: "s3.ap-northeast-1.amazonaws.com",
            "x-amz-content-sha256": EMPTY_PAYLOAD_SHA256,
            "x-amz-date": "20260914T080102Z",
        },
        amzDate: "20260914T080102Z",
        dateStamp: "20260914",
        region: "ap-northeast-1",
        service: "s3",
    },
    CREDS,
);

crossCheck(
    "sign: ranged download (Range signed, host with port)",
    {
        method: "GET",
        canonicalUri: "/music/album/a%20b%2Bc.mp3",
        canonicalQuery: "",
        headers: {
            host: "192.168.1.10:9000",
            "x-amz-content-sha256": EMPTY_PAYLOAD_SHA256,
            "x-amz-date": "20260914T080102Z",
            range: "bytes=1048576-",
        },
        amzDate: "20260914T080102Z",
        dateStamp: "20260914",
        region: "us-east-1",
        service: "s3",
    },
    CREDS,
);

crossCheck(
    "sign: base-path gateway + percent-laden key",
    {
        method: "GET",
        canonicalUri: "/gw/bkt/50%25%20off.mp3",
        canonicalQuery: "",
        headers: {
            host: "files.example.com",
            "x-amz-content-sha256": EMPTY_PAYLOAD_SHA256,
            "x-amz-date": "20150830T123600Z",
            range: "bytes=0-",
        },
        amzDate: "20150830T123600Z",
        dateStamp: "20150830",
        region: "auto",
        service: "s3",
    },
    CREDS,
);

test("sign: header values are space-folded per the spec", () => {
    const p: SigV4Request = {
        method: "GET",
        canonicalUri: "/b",
        canonicalQuery: "",
        headers: {
            host: "h.example.com",
            "x-amz-content-sha256": EMPTY_PAYLOAD_SHA256,
            "x-amz-date": "20150830T123600Z",
            range: "bytes=0-", // already canonical; folding must be a no-op
        },
        amzDate: "20150830T123600Z",
        dateStamp: "20150830",
        region: "us-east-1",
        service: "s3",
    };
    assert.equal(canonicalRequest(p).includes("range:bytes=0-\n"), true);
});

// ---------------------------------------------------------------------------
// s3path: encoding
// ---------------------------------------------------------------------------

test("s3Encode: unreserved pass through, everything else percent-encoded", () => {
    assert.equal(s3Encode("aA0-._~", true), "aA0-._~");
    assert.equal(s3Encode("a b+c.mp3", true), "a%20b%2Bc.mp3");
    assert.equal(s3Encode("音 楽.mp3", true), "%E9%9F%B3%20%E6%A5%BD.mp3");
    assert.equal(s3Encode("50%.mp3", true), "50%25.mp3");
    assert.equal(s3Encode("a/b", true), "a/b");
    assert.equal(s3Encode("a/b", false), "a%2Fb");
});

test("s3Decode: round-trips and tolerates raw keys", () => {
    assert.equal(s3Decode("a%20b%2Bc.mp3"), "a b+c.mp3");
    assert.equal(s3Decode("音 楽.mp3"), "音 楽.mp3"); // no % — untouched
    assert.equal(s3Decode("100%.mp3"), "100%.mp3"); // invalid escape — verbatim
});

// ---------------------------------------------------------------------------
// s3path: endpoints
// ---------------------------------------------------------------------------

test("normalizeEndpoint: scheme default + trailing slash", () => {
    const e = normalizeEndpoint("s3.amazonaws.com/");
    assert.equal(e.origin, "https://s3.amazonaws.com");
    assert.equal(e.host, "s3.amazonaws.com");
    assert.equal(e.basePath, "");
});

test("normalizeEndpoint: port kept for signing", () => {
    const e = normalizeEndpoint("http://192.168.1.10:9000");
    assert.equal(e.origin, "http://192.168.1.10:9000");
    assert.equal(e.host, "192.168.1.10:9000");
});

test("normalizeEndpoint: default ports stripped (Url-crate parity)", () => {
    assert.equal(normalizeEndpoint("https://x.example.com:443").host, "x.example.com");
    assert.equal(normalizeEndpoint("http://x.example.com:80").host, "x.example.com");
    assert.equal(normalizeEndpoint("http://x.example.com:8080").host, "x.example.com:8080");
});

test("normalizeEndpoint: base path supported", () => {
    const e = normalizeEndpoint("https://gw.example.com/s3/prefix/");
    assert.equal(e.origin, "https://gw.example.com/s3/prefix");
    assert.equal(e.host, "gw.example.com");
    assert.equal(e.basePath, "/s3/prefix");
});

test("normalizeEndpoint: rejects garbage", () => {
    assert.throws(() => normalizeEndpoint(""));
    assert.throws(() => normalizeEndpoint("://no-host"));
    assert.throws(() => normalizeEndpoint("ftp://x.com"));
});

// ---------------------------------------------------------------------------
// s3path: dir/prefix/key mapping
// ---------------------------------------------------------------------------

test("dirToPrefix: root variants collapse to empty prefix", () => {
    assert.equal(dirToPrefix(""), "");
    assert.equal(dirToPrefix("/"), "");
});

test("dirToPrefix: nested dirs keep their trailing slash", () => {
    assert.equal(dirToPrefix("/music"), "music/");
    assert.equal(dirToPrefix("/music/"), "music/");
    assert.equal(dirToPrefix("music/album"), "music/album/");
});

test("pathToKey / keyToPath round-trip", () => {
    assert.equal(pathToKey("/music/a.mp3"), "music/a.mp3");
    assert.equal(keyToPath("music/a.mp3"), "/music/a.mp3");
    assert.equal(keyToPath(pathToKey("/音 楽/x.mp3")), "/音 楽/x.mp3");
});

test("canonicalUri: bucket + encoded key, base path prepended", () => {
    const e = normalizeEndpoint("https://gw.example.com/s3");
    assert.equal(canonicalUri(e, "my-bucket", "a b/音.mp3"), "/s3/my-bucket/a%20b/%E9%9F%B3.mp3");
    assert.equal(canonicalUri(e, "my-bucket", null), "/s3/my-bucket");
});

// ---------------------------------------------------------------------------
// listing: ListObjectsV2 fixtures
// ---------------------------------------------------------------------------

const PAGE1 = `<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Name>music</Name>
  <Prefix>music/</Prefix>
  <KeyCount>4</KeyCount>
  <MaxKeys>2</MaxKeys>
  <Delimiter>/</Delimiter>
  <IsTruncated>true</IsTruncated>
  <NextContinuationToken>1ueGcxLPRx1Tr/XYtHm5hCTd2uFTA==</NextContinuationToken>
  <Contents>
    <Key>music/</Key>
    <LastModified>2014-11-21T19:40:05.000Z</LastModified>
    <ETag>&quot;70ee1738b6e2&quot;</ETag>
    <Size>0</Size>
    <StorageClass>STANDARD</StorageClass>
  </Contents>
  <Contents>
    <Key>music/%E9%9F%B3%E6%A5%BD.mp3</Key>
    <LastModified>2014-11-21T19:40:05.000Z</LastModified>
    <ETag>&quot;70ee1738b6e3&quot;</ETag>
    <Size>1048576</Size>
    <StorageClass>STANDARD</StorageClass>
  </Contents>
  <CommonPrefixes>
    <Prefix>music/album/</Prefix>
  </CommonPrefixes>
</ListBucketResult>`;

const PAGE2 = `<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Name>music</Name>
  <Prefix>music/</Prefix>
  <KeyCount>1</KeyCount>
  <MaxKeys>2</MaxKeys>
  <Delimiter>/</Delimiter>
  <IsTruncated>false</IsTruncated>
  <Contents>
    <Key>music/track 2+final.flac</Key>
    <LastModified>2015-01-01T00:00:00.000Z</LastModified>
    <ETag>&quot;abcdef0123&quot;</ETag>
    <Size>42</Size>
    <StorageClass>STANDARD</StorageClass>
  </Contents>
</ListBucketResult>`;

test("parseListPage: truncated page yields keys/prefixes/token", () => {
    const page = parseListPage(PAGE1);
    assert.equal(page.truncated, true);
    assert.equal(page.nextToken, "1ueGcxLPRx1Tr/XYtHm5hCTd2uFTA==");
    assert.equal(page.keys.length, 2);
    assert.equal(page.keys[0].key, "music/"); // dir marker key
    assert.equal(page.keys[0].size, 0);
    assert.deepEqual(page.prefixes, ["music/album/"]);
});

test("parseListPage: final page has no token", () => {
    const page = parseListPage(PAGE2);
    assert.equal(page.truncated, false);
    assert.equal(page.nextToken, null);
    assert.equal(page.keys.length, 1);
    // MinIO-style raw key (with spaces, no percent-escapes) stays verbatim.
    assert.equal(page.keys[0].key, "music/track 2+final.flac");
    assert.equal(page.keys[0].size, 42);
    // LastModified parses to ms since the epoch (RFC 3339 shape).
    assert.equal(page.keys[0].modifiedAt, Date.UTC(2015, 0, 1, 0, 0, 0));
});

test("parseListPage: throws on non-ListBucketResult XML", () => {
    assert.throws(() => parseListPage("<html><body>404</body></html>"));
});

test("pageToEntries: URL-encoded keys decode; markers elided; dirs first", () => {
    const page = parseListPage(PAGE1);
    const entries = pageToEntries(page, "music/");
    assert.deepEqual(
        entries.map((e) => ({ name: e.name, path: e.path, isDir: e.isDir, size: e.size })),
        [
            { name: "album", path: "/music/album", isDir: true, size: undefined },
            { name: "音楽.mp3", path: "/music/音楽.mp3", isDir: false, size: 1048576 },
        ],
    );
    // S3 has no creation time; LastModified rides modifiedAt.
    assert.deepEqual(
        entries.map((e) => e.modifiedAt),
        [undefined, Date.UTC(2014, 10, 21, 19, 40, 5)],
    );
    assert.ok(entries.every((e) => e.createdAt === undefined));
});

test("pageToEntries: nested folder markers elided across pages", () => {
    const all = [...pageToEntries(parseListPage(PAGE1), "music/"), ...pageToEntries(parseListPage(PAGE2), "music/")];
    assert.equal(all.filter((e) => e.isDir).length, 1);
    assert.deepEqual(
        all.map((e) => e.name),
        ["album", "音楽.mp3", "track 2+final.flac"],
    );
});

test("pageToEntries: root listing (empty prefix)", () => {
    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Name>b</Name><Prefix></Prefix><KeyCount>2</KeyCount><Delimiter>/</Delimiter><IsTruncated>false</IsTruncated>
  <Contents><Key>a.mp3</Key><Size>7</Size></Contents>
  <CommonPrefixes><Prefix>dir/</Prefix></CommonPrefixes>
</ListBucketResult>`;
    const entries = pageToEntries(parseListPage(xml), "");
    assert.deepEqual(
        entries.map((e) => [e.name, e.path, e.isDir]),
        [["dir", "/dir", true], ["a.mp3", "/a.mp3", false]],
    );
});

test("parseS3Error: extracts Code/Message", () => {
    const err = parseS3Error(
        `<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>RequestTimeTooSkewed</Code><Message>The difference between the request time and the current time is too large.</Message></Error>`,
    );
    assert.equal(err?.code, "RequestTimeTooSkewed");
    assert.ok(err?.message.includes("too large"));
    assert.equal(parseS3Error("<ListBucketResult/>"), null);
    assert.equal(parseS3Error("not xml <<<"), null);
});

// ---------------------------------------------------------------------------
// infra/http-dates.ts — shared date parser (WebDAV `getlastmodified` /
// `creationdate`, Graph `createdDateTime`, S3 `LastModified` all feed the
// host entry timestamps; sorted by the app's import page)
// ---------------------------------------------------------------------------

test("parseHttpDate: RFC 1123 (WebDAV getlastmodified)", () => {
    assert.equal(parseHttpDate("Sun, 06 Nov 1994 08:49:37 GMT"), 784111777000);
    assert.equal(parseHttpDate("Tue, 15 Nov 1994 12:45:26 GMT"), Date.UTC(1994, 10, 15, 12, 45, 26));
    assert.equal(parseHttpDate("Fri, 21 Nov 2014 19:40:05 GMT"), Date.UTC(2014, 10, 21, 19, 40, 5));
    assert.equal(parseHttpDate("garbage"), undefined);
});

test("parseIsoDate: RFC 3339 / ISO 8601 (creationdate, Graph, S3)", () => {
    assert.equal(parseIsoDate("1994-11-06T08:49:37Z"), 784111777000);
    assert.equal(parseIsoDate("1994-11-06T08:49:37.123Z"), 784111777123);
    // UTC offset: 08:49:37-05:00 === 13:49:37Z
    assert.equal(parseIsoDate("1994-11-06T08:49:37-05:00"), 784111777000 + 5 * 3600 * 1000);
    assert.equal(parseIsoDate("1994-11-06T16:49:37+08:00"), 784111777000);
    // space instead of `T` (some servers)
    assert.equal(parseIsoDate("1994-11-06 08:49:37Z"), 784111777000);
    // leap-year civil math
    assert.equal(parseIsoDate("2000-02-29T00:00:00Z"), Date.UTC(2000, 1, 29));
    assert.equal(parseIsoDate("nonsense"), undefined);
    assert.equal(parseIsoDate("1994-13-06T08:49:37Z"), undefined);
});

test("parseHttpOrIsoDate: accepts either shape", () => {
    assert.equal(parseHttpOrIsoDate("Sun, 06 Nov 1994 08:49:37 GMT"), 784111777000);
    assert.equal(parseHttpOrIsoDate("1994-11-06T08:49:37Z"), 784111777000);
    assert.equal(parseHttpOrIsoDate("nope"), undefined);
});

// ---------------------------------------------------------------------------

console.log(`\n${passed} tests passed`);
