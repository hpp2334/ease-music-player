import { execSync } from "node:child_process";
import { BUILD_GRADLE_KTS, ENVS, ROOT, RUST_LIBS_ROOTS, TARGETS } from "./base";
import path from "node:path";
import { readFileSync } from "node:fs";

console.log("Build ease-client in debug mode");
execSync(`cargo build -p ease-client-backend`, {
  stdio: "inherit",
  cwd: RUST_LIBS_ROOTS,
});

for (const buildTarget of TARGETS) {
  console.log(`Generate kotlin file of ${buildTarget}`);
  execSync(
    `cargo run -p ease-client-android-ffi-builder generate --library ${path.resolve(RUST_LIBS_ROOTS, "./target/debug/libease_client_backend.so")} --language kotlin --out-dir ${path.resolve(ROOT, "shared/src/androidMain/kotlin/")}`,
    {
      stdio: "inherit",
      cwd: RUST_LIBS_ROOTS,
      env: {
        ...process.env,
        RUST_BACKTRACE: "1",
        CARGO_NDK_ANDROID_PLATFORM: "34",
      },
    },
  );

  console.log(`Generate jniLibs of ${buildTarget}`);
  execSync(
    `cargo ndk --no-strip --target ${buildTarget} -o ${path.resolve(ROOT, "androidApp/src/main/jniLibs")} build --release --lib`,
    {
      stdio: "inherit",
      cwd: RUST_LIBS_ROOTS,
      env: {
        ...process.env,
        RUST_BACKTRACE: "1",
        CARGO_NDK_ANDROID_PLATFORM: "34",
      },
    },
  );
}

console.log("Generate kotlin bindings for desktop");
execSync(
  `cargo run -p ease-client-android-ffi-builder generate --library ${path.resolve(RUST_LIBS_ROOTS, "./target/debug/libease_client_backend.so")} --language kotlin --out-dir ${path.resolve(ROOT, "shared/src/desktopMain/kotlin/")}`,
  {
    stdio: "inherit",
    cwd: RUST_LIBS_ROOTS,
    env: {
      ...process.env,
      RUST_BACKTRACE: "1",
    },
  },
);

console.log("Build desktop native library");
execSync(`cargo build --release --lib`, {
  stdio: "inherit",
  cwd: RUST_LIBS_ROOTS,
});
