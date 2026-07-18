import path from "path";

export const ROOT = path.resolve(__dirname, "../");
export const RUST_LIBS_ROOTS = path.resolve(ROOT, "./rust-libs");
export const BUILD_GRADLE_KTS = path.resolve(
  ROOT,
  "android/app/build.gradle.kts",
);
export const TARGETS = ["arm64-v8a"];
