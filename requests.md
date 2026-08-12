# Feature Request: tur Engine — Generic Per-Instance Data Slot

## 1. Background

The Ease Music Player embeds the tur engine as a **plugin system**. Third-party
plugins (e.g. `com.ease.onedrive`) ship two JS bundles:

- **Headless service** (`index.ts`) — runs in a headless tur instance, registers
  RPC handlers for storage operations (list, read, stream).
- **Setup view** (`setup.ts`) — runs in a rendering tur instance, draws a config
  form (alias input + "Connect" button) using `tur:std` widgets.

The host app exposes a custom `ease` module (storage, secret, oauth, themes)
that plugin JS calls to interact with the app's data layer. The bridge functions
behind this module need to know **which plugin owns the calling instance** to
enforce storage/secret scoping — a plugin must never read another plugin's data.

Grouped namespace object exports (`import { storage } from "ease"`) are already
possible today via `PluginContext::boa_mut()` (pub) + `register_module` consts.

The remaining gap: **tur instances carry no embedder-provided metadata.** The
embedder knows contextual information at `createInstance` time (in our case, a
plugin ID), but there's no public mechanism to stash it on the instance and read
it back from bridge functions.

---

## 2. Problem — No Per-Instance Metadata Slot

### Current state

Each tur instance is an isolated JS realm. The embedder decides contextual
metadata when creating an instance:

```kotlin
// Kotlin host knows this instance is for the OneDrive plugin:
val instance = runtime.createInstance(surface, w, h, dpr)
instance.loadModule(setupJs) // setup.js for com.ease.onedrive
```

But `createInstance` doesn't accept metadata, and `TurJsContext` (the per-instance
context passed to ctx-bound bridge functions) carries only engine-internal state
(`element_tree`, `mutation_queue`, `focus_manager`, scheduler handles, …). There
is no public slot for embedder-provided per-instance data.

The workaround: **plugin JS passes the metadata** as a function argument:

```typescript
storage.get("com.ease.onedrive", "myKey"); // plugin ID as a JS argument
```

This is **insecure** — a malicious plugin can pass `"com.ease.other"` to access
another plugin's storage or secrets. The bridge has no way to verify the true
caller.

### Why this should be generic (not plugin-specific)

`pluginId` is **our app's concept** — tur is a general-purpose rendering engine
and shouldn't know about plugins. Different embedders have different metadata
needs (tenant ID, user ID, feature flags, sandbox policy, …). The engine should
provide a **generic typed data slot**; each embedder defines its own metadata
type and stores/reads it. In our case, we define `struct PluginId(String)`.

---

## 3. Proposed Solution — Typed Per-Instance Data Map

Add a `HashMap<TypeId, Box<dyn Any>>` to `TurJsContext`, accessible via
`insert_data::<T>()` / `get_data::<T>()`.

### Engine changes

```rust
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;

pub struct TurJsContext {
    // ... existing fields ...

    /// Embedder-provided per-instance metadata. Typed key → boxed value.
    /// Set by the embedder at instance creation; read by ctx-bound bridge
    /// functions during JS execution. Never accessible to JS itself.
    instance_data: RefCell<HashMap<TypeId, Box<dyn Any>>>,
}

impl TurJsContext {
    pub fn insert_data<T: Any>(&self, value: T) {
        self.instance_data
            .borrow_mut()
            .insert(TypeId::of::<T>(), Box::new(value));
    }

    pub fn get_data<T: Any>(&self) -> Option<&T> {
        self.instance_data
            .borrow()
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
    }
}
```

The embedder sets it right after instance creation. The easiest seam: a new
`TurApp::insert_data::<T>()` that forwards to the `TurJsContext`, callable via
the existing `with_app(handle, |app| ...)` escape hatch (already pub).

```rust
impl TurApp {
    pub fn insert_data<T: Any>(&self, value: T) {
        self.backend.js_ctx().insert_data(value);
    }

    pub fn get_data<T: Any>(&self) -> Option<&T> {
        self.backend.js_ctx().get_data::<T>()
    }
}
```

