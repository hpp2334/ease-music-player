//! `net_host` bodies — full-featured HTTP over the engine's `Http`
//! capability (upstream's rut net rows carry no headers/body; every
//! provider needs `Authorization` / `Depth` / PROPFIND bodies).
//! Identity-free (the capability resolves off the instance, never a
//! plugin).

use std::cell::RefCell;
use std::rc::Rc;

use base64::Engine as _;
use tur_engine::core::rut_runtime::RutHandles;

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

/// Bind the bodies under the `net_host` scope (scope-relative keys).
/// `finish` is the one async row (the five-row family expands at the
/// mount).
pub(crate) fn pkg(handles: &Rc<RutHandles>) -> rut_vm::interp::HostPkg {
    let mut pkg = rut_vm::interp::HostPkg::new("net_host");

    // `request(opts: str) -> opaque` — allocate the task, start the drive.
    // Sync (the drive rides `spawn_local`); await via `finish`. The answer
    // crosses as the bare `OpaqueRef` (its `Ret::TY` is `TY_OPAQUE`, so the
    // decl-row signature check passes — a typed `Opaque<NetTask>` return
    // carries `u32::MAX` and drifts every boot).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "request", (String,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, opts_json: String| {
        request_row(&h, vm, opts_json)
    });

    // `request_sync(opts: str) -> str` — the blocking one-shot. Fires the
    // request and answers a JSON envelope
    // `{"status": u64, "statusText": str, "headers": {name: value},
    //   "body": <base64>, "error": str}` (`body` "" + `error` set on a
    // transport failure). Runs the capability future to completion on the
    // calling thread via the shared tokio runtime — **entry-fn-safe**:
    // the drive lives on the tokio runtime, never the worker lane, so op
    // entries can send+await in one row call (no settled-task spin).
    // `opts` shape: see [`parse_request_opts`].
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "request_sync", (String,) -> String, move |_vm: &mut rut_vm::interp::Vm, opts_json: String| {
        Ok(net_request_sync(&h, &opts_json))
    });

    // `finish(task: opaque) -> bytes` — await the drive; the body (empty on
    // a transport failure — read `error`).
    rut_vm::pkg_async_fn!(pkg, "finish", (rut_vm::Opaque<NetTask>,) -> Vec<u8>, move |t: rut_vm::Opaque<NetTask>| {
        finish_body(t)
    });

    rut_vm::pkg_fn!(pkg, "status", (rut_vm::Opaque<NetTask>,) -> u64, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<NetTask>| {
        Ok(status_body(&t))
    });

    rut_vm::pkg_fn!(pkg, "status_text", (rut_vm::Opaque<NetTask>,) -> String, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<NetTask>| {
        Ok(status_text_body(&t))
    });

    rut_vm::pkg_fn!(pkg, "headers", (rut_vm::Opaque<NetTask>,) -> String, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<NetTask>| {
        Ok(headers_body(&t))
    });

    rut_vm::pkg_fn!(pkg, "body", (rut_vm::Opaque<NetTask>,) -> Vec<u8>, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<NetTask>| {
        Ok(body_body(&t))
    });

    rut_vm::pkg_fn!(pkg, "error", (rut_vm::Opaque<NetTask>,) -> String, move |_vm: &mut rut_vm::interp::Vm, t: rut_vm::Opaque<NetTask>| {
        Ok(error_body(&t))
    });

    pkg
}

// ---------------------------------------------------------------------------
// the shared bodies (the `ease.net` rows call these too)
// ---------------------------------------------------------------------------

/// `request`'s body: allocate the task + start the drive; the answer is
/// the bare box handle.
pub(crate) fn request_row(
    handles: &Rc<RutHandles>,
    vm: &mut rut_vm::interp::Vm,
    opts_json: String,
) -> Result<rut_vm::OpaqueRef, rut_vm::Trap> {
    let task = NetTask::new();
    if let Err(msg) = net_start_drive(handles, &task, &opts_json) {
        task.settle(tur_net_capability::HttpOutcome::Err(msg));
    }
    Ok(rut_vm::Opaque::alloc(vm, task)?.handle().clone())
}

/// `finish`'s body: the completer the async lane drives.
pub(crate) fn finish_body(t: rut_vm::Opaque<NetTask>) -> rut_vm::Completer<Vec<u8>> {
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
}

pub(crate) fn status_body(t: &rut_vm::Opaque<NetTask>) -> u64 {
    t.with(|t| *t.status.borrow() as u64).unwrap_or(0)
}

pub(crate) fn status_text_body(t: &rut_vm::Opaque<NetTask>) -> String {
    t.with(|t| t.status_text.borrow().clone()).unwrap_or_default()
}

pub(crate) fn headers_body(t: &rut_vm::Opaque<NetTask>) -> String {
    let headers = t.with(|t| t.headers.borrow().clone()).unwrap_or_default();
    let map: serde_json::Map<String, serde_json::Value> = headers
        .iter()
        .map(|(k, v)| (k.to_lowercase(), serde_json::Value::String(v.clone())))
        .collect();
    serde_json::Value::Object(map).to_string()
}

pub(crate) fn body_body(t: &rut_vm::Opaque<NetTask>) -> Vec<u8> {
    t.with(|t| t.body.borrow().clone()).unwrap_or_default()
}

pub(crate) fn error_body(t: &rut_vm::Opaque<NetTask>) -> String {
    t.with(|t| t.error.borrow().clone()).unwrap_or_default()
}

/// Parse the net request opts (the shared shape for the async drive and
/// [`net_request_sync`]).
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

/// The blocking one-shot (`request_sync`): drive the capability request
/// to completion on the shared tokio runtime and answer the settled
/// outcome as JSON
/// (`{"status","statusText","headers","body"<base64>,"error"}`).
pub(crate) fn net_request_sync(handles: &Rc<RutHandles>, opts_json: &str) -> String {
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
            &crate::plugin_runtime::third::crypto::b64_encode(&body),
            "",
        ),
        tur_net_capability::HttpOutcome::Err(msg) => reply(0, "", &[], "", &msg),
    }
}

/// Parse the request opts and start the drive on the instance's
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
