#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/target}"
# shaderc-sys 0.8.3 needs CMake 3 on this checkout. Prefer the installed SDK
# copy when present, while honoring an explicitly selected host CMake.
sdk_cmake="${ANDROID_HOME:-$HOME/Android/Sdk}/cmake/3.30.3/bin/cmake"
if [[ -z "${CMAKE:-}" && -x "$sdk_cmake" ]]; then
    export CMAKE="$sdk_cmake"
fi
cd -- "$repo_root"
exec cargo run --release --locked --manifest-path examples/xr-renderer-compare/Cargo.toml -- "$@"
