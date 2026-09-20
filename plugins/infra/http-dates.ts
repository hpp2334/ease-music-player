// Date-string parsing for storage plugins, without a dependency or the
// `Date` constructor (boa's `Date.parse` coverage is not reliable across
// the formats HTTP/WebDAV/cloud APIs actually return).
//
// All functions return milliseconds since the Unix epoch, or `undefined`
// for anything unparseable — callers map that straight to the optional
// `createdAt` / `modifiedAt` fields of the host `StorageEntry` contract.
//
// Two shapes occur in practice:
// - RFC 1123 ("Sun, 06 Nov 1994 08:49:37 GMT") — WebDAV `getlastmodified`.
// - RFC 3339 / ISO 8601 ("1994-11-06T08:49:37Z", with optional fractional
//   seconds and ±hh:mm offsets) — WebDAV `creationdate`, Microsoft Graph
//   `createdDateTime` / `lastModifiedDateTime`, S3 `LastModified`.
//
// Timestamps are computed against the UTC epoch (leap seconds ignored,
// like every mainstream HTTP client).

/** Days from 1970-01-01 to the first of `month` in `year` (month 1-12). */
function daysFromCivil(year: number, month: number, day: number): number {
    const y = month <= 2 ? year - 1 : year;
    const era = Math.floor(y / 400);
    const yoe = y - era * 400; // [0, 399]
    const mp = (month + 9) % 12; // [0, 11]
    const doy = Math.floor((153 * mp + 2) / 5) + day - 1; // [0, 365]
    const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
    return era * 146097 + doe - 719468;
}

const MONTHS: Record<string, number> = {
    jan: 1, feb: 2, mar: 3, apr: 4, may: 5, jun: 6,
    jul: 7, aug: 8, sep: 9, oct: 10, nov: 11, dec: 12,
};

/** RFC 1123 / RFC 850-ish HTTP date: "Sun, 06 Nov 1994 08:49:37 GMT".
 *  The weekday name (if present) and "GMT" suffix are not validated —
 *  servers abbreviate inconsistently and the offset is always GMT here. */
export function parseHttpDate(s: string): number | undefined {
    // Strip an optional "Wkd, " prefix.
    const rest = s.replace(/^[A-Za-z]{3,9},\s*/, "").trim();
    const m = /^(\d{1,2})\s+([A-Za-z]{3})[A-Za-z]*\s+(\d{2,4})\s+(\d{1,2}):(\d{2})(?::(\d{2}))?/.exec(rest);
    if (m == null) return undefined;
    const month = MONTHS[m[2].toLowerCase()];
    if (month == null) return undefined;
    const day = parseInt(m[1], 10);
    let year = parseInt(m[3], 10);
    // Two-digit years: RFC 1123 (years 00-49 => 2000s, 50-99 => 1900s).
    if (m[3].length <= 2) year = year < 50 ? 2000 + year : 1900 + year;
    const hh = parseInt(m[4], 10);
    const mm = parseInt(m[5], 10);
    const ss = m[6] != null ? parseInt(m[6], 10) : 0;
    if (day < 1 || day > 31 || hh > 23 || mm > 59 || ss > 60) return undefined;
    return (daysFromCivil(year, month, day) * 86400 + hh * 3600 + mm * 60 + ss) * 1000;
}

/** RFC 3339 / ISO 8601: "1994-11-06T08:49:37Z", "…T08:49:37.123Z",
 *  "…T08:49:37+08:00". A space may appear where a `T` should (some
 *  servers emit "1994-11-06 08:49:37"). */
export function parseIsoDate(s: string): number | undefined {
    const m = /^(\d{4})-(\d{2})-(\d{2})[Tt ](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,9}))?)?(Z|z|[+-]\d{2}:?\d{2})?$/.exec(s.trim());
    if (m == null) return undefined;
    const year = parseInt(m[1], 10);
    const month = parseInt(m[2], 10);
    const day = parseInt(m[3], 10);
    const hh = parseInt(m[4], 10);
    const mm = parseInt(m[5], 10);
    const ss = m[6] != null ? parseInt(m[6], 10) : 0;
    if (month < 1 || month > 12 || day < 1 || day > 31 || hh > 23 || mm > 59 || ss > 60) return undefined;
    let seconds = daysFromCivil(year, month, day) * 86400 + hh * 3600 + mm * 60 + ss;
    let ms = m[7] != null ? parseInt((m[7] + "00").slice(0, 3), 10) : 0;
    const offset = m[8];
    if (offset != null && offset !== "Z" && offset !== "z") {
        const sign = offset[0] === "-" ? -1 : 1;
        const digits = offset.slice(1).replace(":", "");
        const oh = parseInt(digits.slice(0, 2), 10);
        const om = parseInt(digits.slice(2, 4), 10);
        seconds -= sign * (oh * 3600 + om * 60);
    }
    return seconds * 1000 + ms;
}

/** Try both known shapes; the input's own shape decides. */
export function parseHttpOrIsoDate(s: string): number | undefined {
    return parseIsoDate(s) ?? parseHttpDate(s);
}
