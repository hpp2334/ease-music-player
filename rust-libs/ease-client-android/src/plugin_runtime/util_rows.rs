//! The `ease` utility rows — the support surface that replaced the old npm
//! plugin dependencies (`fast-xml-parser`, `js-md5`, `@noble/hashes`,
//! `uuid`, WHATWG polyfills, `http-dates`).
//!
//! - `ease.crypto` — OS-entropy randomness, UUID v4, MD5 (WebDAV auth),
//!   Base64, SHA-256 + HMAC-SHA-256 (S3 SigV4).
//! - `ease.xml` — `to_json`: an XML document (bytes) → a JSON tree string
//!   (quick-xml → `serde_json::Value`), the PROPFIND/multistatus decoder.
//! - `ease.net` — full-featured HTTP over the engine's `Http` capability
//!   (upstream's rut net rows carry no headers/body; every provider needs
//!   `Authorization` / `Depth` / `PROPFIND` bodies). `request(opts) ->
//!   opaque` allocates the task and starts the drive; `finish(task) ->
//!   bytes` awaits completion; the accessors (`status` / `status_text` /
//!   `headers` / `body` / `error`) read the settled task.
//! - `ease.time` — `Date` replacements: unix-ms now, RFC-1123 formatting
//!   (`Date` / `If-Modified-Since`), the `x-amz-date` compact form, and
//!   HTTP date parsing (`createdAt` / `modifiedAt` display).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use base64::Engine as _;
use rut_core::types::{TY_BYTES, TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64, TY_OPT_STR};
use tur_engine::core::rut_runtime::{host_row_at, RutHandles, RutPkgCx};

// ---------------------------------------------------------------------------
// the net task — Rc-field sharing between the opaque box and the drive
// ---------------------------------------------------------------------------

/// The opaque payload. Allocated into the heap as a clone whose `Rc` fields
/// share state with the background drive (the upstream `RutNetTask`
/// discipline — the box and the drive see the same cells).
#[derive(Clone)]
pub struct NetTask {
    status: Rc<RefCell<u16>>,
    status_text: Rc<RefCell<String>>,
    headers: Rc<RefCell<Vec<(String, String)>>>,
    body: Rc<RefCell<Vec<u8>>>,
    error: Rc<RefCell<String>>,
    done: Rc<RefCell<bool>>,
    waiter: Rc<RefCell<Option<rut_vm::Completer<Vec<u8>>>>>,
}

impl NetTask {
    fn new() -> Self {
        Self {
            status: Rc::new(RefCell::new(0)),
            status_text: Rc::new(RefCell::new(String::new())),
            headers: Rc::new(RefCell::new(Vec::new())),
            body: Rc::new(RefCell::new(Vec::new())),
            error: Rc::new(RefCell::new(String::new())),
            done: Rc::new(RefCell::new(false)),
            waiter: Rc::new(RefCell::new(None)),
        }
    }

    fn settle(&self, outcome: tur_net_capability::HttpOutcome) {
        match outcome {
            tur_net_capability::HttpOutcome::Ok {
                status,
                status_text,
                headers,
                body,
            } => {
                *self.status.borrow_mut() = status;
                *self.status_text.borrow_mut() = status_text;
                *self.headers.borrow_mut() = headers;
                *self.body.borrow_mut() = body;
                *self.error.borrow_mut() = String::new();
            }
            tur_net_capability::HttpOutcome::Err(msg) => {
                *self.error.borrow_mut() = msg;
            }
        }
        *self.done.borrow_mut() = true;
        if let Some(w) = self.waiter.borrow_mut().take() {
            w.complete(self.body.borrow().clone());
        }
    }
}

