import { execSync } from "node:child_process";
import { ROOT, RUST_LIBS_ROOTS, TARGETS } from "./base";
import path from "node:path";
import { readdirSync, rmSync } from "node:fs";

// Unified JSON+buffer bridge — no UniFFI bindgen step required.
// The Rust cdylib is cross-compiled directly; the JNI entrypoints
// (`EaseBridge.call`, `TurNative.*`, `EasePluginBridge.*`) are hand-written
// in `rust-libs/ease-client-android/src/` (bridge_jni.rs +
// plugin_runtime/plugin_jni.rs) — the Android embedder crate that links
// the platform-agnostic `ease-client-backend` as an rlib.

// The only .so the app loads via System.loadLibrary. Upstream deps
// (`boa_engine`, `redb`) declare `cdylib` crate-types, so cargo builds
// hash-named `libboa_engine-*.so` / `libredb-*.so` units too — and
// cargo-ndk blindly copies every `lib*.so` from the target dir into
// jniLibs, from where AGP ships them in the APK. Filter everything
// else back out.
const JNI_LIB = "libease_client_android.so";

for (const buildTarget of TARGETS) {
  // Wipe the ABI dir first: cargo ndk only *adds* files there, so stale
  // outputs would otherwise survive forever.
  const jniLibsAbiDir = path.resolve(ROOT, "android/app/src/main/jniLibs", buildTarget);
  rmSync(jniLibsAbiDir, { recursive: true, force: true });
  console.log(`Cross-compiling ease-client-android for ${buildTarget}`);
  execSync(
    `cargo ndk --platform 30 --target ${buildTarget} -o ${path.resolve(ROOT, "android/app/src/main/jniLibs")} build -p ease-client-android --release --lib`,
    {
      stdio: "inherit",
      cwd: RUST_LIBS_ROOTS,
      env: {
        ...process.env,
        RUST_BACKTRACE: "1",
      },
    },
  );
  for (const f of readdirSync(jniLibsAbiDir)) {
    if (f !== JNI_LIB) {
      rmSync(path.join(jniLibsAbiDir, f));
      console.log(`dropped stray jniLib ${f} (not ${JNI_LIB})`);
    }
  }
}