No changes to `createInstance` signatures needed — the embedder stamps data via
`with_app` immediately after creation:

```rust
let h = ops::create_instance(env, runtime_handle, surface, ...);
with_app(h, |app| app.insert_data(PluginId(plugin_id.to_string())));
```

---

## 4. End-to-End Usage Example (Ease Music Player)

### App-defined metadata type (in `ease-client-backend`)

```rust
/// The plugin that owns this tur instance. Stamped at creation by the host;
/// read by `ease` bridge functions to enforce per-plugin scoping.
#[derive(Debug, Clone)]
pub struct PluginId(pub String);
```

### Kotlin (instance creation + stamping)

```kotlin
// View instance for the OneDrive setup form:
val instance = runtime.createInstance(surface, w, h, dpr)
EasePluginBridge.stampPluginId(instance.nativeHandle(), "com.ease.onedrive")
instance.loadModule(setupJs)
```

```rust
// JNI trampoline (plugin_jni.rs):
#[unsafe(no_mangle)]
pub extern "system" fn Java_..._EasePluginBridge_stampPluginId(
    _env: JNIEnv, _class: JClass, handle: jlong, plugin_id: JString,
) {
    let id: String = env.get_string(&plugin_id).unwrap().into();
    tur_android::ops::with_app(handle, |app| {
        app.insert_data(PluginId(id));
    });
}
```

### Rust (bridge function — secure identity)

```rust
/// `storage.get(key)` — the calling plugin's identity comes from the
/// per-instance `PluginId` data slot, NOT from a JS argument.
fn storage_get(
    _this: &JsValue,
    args: &[JsValue],
    js_ctx: &TurJsContext,
    _ctx: &mut Context,
) -> JsResult<JsValue> {
    let plugin_id = js_ctx.get_data::<PluginId>().ok_or_else(|| {
        JsError::from(JsNativeError::typ()
            .with_message("ease: no plugin context bound to this instance"))
    })?;
    let key = require_string(args, 1)?;
    let backend = BACKEND_CONTEXT.get().unwrap();
    match backend.plugin_storage.get(&plugin_id.0, &key) {
        Some(v) => Ok(JsValue::from(js_string!(v.as_str()))),
        None => Ok(JsValue::null()),
    }
}
```

### Plugin JS (no pluginId anywhere)

```typescript
import { storage, secret, oauth, themes } from "ease";

// The host knows who we are — no identity argument:
const value = storage.get("myKey");
secret.put("refresh-token");
oauth.start("onedrive", "my-alias");
themes.primary(); // "#2E89B0"
```

### Shared TypeScript declarations (`plugins/infra/ease.d.ts`)

```typescript
declare module "ease" {
    export const storage: {
        get(key: string): string | null;
        set(key: string, value: string): void;
        delete(key: string): void;
    };
    export const secret: {
        get(secretId: number): string | null;
        put(secret: string): number;
        remove(secretId: number): void;
    };
    export const oauth: {
        start(provider: string, alias: string | null): void;
    };
    export const themes: {
        primary(): string;
        onPrimary(): string;
        primaryContainer(): string;
        onPrimaryContainer(): string;
        secondary(): string;
        onSecondary(): string;
        background(): string;
        onBackground(): string;
        surface(): string;
        onSurface(): string;
        surfaceVariant(): string;
        onSurfaceVariant(): string;
        outline(): string;
        outlineVariant(): string;
        error(): string;
        onError(): string;
        isDark(): boolean;
    };
}
```

---

## 5. Summary

| Need | Root cause | Impact | Proposed fix |
| --- | ---------- | ------ | ------------ |
| Bridge functions need per-instance embedder metadata | `TurJsContext` has no public data slot; all fields are engine-internal | Embedders must pass metadata through JS arguments (insecure — spoofable) | Add `HashMap<TypeId, Box<dyn Any>>` to `TurJsContext` with `insert_data::<T>()` / `get_data::<T>()`, exposed on `TurApp` via `with_app`. Generic — each embedder defines its own metadata type. |
