//! Decisive semantics probes for the rut pin tur@3b34983 ships (the things
//! compile+boot checks cannot see): does state written through an
//! `opaque::downcast` binding persist across entry calls — via the binding
//! itself, via a copy, and via a parameter? (Bindings are mutable by
//! default — the drop-mut law; the `mut` keyword is gone from the grammar.)

#[cfg(test)]
mod probes {
    use ease_client_android::plugin_runtime::EaseMusicPlugin;
    use ease_client_backend::ctx::BackendContext;
    use ease_tur_rpc::TurRpcBindPlugin;
    
    use tur_integration_tests::TurTestApp;

    /// `set_in_place` writes through the downcast binding directly;
    /// `set_via_copy` writes through a copy; `set_via_param`
    /// takes the record as a parameter and writes there. `get`
    /// answers all three. If value semantics rule, only the first
    /// persists; if downcast yields a cell alias and copies alias the
    /// same cell, all three do.
    const MUTATION_RUT: &str = r#"
struct Ctx {
    in_place: i64,
    via_copy: i64,
    via_param: i64,
}

entry fn start() -> opaque {
    return opaque(Ctx { in_place: 0, via_copy: 0, via_param: 0 });
}

entry fn set_in_place(cx: opaque) -> str {
    let c = opaque::downcast<Ctx>(cx);
    if (c == nil) {
        panic("set_in_place: cx");
    }
    c.in_place = 1;
    return "{\"ok\":true,\"result\":true}";
}

entry fn set_via_copy(cx: opaque) -> str {
    let c = opaque::downcast<Ctx>(cx);
    if (c == nil) {
        panic("set_via_copy: cx");
    }
    let cc: Ctx = c;
    cc.via_copy = 1;
    return "{\"ok\":true,\"result\":true}";
}

entry fn set_via_param(cx: opaque) -> str {
    let c = opaque::downcast<Ctx>(cx);
    if (c == nil) {
        panic("set_via_param: cx");
    }
    let cc: Ctx = c;
    write_param(cc);
    return "{\"ok\":true,\"result\":true}";
}

fn write_param(p: Ctx) {
    p.via_param = 1;
}

entry fn get(cx: opaque) -> str {
    let c = opaque::downcast<Ctx>(cx);
    if (c == nil) {
        panic("get: cx");
    }
    return f"{{\"ok\":true,\"result\":{{\"in_place\":{c.in_place},\"via_copy\":{c.via_copy},\"via_param\":{c.via_param}}}}}";
}
"#;

    #[test]
    fn documents_ctx_mutation_semantics() {
        let app = TurTestApp::new_with_extra_plugins(
            400.0,
            400.0,
            vec![
                Box::new(TurRpcBindPlugin::new()),
                Box::new(EaseMusicPlugin::new(BackendContext::new())),
            ],
        )
        .expect("harness app");
        app.load_rut_module(MUTATION_RUT).expect("fixture loads");

        app.call_rut_entry_cx_str("set_in_place", app.rut_start_answer())
            .expect("set_in_place");
        app.call_rut_entry_cx_str("set_via_copy", app.rut_start_answer())
            .expect("set_via_copy");
        app.call_rut_entry_cx_str("set_via_param", app.rut_start_answer())
            .expect("set_via_param");
        let state = app
            .call_rut_entry_cx_str("get", app.rut_start_answer())
            .expect("get");
        // THE LAW this pins: rut's downcast yields a cell alias, struct
        // bindings share that cell (no copy), and params write through
        // (mutable by default). The plugins' persistent-state pattern (ctx
        // record in `start() -> opaque`, mutated through copies and params)
        // depends on it — if a future rut pin turns copies into value
        // semantics, this fails and every session/config cache needs a
        // redesign.
        assert_eq!(
            state,
            "{\"ok\":true,\"result\":{\"in_place\":1,\"via_copy\":1,\"via_param\":1}}",
            "rut ctx-mutation semantics drifted — the plugins' state pattern depends on cell-aliasing"
        );
    }
}
