//! The `ease` utility rows — the OLD surface (D5: alive until phase 5,
//! un-migrated plugins compile against it). The per-op WORK now lives
//! in the per-lib modules ([`super::third`]) — one body source shared
//! with the `crypto_host` / `xml_host` / `net_host` / `time_host` pkgs;
//! this installer keeps the `ease.crypto` / `ease.xml` / `ease.net` /
//! `ease.time` namespaces and their exact row set.
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

use rut_core::types::{TY_BYTES, TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64, TY_OPT_STR};
use tur_engine::core::rut_runtime::{host_row_at, RutPkgCx};

use crate::plugin_runtime::third::{crypto as third_crypto, net as third_net, time as third_time, xml as third_xml};

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

    // ---- crypto bodies (shared with `crypto_host` via third::crypto) ----------

    rut_vm::pkg_fn!(pkg, "ease::crypto::random_bytes", (u64,) -> Vec<u8>, move |_vm: &mut rut_vm::interp::Vm, n: u64| {
        third_crypto::random_bytes_body(n)
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::random_uuid", () -> String, move |_vm: &mut rut_vm::interp::Vm| {
        Ok(uuid::Uuid::new_v4().to_string())
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::md5_hex", (Vec<u8>,) -> String, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        Ok(third_crypto::md5_hex_body(&data))
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::md5_base64", (Vec<u8>,) -> String, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        Ok(third_crypto::b64_encode(third_crypto::md5_hex_raw(&data)))
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::base64_encode", (Vec<u8>,) -> String, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        Ok(third_crypto::b64_encode(&data))
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::base64_decode", (String,) -> Vec<u8>, move |_vm: &mut rut_vm::interp::Vm, s: String| {
        third_crypto::b64_decode_body(&s)
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::sha256_hex", (Vec<u8>,) -> String, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        Ok(third_crypto::sha256_hex_body(&data))
    });

    rut_vm::pkg_fn!(pkg, "ease::crypto::hmac_sha256_hex", (Vec<u8>, Vec<u8>) -> String, move |_vm: &mut rut_vm::interp::Vm, key: Vec<u8>, data: Vec<u8>| {
        third_crypto::hmac_sha256_hex_body(&key, &data)
    });

    // ---- xml bodies (shared with `xml_host` via third::xml) ---------------------

    rut_vm::pkg_fn!(pkg, "ease::xml::to_json", (Vec<u8>,) -> Option<String>, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        Ok(third_xml::xml_to_json(&data))
    });

    // ---- net bodies (shared with `net_host` via third::net) ----------------------

    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ease::net::request", (String,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, opts_json: String| {
        third_net::request_row(&h, vm, opts_json)
    });

    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ease::net::request_sync", (String,) -> String, move |_vm: &mut rut_vm::interp::Vm, opts_json: String| {
        Ok(third_net::net_request_sync(&h, &opts_json))
    });

    rut_vm::pkg_async_fn!(pkg, "ease::net::finish", (rut_vm::Opaque<third_net::NetTask>,) -> Vec<u8>, move |t: rut_vm::Opaque<third_net::NetTask>| {
        third_net::finish_body(t)
    });

    rut_vm::pkg_fn!(pkg, "ease::net::status", (rut_vm::Opaque<third_net::NetTask>,) -> u64, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<third_net::NetTask>| {
        Ok(third_net::status_body(&t))
    });

    rut_vm::pkg_fn!(pkg, "ease::net::status_text", (rut_vm::Opaque<third_net::NetTask>,) -> String, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<third_net::NetTask>| {
        Ok(third_net::status_text_body(&t))
    });

    rut_vm::pkg_fn!(pkg, "ease::net::headers", (rut_vm::Opaque<third_net::NetTask>,) -> String, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<third_net::NetTask>| {
        Ok(third_net::headers_body(&t))
    });

    rut_vm::pkg_fn!(pkg, "ease::net::body", (rut_vm::Opaque<third_net::NetTask>,) -> Vec<u8>, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<third_net::NetTask>| {
        Ok(third_net::body_body(&t))
    });

    rut_vm::pkg_fn!(pkg, "ease::net::error", (rut_vm::Opaque<third_net::NetTask>,) -> String, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<third_net::NetTask>| {
        Ok(third_net::error_body(&t))
    });

    // ---- time bodies (clock + parse shared with `time_host` via third::time;
    //      the formatters are the OLD rows' Rust truth — the lib spells them
    //      in pure rut) -----------------------------------------------------------

    rut_vm::pkg_fn!(pkg, "ease::time::now_unix_ms", () -> u64, move |_vm: &mut rut_vm::interp::Vm| {
        Ok(third_time::now_unix_ms())
    });

    rut_vm::pkg_fn!(pkg, "ease::time::format_http_date", (u64,) -> String, move |_vm: &mut rut_vm::interp::Vm, ms: u64| {
        Ok(third_time::format_http_date(ms))
    });

    rut_vm::pkg_fn!(pkg, "ease::time::format_amz_date", (u64,) -> String, move |_vm: &mut rut_vm::interp::Vm, ms: u64| {
        Ok(third_time::format_amz_date(ms))
    });

    rut_vm::pkg_fn!(pkg, "ease::time::parse_http_date", (String,) -> u64, move |_vm: &mut rut_vm::interp::Vm, s: String| {
        Ok(third_time::parse_http_date(&s).unwrap_or(0))
    });

    rut_vm::pkg_fn!(pkg, "ease::time::unix_ms", (f64,) -> u64, move |_vm: &mut rut_vm::interp::Vm, secs: f64| {
        // convenience: `seconds since epoch` (the old JS `Date.now()/1000`
        // round-trips) → unix ms
        Ok((secs * 1000.0) as u64)
    });
}
