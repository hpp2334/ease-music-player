import path from "path";

export const ROOT = path.resolve(__dirname, "../");
export const RUST_LIBS_ROOTS = path.resolve(ROOT, "./rust-libs");
export const COMPOSE_APP = path.resolve(ROOT, "./composeApp");
export const ENVS = {
  Build: Boolean(process.env.EBUILD),
};

export const BUILD_GRADLE_KTS = path.resolve(COMPOSE_APP, "./build.gradle.kts");
export const TARGETS = ["arm64-v8a"];