pub fn install(cx: &mut RutPkgCx<'_>) {
    // ---- crypto ------------------------------------------------------------
    let crow = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId, a: bool| {
        host_row_at("ease/crypto", n, p, r, a)
    };
    cx.decl.extend(
        vec![
            crow("random_bytes", vec![TY_U64], TY_BYTES, false),
            crow("random_uuid", vec![], TY_STR, false),
            crow("md5_hex", vec![TY_BYTES], TY_STR, false),
            crow("md5_base64", vec![TY_BYTES], TY_STR, false),
            crow("base64_encode", vec![TY_BYTES], TY_STR, false),
            crow("base64_decode", vec![TY_STR], TY_BYTES, false),
            crow("sha256_hex", vec![TY_BYTES], TY_STR, false),
            crow("hmac_sha256_hex", vec![TY_BYTES, TY_BYTES], TY_STR, false),
        ]
        .into_iter(),
    );

    // ---- xml ---------------------------------------------------------------
    let xrow = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId, a: bool| {
        host_row_at("ease/xml", n, p, r, a)
    };
    cx.decl.extend(vec![xrow("to_json", vec![TY_BYTES], TY_OPT_STR, false)].into_iter());

    // ---- net -----------------------------------------------------------------
    let nrow = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId, a: bool| {
        host_row_at("ease/net", n, p, r, a)
    };
    cx.decl.extend(
        vec![
            nrow("request", vec![TY_STR], TY_OPAQUE, false),
            nrow("finish", vec![TY_OPAQUE], TY_BYTES, true),
            nrow("request_sync", vec![TY_STR], TY_STR, false),
            nrow("status", vec![TY_OPAQUE], TY_U64, false),
            nrow("status_text", vec![TY_OPAQUE], TY_STR, false),
            nrow("headers", vec![TY_OPAQUE], TY_STR, false),
            nrow("body", vec![TY_OPAQUE], TY_BYTES, false),
            nrow("error", vec![TY_OPAQUE], TY_STR, false),
        ]
        .into_iter(),
    );

    // ---- time ------------------------------------------------------------------
    let trow = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId, a: bool| {
        host_row_at("ease/time", n, p, r, a)
    };
    cx.decl.extend(
        vec![
            trow("now_unix_ms", vec![], TY_U64, false),
            trow("format_http_date", vec![TY_U64], TY_STR, false),
            trow("format_amz_date", vec![TY_U64], TY_STR, false),
            trow("parse_http_date", vec![TY_STR], TY_U64, false),
            trow("unix_ms", vec![TY_F64], TY_U64, false),
        ]
        .into_iter(),
    );

    let Some(handles) = cx.handles else {
        return; // compile-time decl probe
    };
    let handles = handles.clone();
    let pkg = &mut *cx.pkg;

    // ---- crypto bodies -----------------------------------------------------------

    rut_vm::pkg_fn!(pkg, "ease::crypto::random_bytes", (u64,) -> Vec<u8>, move |_vm: &mut rut_vm::interp::Vm, n: u64| {
        let n = n.min(1 << 20) as usize;
        let mut buf = vec![0u8; n];
        getrandom::fill(&mut buf)
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("ease:crypto random_bytes: {e}")))?;
        Ok(buf)
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::random_uuid", () -> String, move |_vm: &mut rut_vm::interp::Vm| {
        Ok(uuid::Uuid::new_v4().to_string())
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::md5_hex", (Vec<u8>,) -> String, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        use md5::Digest;
        let mut h = md5::Md5::new();
        h.update(&data);
        Ok(hex_encode(h.finalize()))
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::md5_base64", (Vec<u8>,) -> String, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        use md5::Digest;
        let mut h = md5::Md5::new();
        h.update(&data);
        Ok(b64_encode(h.finalize()))
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::base64_encode", (Vec<u8>,) -> String, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        Ok(b64_encode(&data))
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::base64_decode", (String,) -> Vec<u8>, move |_vm: &mut rut_vm::interp::Vm, s: String| {
        base64::engine::general_purpose::STANDARD
            .decode(s.as_bytes())
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("ease:crypto base64_decode: {e}")))
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::sha256_hex", (Vec<u8>,) -> String, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        use sha2::Digest;
        let mut h = sha2::Sha256::new();
        h.update(&data);
        Ok(hex_encode(h.finalize()))
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::hmac_sha256_hex", (Vec<u8>, Vec<u8>) -> String, move |_vm: &mut rut_vm::interp::Vm, key: Vec<u8>, data: Vec<u8>| {
        use hmac::Mac;
        let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(&key)
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("ease:crypto hmac: {e}")))?;
        mac.update(&data);
        Ok(hex_encode(mac.finalize().into_bytes()))
    });

    // ---- xml bodies ---------------------------------------------------------------

    rut_vm::pkg_fn!(pkg, "ease::xml::to_json", (Vec<u8>,) -> Option<String>, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        Ok(xml_to_json(&data))
    });

    // ---- net bodies ------------------------------------------------------------------

    /// `request(opts: str) -> opaque` — allocate the task, start the drive.
    /// Sync (the drive rides `spawn_local`); await via `finish`. The answer
    /// crosses as the bare `OpaqueRef` (its `Ret::TY` is `TY_OPAQUE`, so the
    /// decl-row signature check passes — a typed `Opaque<NetTask>` return
    /// carries `u32::MAX` and drifts every boot).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ease::net::request", (String,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, opts_json: String| {
        let task = NetTask::new();
        if let Err(msg) = net_start_drive(&h, &task, &opts_json) {
            task.settle(tur_net_capability::HttpOutcome::Err(msg));
        }
        Ok(rut_vm::Opaque::alloc(vm, task)?.handle().clone())
    });

    /// `request_sync(opts: str) -> str` — the blocking one-shot. Fires the
    /// request and answers a JSON envelope
    /// `{"status": u64, "statusText": str, "headers": {name: value},
    ///   "body": <base64>, "error": str}` (`body` "" + `error` set on a
    /// transport failure). Runs the capability future to completion on the
    /// calling thread via the shared tokio runtime — **entry-fn-safe**:
    /// the drive lives on the tokio runtime, never the worker lane, so op
    /// entries can send+await in one row call (no settled-task spin).
    /// `opts` shape: see [`net_start_drive`].
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ease::net::request_sync", (String,) -> String, move |_vm: &mut rut_vm::interp::Vm, opts_json: String| {
        net_request_sync(&h, &opts_json)
    });

    /// `finish(task: opaque) -> bytes` — await the drive; the body (empty on
    /// a transport failure — read `error`).
    rut_vm::pkg_async_fn!(pkg, "ease::net::finish", (rut_vm::Opaque<NetTask>,) -> Vec<u8>, move |t: rut_vm::Opaque<NetTask>| {
        let done = rut_vm::Completer::<Vec<u8>>::new();
        // Poison on `with` is impossible while the box lives; treat it as a
        // settled-empty task regardless (infallible body).
        let _ = t.with(|t| {
            if *t.done.borrow() {
                done.complete(t.body.borrow().clone());
            } else {
                *t.waiter.borrow_mut() = Some(done.clone());
            }
        });
        done
    });

    rut_vm::pkg_fn!(pkg, "ease::net::status", (rut_vm::Opaque<NetTask>,) -> u64, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<NetTask>| {
        t.with(|t| *t.status.borrow() as u64).unwrap_or(0)
    });

    rut_vm::pkg_fn!(pkg, "ease::net::status_text", (rut_vm::Opaque<NetTask>,) -> String, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<NetTask>| {
        t.with(|t| t.status_text.borrow().clone()).unwrap_or_default()
    });

    rut_vm::pkg_fn!(pkg, "ease::net::headers", (rut_vm::Opaque<NetTask>,) -> String, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<NetTask>| {
        let headers = t.with(|t| t.headers.borrow().clone()).unwrap_or_default();
        let map: serde_json::Map<String, serde_json::Value> = headers
            .iter()
            .map(|(k, v)| (k.to_lowercase(), serde_json::Value::String(v.clone())))
            .collect();
        serde_json::Value::Object(map).to_string()
    });

    rut_vm::pkg_fn!(pkg, "ease::net::body", (rut_vm::Opaque<NetTask>,) -> Vec<u8>, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<NetTask>| {
        t.with(|t| t.body.borrow().clone()).unwrap_or_default()
    });

    rut_vm::pkg_fn!(pkg, "ease::net::error", (rut_vm::Opaque<NetTask>,) -> String, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<NetTask>| {
        t.with(|t| t.error.borrow().clone()).unwrap_or_default()
    });

    // ---- time bodies -------------------------------------------------------------------

    rut_vm::pkg_fn!(pkg, "ease::time::now_unix_ms", () -> u64, move |_vm: &mut rut_vm::interp::Vm| {
        Ok(now_unix_ms())
    });

    rut_vm::pkg_fn!(pkg, "ease::time::format_http_date", (u64,) -> String, move |_vm: &mut rut_vm::interp::Vm, ms: u64| {
        Ok(format_http_date(ms))
    });

    rut_vm::pkg_fn!(pkg, "ease::time::format_amz_date", (u64,) -> String, move |_vm: &mut rut_vm::interp::Vm, ms: u64| {
        Ok(format_amz_date(ms))
    });

    rut_vm::pkg_fn!(pkg, "ease::time::parse_http_date", (String,) -> u64, move |_vm: &mut rut_vm::interp::Vm, s: String| {
        Ok(parse_http_date(&s).unwrap_or(0))
    });

    rut_vm::pkg_fn!(pkg, "ease::time::unix_ms", (f64,) -> u64, move |_vm: &mut rut_vm::interp::Vm, secs: f64| {
        // convenience: `seconds since epoch` (the old JS `Date.now()/1000`
        // round-trips) → unix ms
        Ok((secs * 1000.0) as u64)
    });
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

