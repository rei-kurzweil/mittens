#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)

: "${ANDROID_HOME:?Set ANDROID_HOME to your Android SDK directory}"
: "${ANDROID_NDK_HOME:=$ANDROID_HOME/ndk/28.1.13356709}"
: "${CMAKE:=$ANDROID_HOME/cmake/3.30.3/bin/cmake}"
export ANDROID_NDK_HOME CMAKE
# shaderc's CMake build also needs this name to locate the same NDK.
export ANDROID_NDK="$ANDROID_NDK_HOME"

if [[ ! -d "$ANDROID_NDK_HOME" ]]; then
    printf 'Android NDK not found: %s\nSet ANDROID_NDK_HOME to the installed NDK.\n' "$ANDROID_NDK_HOME" >&2
    exit 1
fi
if ! command -v "$CMAKE" >/dev/null; then
    printf 'CMake not found: %s\nSet CMAKE to a CMake 3.x executable for shaderc-sys 0.8.3.\n' "$CMAKE" >&2
    exit 1
fi

cd -- "$repo_root"
exec cargo apk build --manifest-path android/Cargo.toml -p mitten-droid --lib "$@"
