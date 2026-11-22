//! `crypto_host` bodies — OS entropy, UUID v4, MD5, Base64, SHA-256 +
//! HMAC-SHA-256 (the WebDAV/S3 support surface that replaced the old
//! npm deps `js-md5` / `@noble/hashes` / `uuid`). Identity-free: no
//! handle, no instance slot. The same bodies the old
//! `tur_host::ease::crypto` rows bind (D5) — one body source, two
//! namespaces.

use base64::Engine as _;
use rut_vm::TrapKind;

/// Bind the bodies under the `crypto_host` scope (scope-relative keys).
pub(crate) fn pkg() -> rut_vm::interp::HostPkg {
    let mut pkg = rut_vm::interp::HostPkg::new("crypto_host");

    rut_vm::pkg_fn!(pkg, "random_bytes", (u64,) -> Vec<u8>, move |_vm: &mut rut_vm::interp::Vm, n: u64| {
        random_bytes_body(n)
    });

    rut_vm::pkg_fn!(pkg, "random_uuid", () -> String, move |_vm: &mut rut_vm::interp::Vm| {
        Ok(uuid::Uuid::new_v4().to_string())
    });

    rut_vm::pkg_fn!(pkg, "md5_hex", (Vec<u8>,) -> String, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        Ok(md5_hex_body(&data))
    });

    rut_vm::pkg_fn!(pkg, "md5_base64", (Vec<u8>,) -> String, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        Ok(b64_encode(&md5_hex_raw(&data)))
    });

    rut_vm::pkg_fn!(pkg, "base64_encode", (Vec<u8>,) -> String, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        Ok(b64_encode(&data))
    });

    rut_vm::pkg_fn!(pkg, "base64_decode", (String,) -> Vec<u8>, move |_vm: &mut rut_vm::interp::Vm, s: String| {
        b64_decode_body(&s)
    });

    rut_vm::pkg_fn!(pkg, "sha256_hex", (Vec<u8>,) -> String, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        Ok(sha256_hex_body(&data))
    });

    rut_vm::pkg_fn!(pkg, "hmac_sha256_hex", (Vec<u8>, Vec<u8>) -> String, move |_vm: &mut rut_vm::interp::Vm, key: Vec<u8>, data: Vec<u8>| {
        hmac_sha256_hex_body(&key, &data)
    });

    pkg
}

// ---- the shared bodies (the old ease rows call these too) ------------

pub(crate) fn random_bytes_body(n: u64) -> Result<Vec<u8>, rut_vm::Trap> {
    let n = n.min(1 << 20) as usize;
    let mut buf = vec![0u8; n];
    getrandom::fill(&mut buf).map_err(|e| {
        rut_vm::Trap::new(
            rut_vm::TrapKind::Invalid,
            format!("ease:crypto random_bytes: {e}"),
        )
    })?;
    Ok(buf)
}

pub(crate) fn md5_hex_raw(data: &[u8]) -> [u8; 16] {
    use md5::Digest;
    let mut h = md5::Md5::new();
    h.update(data);
    h.finalize().into()
}

pub(crate) fn md5_hex_body(data: &[u8]) -> String {
    hex_encode(md5_hex_raw(data))
}

pub(crate) fn b64_decode_body(s: &str) -> Result<Vec<u8>, rut_vm::Trap> {
    base64::engine::general_purpose::STANDARD.decode(s.as_bytes()).map_err(|e| {
        rut_vm::Trap::new(
            TrapKind::Invalid,
            format!("ease:crypto base64_decode: {e}"),
        )
    })
}

pub(crate) fn sha256_hex_body(data: &[u8]) -> String {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(data);
    hex_encode(h.finalize())
}

pub(crate) fn hmac_sha256_hex_body(key: &[u8], data: &[u8]) -> Result<String, rut_vm::Trap> {
    use hmac::Mac;
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(key).map_err(|e| {
        rut_vm::Trap::new(
            rut_vm::TrapKind::Invalid,
            format!("ease:crypto hmac: {e}"),
        )
    })?;
    mac.update(data);
    Ok(hex_encode(mac.finalize().into_bytes()))
}

pub(crate) fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn b64_encode(bytes: impl AsRef<[u8]>) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes.as_ref())
}