fn b64_encode(bytes: impl AsRef<[u8]>) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes.as_ref())
}

/// `unix ms` now (wall clock).
fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Civil breakdown of a unix-ms instant (UTC) — Howard Hinnant's
/// `civil_from_days`.
fn civil(ms: u64) -> (i64, u32, u32, u32, u32, u32) {
    let secs = (ms / 1000) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = ((rem / 3600) as u32, ((rem % 3600) / 60) as u32, (rem % 60) as u32);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = if month <= 2 { y + 1 } else { y };
    (year, month, d, h, m, s)
}

fn weekday_short(ms: u64) -> &'static str {
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    // 1970-01-01 was a Thursday.
    let days = (ms / 86_400_000) as i64;
    DAYS[days.rem_euclid(7) as usize]
}

fn month_short(month: u32) -> &'static str {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    MONTHS[(month - 1).clamp(0, 11) as usize]
}

/// RFC 1123 IMF-fixdate: `Mon, 02 Jan 2006 15:04:05 GMT`.
fn format_http_date(ms: u64) -> String {
    let (y, mo, d, h, m, s) = civil(ms);
    format!(
        "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
        weekday_short(ms),
        d,
        month_short(mo),
        y,
        h,
        m,
        s
    )
}

/// `x-amz-date` compact form: `20060102T150405Z`.
fn format_amz_date(ms: u64) -> String {
    let (y, mo, d, h, m, s) = civil(ms);
    format!("{y:04}{mo:02}{d:02}T{h:02}{m:02}{s:02}Z")
}

