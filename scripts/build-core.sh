#!/usr/bin/env bash
# Builds the Rust core for Android and generates the Kotlin bindings.
# Usage: scripts/build-core.sh <jniLibs-out-dir> <kotlin-out-dir> <host-lib-out-dir> [release|debug]
set -euo pipefail
JNI_OUT="$1"; KT_OUT="$2"; HOST_OUT="$3"; PROFILE="${4:-release}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/core"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
: "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME must point to the Android NDK}"
export ANDROID_NDK="$ANDROID_NDK_HOME" # read by llama-cpp-sys-2

FLAG=""; [ "$PROFILE" = release ] && FLAG="--release"
# Device ABIs: arm64 phones (the common case today), armeabi-v7a (older/
# budget 32-bit-only phones and tablets — confirmed a real crash-on-launch
# cause 2026-11-26: a Samsung Galaxy Tab A6 has no 64-bit ABI at all, so an
# APK built without this immediately fails to load the native library),
# and x86_64 (emulator).
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -P 26 -o "$JNI_OUT" build $FLAG -p ara-ffi
# Host library: used by JVM unit tests through JNA, and as bindgen input.
cargo build $FLAG -p ara-ffi
mkdir -p "$HOST_OUT"
cp "target/$PROFILE/libara_ffi.so" "$HOST_OUT/"
cargo run -q $FLAG -p ara-ffi --features bindgen --bin uniffi-bindgen -- \
  generate --library "target/$PROFILE/libara_ffi.so" --language kotlin --no-format --out-dir "$KT_OUT"
