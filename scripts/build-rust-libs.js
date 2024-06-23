const path = require('path')
const { execSync } = require('child_process')
const { ROOT } = require('./base')

const extraEnvs = {
    ANDROID_NDK_HOME: "D:\\Android_SDK\\ndk"
}


const TARGETS = [
    'x86_64',
    'x86',
    // 'arm64-v8a',
    // 'armeabi-v7a'
]


for (const buildTarget of TARGETS) {
    execSync(`cargo build --lib`, {
        stdio: 'inherit',
        env: {
            ...process.env,
            ...extraEnvs,
        }
    });
    execSync(`cargo run --features=uniffi/cli --bin uniffi-bindgen generate --library target/debug/unffi_playground.dll --language kotlin --out-dir ${path.resolve(ROOT, 'app/src/main/java/unffi_playground')}`, {
        stdio: 'inherit',
        env: {
            ...process.env,
            ...extraEnvs,
        }
    })
    execSync(`cargo ndk --no-strip --target ${buildTarget} -o ${path.resolve(ROOT, 'app/src/main/jniLibs')} build --release --lib`, {
        stdio: 'inherit',
        env: {
            ...process.env,
            ...extraEnvs,
        }
    })
}