/// Parse the three HTTP date forms (IMF-fixdate, RFC 850, asctime) to unix
/// ms; `None` on anything else (the old `http-dates.ts` contract).
fn parse_http_date(s: &str) -> Option<u64> {
    parse_imf_fixdate(s.trim())
        .or_else(|| parse_asctime(s.trim()))
        .or_else(|| parse_rfc850(s.trim()))
}

fn month_from_short(m: &str) -> Option<u32> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    MONTHS
        .iter()
        .position(|x| *x == m.to_ascii_lowercase().as_str())
        .map(|i| i as u32 + 1)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn to_unix_ms(y: i64, mo: u32, d: u32, h: u32, m: u32, s: u32) -> Option<u64> {
    let days = days_from_civil(y, mo, d);
    Some((days * 86_400 + h as i64 * 3600 + m as i64 * 60 + s as i64) as u64 * 1000)
}

/// `Mon, 02 Jan 2006 15:04:05 GMT` (2-digit year variant tolerated).
fn parse_imf_fixdate(s: &str) -> Option<u64> {
    let rest = s.split_once(',').map(|(_, r)| r.trim()).unwrap_or(s);
    let mut it = rest.split_ascii_whitespace();
    let d: u32 = it.next()?.parse().ok()?;
    let mon = month_from_short(it.next()?)?;
    let year_raw = it.next()?;
    let year: i64 = if year_raw.len() == 2 {
        let n: i64 = year_raw.parse().ok()?;
        if n >= 70 { 1900 + n } else { 2000 + n }
    } else {
        year_raw.parse().ok()?
    };
    let (h, m, sec) = parse_hms(it.next()?)?;
    if !matches!(it.next(), Some("GMT") | Some("UTC") | None) {
        return None;
    }
    to_unix_ms(year, mon, d, h, m, sec)
}

