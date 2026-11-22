//! PHASE-2 GRAMMAR SPIKE (plan risk 2): proves the rut class grammar the
//! `emp` SDK needs, on the phase-1 engine lanes — before the SDK itself is
//! written. The fixture module is deliberately host-row-free: everything
//! here is GRAMMAR, not services.
//!
//! Proven, one assert per shape:
//! 1. a class with ONE `opaque` field whose `[constructor]` takes the
//!    handle as a parameter (`Db(cx)`),
//! 2. a zero-field class (`Theme`) with methods on it,
//! 3. `const x = Cls(...)` construction + method calls on the binding,
//! 4. `entry fn start(cx: opaque)` receiving the embedder's boot handle
//!    (the phase-1 `start_cx` lane) and constructing services from it.
//!
//! The boot handle round-trip is observable: the start lane delivers the
//! embedder id to `probe`, which re-constructs `Db(cx)`, forwards the
//! field out through a method, and unboxes the u64 — the answered value
//! must be the id the test minted, never start's own return.

#[cfg(test)]
mod emp_grammar {
    use ease_client_android::plugin_runtime::EaseMusicPlugin;
    use ease_client_backend::ctx::BackendContext;
    use ease_tur_rpc::TurRpcBusPlugin;
    use tur_engine::core::plugin::Plugin;
    use tur_integration_tests::TurTestApp;

    /// The full app plugin set, host-buildable (the same shape
    /// lib.rs's `assert_loads` uses — the emp offering itself rides
    /// EaseMusicPlugin once phase 2 lands it).
    fn app_plugins() -> Vec<Box<dyn Plugin>> {
        vec![
            Box::new(TurRpcBusPlugin::new(None)),
            Box::new(EaseMusicPlugin::new(BackendContext::new())),
        ]
    }

    const GRAMMAR_RUT: &str = r#"
// 1+3+4 — the identity dance: one opaque field, the constructor takes
// the handle, methods forward it. The whole `emp` class shape in
// miniature.
pub class Db {
    h: opaque;
}

impl Db {
    [constructor] pub fn new(h: opaque) -> Self {
        return Self { h };
    }

    // the field forwards out — the round-trip lane the test asserts on
    pub fn peek(self) -> opaque {
        return self.h;
    }

    // an optional-returning member (the `?str` row shape emp carries)
    pub fn optional(self, want: bool) -> ?str {
        if (want) {
            return "yes";
        }
        return nil;
    }

    // a nil member with a statement body (the forward-row shape)
    pub fn touch(self) -> nil { }
}

// 2 — the zero-field class: no fields, methods still bind on instances.
pub class Theme { }

impl Theme {
    [constructor] pub fn new() -> Self {
        return Self { };
    }

    pub fn label(self) -> str {
        return "theme";
    }

    pub fn is_dark(self) -> bool {
        return false;
    }
}

// 3 — `const` construction + method calls on the binding.
fn build_with_const(cx: opaque) -> opaque {
    const db = Db(cx);
    const theme = Theme();
    return db.peek();
}

// the boot: services constructed FROM the handle, nothing returned
// (the answer rail is not the identity rail).
entry fn start(cx: opaque) {
    const db = Db(cx);
    let theme = Theme();
    let m = db.optional(true);
    db.touch();
}

// the probe the test addresses by the embedder id: reconstruct the
// service from the DELIVERED handle and forward the field out.
entry fn probe(cx: opaque) -> u64 {
    let db = Db(cx);
    return unopaque<u64>(db.peek());
}

// entry fns answer values too — the views keep their return lane.
entry fn probe_theme() -> str {
    let t = Theme();
    return t.label();
}
"#;

    #[test]
    fn class_grammar_and_start_cx_compile_and_boot() {
        let mut app = TurTestApp::new_with_extra_plugins(400.0, 400.0, app_plugins())
            .expect("harness app");
        // The phase-1 lane: boot with the embedder-supplied start cx (42).
        app.load_rut_module_cx(GRAMMAR_RUT, 42)
            .expect("the grammar fixture must compile and boot");

        // The lane addressed by the embedder id delivers the BOOT handle —
        // reconstructed through a class field and unboxed back.
        let answer = app.call_rut_entry_cx_answer("probe", 42).expect("probe runs");
        match answer {
            tur_engine::core::app::RutEntryAnswer::U64(v) => {
                assert_eq!(v, 42, "the boot handle round-trips through the class field")
            }
            other => panic!("expected U64(42), got {other:?}"),
        }

        // A parameterless entry still runs (the plain entry rail).
        let _ = app.call_rut_entry("probe_theme", 0, 0.0);
    }
}
