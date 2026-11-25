#!/usr/bin/env bash
# Builds the Rust protocol library for Android (arm64) and generates its Kotlin wrapper.
# Needs: Rust, the aarch64-linux-android target, cargo-ndk, and ANDROID_NDK_HOME (the Android CI job sets all of this up).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/android/core/link/src/generated"
rm -rf "$out"
mkdir -p "$out/jniLibs" "$out/kotlin"

cd "$root"
cargo ndk -t arm64-v8a -o "$out/jniLibs" build --release -p vex-link-ffi

# Bindings are generated from a build of the same source for this machine. It must be a DEBUG build: the release profile
# strips symbols, and UniFFI reads its interface description from those symbols.
cargo build -p vex-link-ffi -p uniffi-bindgen
lib="target/debug/libvex_link_ffi.so"
[ -f "$lib" ] || lib="target/debug/libvex_link_ffi.dylib"
target/debug/uniffi-bindgen generate --library "$lib" --crate vex_link_ffi --language kotlin --out-dir "$out/kotlin" --no-format
[ -n "$(find "$out/kotlin" -name '*.kt')" ] || { echo "no Kotlin bindings were generated" >&2; exit 1; }
echo "native library and Kotlin bindings written to $out"
find "$out" -type f | sort