/// `Sun Nov  6 08:49:37 1994` (asctime).
fn parse_asctime(s: &str) -> Option<u64> {
    let mut it = s.split_ascii_whitespace();
    let _wd = it.next()?;
    let mon = month_from_short(it.next()?)?;
    let d: u32 = it.next()?.parse().ok()?;
    let (h, m, sec) = parse_hms(it.next()?)?;
    let year: i64 = it.next()?.parse().ok()?;
    to_unix_ms(year, mon, d, h, m, sec)
}

/// `Sunday, 06-Nov-94 08:49:37 GMT` (RFC 850).
fn parse_rfc850(s: &str) -> Option<u64> {
    let rest = s.split_once(',').map(|(_, r)| r.trim())?;
    let mut it = rest.split_ascii_whitespace();
    let dmy = it.next()?;
    let mut parts = dmy.split('-');
    let d: u32 = parts.next()?.parse().ok()?;
    let mon = month_from_short(parts.next()?)?;
    let year_raw = parts.next()?;
    let year: i64 = if year_raw.len() == 2 {
        let n: i64 = year_raw.parse().ok()?;
        if n >= 70 { 1900 + n } else { 2000 + n }
    } else {
        year_raw.parse().ok()?
    };
    let (h, m, sec) = parse_hms(it.next()?)?;
    to_unix_ms(year, mon, d, h, m, sec)
}

fn parse_hms(s: &str) -> Option<(u32, u32, u32)> {
    let mut it = s.split(':');
    let h: u32 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let sec: u32 = it.next().unwrap_or("0").parse().ok()?;
    Some((h, m, sec))
}

/// quick-xml → JSON tree. Element: `{"attrs…": …, "text": "…", "children":
/// {tag: el | [el, …]}}`. Namespace prefixes stay verbatim (the plugins
/// match `D:multistatus` / `d:response` case-insensitively downstream).
/// `None` = unparseable XML or invalid UTF-8.
fn xml_to_json(bytes: &[u8]) -> Option<String> {
    use quick_xml::events::Event;

    let text = std::str::from_utf8(bytes).ok()?;
    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().trim_text(true);

    #[derive(Default)]
    struct Node {
        attrs: serde_json::Map<String, serde_json::Value>,
        text: String,
        children: HashMap<String, Vec<serde_json::Value>>,
    }

    fn finish(n: Node) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        for (k, v) in n.attrs {
            obj.insert(k, v);
        }
        if !n.text.is_empty() {
            obj.insert("text".into(), serde_json::Value::String(n.text));
        }
        if !n.children.is_empty() {
            obj.insert(
                "children".into(),
                serde_json::Value::Object(
                    n.children
                        .into_iter()
                        .map(|(k, mut v)| {
                            if v.len() == 1 {
                                (k, v.pop().unwrap_or(serde_json::Value::Null))
                            } else {
                                (k, serde_json::Value::Array(v))
                            }
                        })
                        .collect(),
                ),
            );
        }
        serde_json::Value::Object(obj)
    }

    let mut stack: Vec<(String, Node)> = vec![(String::new(), Node::default())];
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut node = Node::default();
                for a in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                    node.attrs.insert(
                        k,
                        serde_json::Value::String(
                            String::from_utf8_lossy(a.unescape_value().unwrap_or_default().as_bytes())
                                .to_string(),
                        ),
                    );
                }
                stack.push((name, node));
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut node = Node::default();
                for a in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                    node.attrs.insert(
                        k,
                        serde_json::Value::String(
                            String::from_utf8_lossy(a.unescape_value().unwrap_or_default().as_bytes())
                                .to_string(),
                        ),
                    );
                }
                if let Some((_, top)) = stack.last_mut() {
                    top.children.entry(name).or_default().push(finish(node));
                }
            }
            Ok(Event::End(_)) => {
                if stack.len() <= 1 {
                    break;
                }
                let (name, node) = stack.pop().expect("stack non-empty");
                if let Some((_, top)) = stack.last_mut() {
                    top.children.entry(name).or_default().push(finish(node));
                }
            }
            Ok(Event::Text(t)) => {
                if let Ok(txt) = t.unescape() {
                    if let Some((_, top)) = stack.last_mut() {
                        top.text.push_str(&txt);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            Ok(_) => {}
        }
        buf.clear();
    }

    if stack.len() != 1 {
        return None; // unbalanced document
    }
    let (_, root) = stack.pop().expect("stack non-empty");
    Some(finish(root).to_string())
}

