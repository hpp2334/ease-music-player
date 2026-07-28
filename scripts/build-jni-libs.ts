import { execSync } from "node:child_process";
import { ROOT, RUST_LIBS_ROOTS, TARGETS } from "./base";
import path from "node:path";

// Unified JSON+buffer bridge — no UniFFI bindgen step required.
// The Rust cdylib is cross-compiled directly; the single JNI entrypoint
// `Java_com_kutedev_easemusicplayer_singleton_EaseBridge_call` is
// hand-written in `rust-libs/ease-client-backend/src/bridge/jni.rs`.

for (const buildTarget of TARGETS) {
  console.log(`Cross-compiling ease-client-backend for ${buildTarget}`);
  execSync(
    `cargo ndk --platform 30 --target ${buildTarget} -o ${path.resolve(ROOT, "android/app/src/main/jniLibs")} build -p ease-client-backend --release --lib`,
    {
      stdio: "inherit",
      cwd: RUST_LIBS_ROOTS,
      env: {
        ...process.env,
        RUST_BACKTRACE: "1",
      },
    },
  );
}
