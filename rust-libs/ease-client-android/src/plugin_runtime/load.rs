//! The cx-bearing module-load lane — the embedder side of the phase-1
//! engine rails. `tur_android::ops::load_module` has no start-argument
//! parameter, so the embedder drives the SAME engine lanes itself
//! ([`tur_android::ops::with_runtime`] for the source registries,
//! [`tur_android::ops::with_app`] for the load) and passes the
//! instance's minted handle id through `load_rut_module_source_cx` /
//! `load_rut_module_files_cx`. FIFO op order on the tur-host thread
//! preserves the `createInstance` → load ordering the plain op gave.
//!
//! Callers: the headless spawn path ([`super::engine`]) and the view
//! load trampoline ([`super::plugin_jni`]) — both mint a
//! [`super::emp::PluginHandle`] at instance build, stamp it into the
//! instance's data, and pass its id here.

#[cfg(target_os = "android")]
pub(crate) fn load_module_source(
    runtime_handle: i64,
    instance_handle: i64,
    source_handle: i64,
    start_cx: Option<u64>,
) {
    // POST-SHAPED like the old `ops::load_module` lane: the blocking
    // `with_runtime` / `with_app` round-trips run on a side thread, so the
    // caller (the JNI main thread for views, a tokio worker for the
    // headless reload) returns immediately. Ordering is safe by
    // construction: the caller only spawns after `createInstance` /
    // `spawn_headless` returned — the instance-build op is already queued
    // on the tur-host thread, and this thread's `with_app` enqueues after
    // it (FIFO).
    std::thread::spawn(move || {
        let registries = tur_android::ops::with_runtime(runtime_handle, |rt| {
            (rt.module_sources.clone(), rt.module_files.clone())
        });
        let Some((sources, files)) = registries else {
            tracing::error!(
                "loadModule: runtime handle {runtime_handle} not found (stale binding) — load skipped"
            );
            return;
        };
        let posted = tur_android::ops::with_app(instance_handle, move |app| {
            // Multi-file rut modules first (the file-set registry lane), then
            // the plain single-source lane — the same dispatch
            // `tur_android::ops::load_module` does, with the cx threaded.
            if let Some(f) = files.get(source_handle as u64) {
                tracing::info!(
                    "loadModule: file set @ handle {source_handle} (root {} bytes, {} mod children, start cx {start_cx:?})",
                    f.root.len(),
                    f.mods.len()
                );
                let result = match start_cx {
                    Some(cx) => futures::executor::block_on(app.load_rut_module_files_cx(f, cx)),
                    None => futures::executor::block_on(app.load_rut_module_files(f)),
                };
                match result {
                    Ok(()) => tracing::info!("loadModule: rut module booted OK (multi-file)"),
                    Err(e) => tracing::error!("loadModule: rut module load failed: {e}"),
                }
                return;
            }
            tracing::info!(
                "loadModule: source handle {source_handle} ({} bytes, start cx {start_cx:?})",
                match sources.get(source_handle as u64) {
                    Some(tur_engine::ModuleSourceEntry::Files(files)) => files.root.len(),
                    Some(tur_engine::ModuleSourceEntry::Source(s)) => s.len(),
                    None => 0,
                }
            );
            let result = match start_cx {
                Some(cx) => {
                    futures::executor::block_on(app.load_rut_module_source_cx(&sources, source_handle as u64, cx))
                }
                None => futures::executor::block_on(app.load_rut_module_source(&sources, source_handle as u64)),
            };
            match result {
                Ok(()) => {
                    tracing::info!("loadModule: rut module booted OK");
                    tracing::info!("loadModule: paint requested");
                }
                Err(e) => tracing::error!("loadModule: rut module load failed: {e}"),
            }
        });
        if posted.is_none() {
            tracing::warn!(
                "loadModule: instance {instance_handle} not present (build failed or destroyed) — load skipped"
            );
        }
    });
}

/// Host-build stub: there is never a live runtime to load against.
#[cfg(not(target_os = "android"))]
pub(crate) fn load_module_source(
    _runtime_handle: i64,
    _instance_handle: i64,
    _source_handle: i64,
    _start_cx: Option<u64>,
) {
}
