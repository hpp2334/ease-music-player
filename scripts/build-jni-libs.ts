import { execSync } from "node:child_process"
import { ROOT, RUST_LIBS_ROOTS } from "./base";
import path from "node:path";

const TARGETS = [
    // 'x86_64',
    'x86',
    // 'arm64-v8a',
    // 'armeabi-v7a'
]


// sys env "ANDROID_NDK_HOME"

const isProduction = Boolean(process.env.PRODUCTION)

console.log("Build ease-client in debug mode")
execSync(`cargo build -p ease-client`, {
    stdio: 'inherit',
    cwd: RUST_LIBS_ROOTS
});

for (const buildTarget of TARGETS) {
    console.log(`Generate kotlin file of ${buildTarget}`)
    execSync(`cargo run -p ease-client-android-ffi-builder generate --library ${path.resolve(RUST_LIBS_ROOTS, './target/debug/libease_client.so')} --language kotlin --out-dir ${path.resolve(ROOT, 'android/app/src/main/java/')}`, {
        stdio: 'inherit',
        cwd: RUST_LIBS_ROOTS,
        env: {
            ...process.env,
            RUST_BACKTRACE: '1',
        }
    })

    console.log(`Generate jniLibs of ${buildTarget}`)
    execSync(`cargo ndk --no-strip --target ${buildTarget} -o ${path.resolve(ROOT, 'android/app/src/main/jniLibs')} build --release --lib`, {
        stdio: 'inherit',
        cwd: RUST_LIBS_ROOTS,
        env: {
            ...process.env,
            RUST_BACKTRACE: '1',
        }
    })
}
