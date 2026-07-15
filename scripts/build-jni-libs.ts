import { execSync } from "node:child_process";
import { COMPOSE_APP, ENVS, ROOT, RUST_LIBS_ROOTS, TARGETS } from "./base";
import path from "node:path";

console.log("Build ease-client in debug mode");
execSync(`cargo build -p ease-client-backend`, {
  stdio: "inherit",
  cwd: RUST_LIBS_ROOTS,
});

for (const buildTarget of TARGETS) {
  console.log(`Generate kotlin bindings`);
  execSync(
    `cargo run -p ease-client-android-ffi-builder generate --library ${path.resolve(RUST_LIBS_ROOTS, "./target/debug/libease_client_backend.so")} --language kotlin --out-dir ${path.resolve(COMPOSE_APP, "src/jvmShared/kotlin/")}`,
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

  if (!ENVS.Build) {
    console.log(`Copy host native library for desktop`);
    const libExt = process.platform === "darwin" ? "dylib" : "so";
    const hostLib = path.resolve(RUST_LIBS_ROOTS, `./target/debug/libease_client_backend.${libExt}`);
    const desktopNatives = path.resolve(COMPOSE_APP, "src/desktopMain/resources/natives");
    execSync(`mkdir -p ${desktopNatives} && cp ${hostLib} ${desktopNatives}/`, {
      stdio: "inherit",
    });
  }

  console.log(`Generate jniLibs for ${buildTarget}`);
  execSync(
    `cargo ndk --no-strip --target ${buildTarget} -o ${path.resolve(COMPOSE_APP, "src/androidMain/jniLibs")} build --release --lib`,
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