/// Parse the `ease.net::request` opts (the shared shape for the async
/// drive and [`net_request_sync`]).
///
/// `opts` JSON: `{ url: str, method?: str (default GET), headers?: {name:
/// value}, body?: {text} | {base64} }`.
fn parse_request_opts(opts_json: &str) -> Result<tur_net_capability::RequestOpts, String> {
    #[derive(serde::Deserialize)]
    struct Opts {
        url: String,
        #[serde(default)]
        method: Option<String>,
        #[serde(default)]
        headers: serde_json::Map<String, serde_json::Value>,
        #[serde(default)]
        body: Option<serde_json::Value>,
    }

    let opts: Opts =
        serde_json::from_str(opts_json).map_err(|e| format!("ease:net: opts is not valid JSON: {e}"))?;

    let headers: Vec<(String, String)> = opts
        .headers
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                v.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| v.to_string()),
            )
        })
        .collect();
    let body = opts.body.as_ref().and_then(|b| {
        if let Some(t) = b.get("text").and_then(|v| v.as_str()) {
            Some(tur_net_capability::HttpBody::Text(t.to_string()))
        } else {
            b.get("base64")
                .and_then(|v| v.as_str())
                .and_then(|s| {
                    base64::engine::general_purpose::STANDARD
                        .decode(s.as_bytes())
                        .ok()
                })
                .map(tur_net_capability::HttpBody::Bytes)
        }
    });

    Ok(tur_net_capability::RequestOpts {
        url: opts.url,
        method: opts.method.unwrap_or_else(|| "GET".to_string()),
        headers,
        body,
        stream_buffer_bytes: None,
    })
}

/// The blocking one-shot ([`install`]'s `ease.net.request_sync`): drive the
/// capability request to completion on the shared tokio runtime and answer
/// the settled outcome as JSON
/// (`{"status","statusText","headers","body"<base64>,"error"}`).
fn net_request_sync(handles: &Rc<RutHandles>, opts_json: &str) -> String {
    let reply = |status: u64, status_text: &str, headers: &[(String, String)], body_b64: &str, error: &str| {
        let map: serde_json::Map<String, serde_json::Value> = headers
            .iter()
            .map(|(k, v)| (k.to_lowercase(), serde_json::Value::String(v.clone())))
            .collect();
        serde_json::json!({
            "status": status,
            "statusText": status_text,
            "headers": serde_json::Value::Object(map),
            "body": body_b64,
            "error": error,
        })
        .to_string()
    };

    let opts = match parse_request_opts(opts_json) {
        Ok(o) => o,
        Err(msg) => return reply(0, "", &[], "", &msg),
    };
    let Some(http) = handles
        .inst
        .capability()
        .of::<tur_net_capability::Http>()
        .map(|c| c.backend().clone())
    else {
        return reply(0, "", &[], "", "ease:net: no http capability on this instance");
    };

    // The capability future is executor-agnostic (waker-driven reqwest on
    // the registered backend) — driving it on the shared ease tokio runtime
    // never touches the worker lane, so a blocking row here cannot wedge
    // the VM's own drive.
    match ease_client_tokio::tokio_runtime().block_on(http.request(opts)) {
        tur_net_capability::HttpOutcome::Ok {
            status,
            status_text,
            headers,
            body,
        } => reply(
            status as u64,
            &status_text,
            &headers,
            &b64_encode(&body),
            "",
        ),
        tur_net_capability::HttpOutcome::Err(msg) => reply(0, "", &[], "", &msg),
    }
}

/// Parse the `ease.net::request` opts and start the drive on the instance's
/// worker spawner. `Err(msg)` = the request never fired (bad opts / no
/// capability) — the caller settles the task as failed.
fn net_start_drive(
    handles: &Rc<RutHandles>,
    task: &NetTask,
    opts_json: &str,
) -> Result<(), String> {
    let request = parse_request_opts(opts_json)?;
    let http = handles
        .inst
        .capability()
        .of::<tur_net_capability::Http>()
        .ok_or_else(|| "ease:net: no http capability on this instance".to_string())?
        .backend()
        .clone();

    let task2 = task.clone();
    handles.inst.spawn_local(move |_aw| async move {
        let outcome = http.request(request).await;
        task2.settle(outcome);
    });
    Ok(())
}
