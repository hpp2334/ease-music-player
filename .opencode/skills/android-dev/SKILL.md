---
name: android-dev
description: Use when building, signing, installing, or debugging the Ease Music Player Android app on a device or emulator. Covers cargo ndk and gradlew assembleRelease or assembleDebug, signing an unsigned APK with apksigner, reading Rust panic backtraces in logcat, and the macOS Sequoia adb local-network block workaround using a python3 localhost proxy. Trigger on adb connect, adb pair, wireless debugging, device offline, No route to host, SIGABRT, INSTALL_PARSE_FAILED_NO_CERTIFICATES, or panic stack.
---

# Ease Music Player Android on-device debug

Everything needed to get the Ease Music Player running on an Android device and to
keep `adb` talking to it from macOS. The native engine is `ease-client-backend`
(a `cdylib` — `libease_client_backend.so`); the shell is `android/app`
(Kotlin/Compose).

Environment: Android cmdline-tools at `/usr/local/share/android-commandlinetools`,
NDK `27.0.12077973`, JDK 21 at `/usr/local/opt/openjdk@21`
(set `JAVA_HOME`). adb at `…/platform-tools/adb`. `cargo-ndk` must be installed.

## Build

Build the Rust cdylib + UniFFI bindings + debug APK:

```sh
export JAVA_HOME=/usr/local/opt/openjdk@21
export ANDROID_SDK_ROOT=/usr/local/share/android-commandlinetools
export ANDROID_NDK_HOME=/usr/local/share/android-commandlinetools/ndk/27.0.12077973
export PATH="$JAVA_HOME/bin:$ANDROID_SDK_ROOT/cmdline-tools/latest/bin:$PATH"

pnpm build:jni          # Rust host build → UniFFI bindings → cargo ndk cross-compile
cd android && ./gradlew :app:assembleDebug
# → android/app/build/outputs/apk/debug/app-arm64-v8a-debug.apk
```

For a release APK (with signing keys):

```sh
EBUILD=1 pnpm build:jni && npx tsx ./scripts/build-apk.ts
# requires ANDROID_SIGN_JKS (brotli+base64) and ANDROID_SIGN_PASSWORD env vars
```

## Install

```sh
adb -s <device> install -r -t android/app/build/outputs/apk/debug/app-arm64-v8a-debug.apk
adb -s <device> shell am start -n com.kutedev.easemusicplayer/.MainActivity
```

## Sign a release APK (if unsigned)

```sh
BT=/usr/local/share/android-commandlinetools/build-tools/35.0.0
SRC=android/app/build/outputs/apk/release/app-arm64-v8a-release.apk
$BT/zipalign -p -f 4 "$SRC" /tmp/ease-signed.apk
JAVA_HOME=/usr/local/opt/openjdk@21 $BT/apksigner sign \
    --ks ~/.android/debug.keystore --ks-pass pass:android \
    --ks-key-alias androiddebugkey --key-pass pass:android /tmp/ease-signed.apk
adb -s <device> install -r -t /tmp/ease-signed.apk
```

## adb over wireless + the macOS Sequoia local-network block

On Android 11+ the device's **Wireless debugging** exposes a *connect* port
(shown on the main wireless-debugging screen) plus a separate short-lived
*pairing* port (shown under "Pair device with pairing code"). Pair once with
`adb pair <host> <pair_port> <code>`, then `adb connect <host> <connect_port>`.

**macOS Sequoia silently blocks the `adb` binary from the local network.**
Symptom: `nc` (`/usr/bin/nc`) and `python3` (`/usr/bin/python3`) — both
Apple-signed — connect to the device fine, but `adb connect` fails instantly
with `No route to host` (`EHOSTUNREACH`). Workaround: bridge adb through a
localhost proxy run by Apple-signed `python3`.

Save as `/tmp/adb_proxy.py`:

```python
import socket, threading, sys
local_port, target_host, target_port = int(sys.argv[1]), sys.argv[2], int(sys.argv[3])
def forward(src, dst):
    try:
        while True:
            d = src.recv(65536)
            if not d: break
            dst.sendall(d)
    except Exception: pass
    finally:
        try: dst.shutdown(socket.SHUT_WR)
        except Exception: pass
def handle(client):
    try: up = socket.create_connection((target_host, target_port), timeout=5)
    except Exception as e: print(f"upstream fail: {e}"); client.close(); return
    t = threading.Thread(target=forward, args=(client, up), daemon=True); t.start()
    forward(up, client); t.join(timeout=3)
    for s in (client, up):
        try: s.close()
        except Exception: pass
srv = socket.socket(); srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
srv.bind(("127.0.0.1", local_port)); srv.listen(16)
print(f"proxy 127.0.0.1:{local_port} -> {target_host}:{target_port}", flush=True)
while True:
    c, _ = srv.accept(); threading.Thread(target=handle, args=(c,), daemon=True).start()
```

Then:

```sh
nohup /usr/bin/python3 /tmp/adb_proxy.py 7777 192.168.124.36 40011 >/tmp/adb_proxy.log 2>&1 &
adb connect 127.0.0.1:7777
adb -s 127.0.0.1:7777 devices -l
```

- **Once connected, immediately lock adbd to a fixed port** (`adb tcpip 5555`):
  the wireless-debugging port changes every time adbd restarts, which forces
  a re-scan + proxy retarget each session. Locking makes the device always
  reachable at `<device-ip>:5555`. The current link drops when adbd restarts
  — that's expected: kill the proxy, restart it targeting `<device-ip>:5555`,
  reconnect, and verify with `adb shell getprop service.adb.tcp.port` (= 5555).
  ```sh
  adb -s 127.0.0.1:7777 tcpip 5555   # adbd restarts on the fixed port
  # proxy: kill + restart with target port 5555, then:
  adb connect 127.0.0.1:7777
  ```
- If `adb` reports `device offline`: `adb disconnect 127.0.0.1:7777 && adb connect 127.0.0.1:7777`.
- If the device's connect port changed (device not locked yet), re-check it on the device and restart the proxy.

## Reading logs on-device

```sh
adb -s <device> logcat -d -v time --pid=$(adb -s <device> shell pidof com.kutedev.easemusicplayer)
```

Rust logs appear under the `RustStdoutStderr` tag (or whatever tag the backend uses).

## Crash diagnostics (readable Rust panic stacks in logcat)

A panic inside the Rust backend would show as `Fatal signal 6 (SIGABRT)`. For
diagnosable backtraces:

1. The `.symtab` must be preserved in the packaged `.so`. AGP's
   `stripReleaseDebugSymbols` strips it from prebuilt jniLibs; ensure the
   build keeps it.
2. If using a custom panic hook, capture `std::backtrace::Backtrace::force_capture()`
   and log line-by-line at `ERROR`.

## Rebuilding after a Rust change

```sh
export JAVA_HOME=/usr/local/opt/openjdk@21
export ANDROID_SDK_ROOT=/usr/local/share/android-commandlinetools
export ANDROID_NDK_HOME=/usr/local/share/android-commandlinetools/ndk/27.0.12077973
export PATH="$JAVA_HOME/bin:$ANDROID_SDK_ROOT/cmdline-tools/latest/bin:$PATH"

# Just the native lib (skip UniFFI rebind if API surface unchanged):
cargo ndk --platform 30 --target arm64-v8a -o android/app/src/main/jniLibs build -p ease-client-backend --release --lib
cd android && ./gradlew :app:assembleDebug
```
