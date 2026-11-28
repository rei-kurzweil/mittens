#!/usr/bin/env bash
# Push the MMS project tree to the headset for the launcher build.
#
#   android/packaging/sync-project.sh [--launch]
#
# Mirrors examples/, assets/ and android/{examples,shaders}/ into the app's
# external files folder, where the launcher uses it as its working directory.
# Git-ignored local models (assets/models/local/) are included when present on
# this machine. Desktop-only files are skipped. Only changed files are pushed
# after the first run. --launch restarts Mittens Host afterwards.
set -euo pipefail

package=org.mittens.engine.dev
remote="/sdcard/Android/data/$package/files/project"
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
stage="$repo_root/target/android-project"

command -v adb >/dev/null || { echo "adb not found on PATH" >&2; exit 1; }
command -v rsync >/dev/null || { echo "rsync not found on PATH" >&2; exit 1; }
devices=$(adb devices | awk 'NR > 1 && $2 == "device"' | wc -l)
if [[ "$devices" -ne 1 ]]; then
    echo "expected exactly one authorized adb device, found $devices" >&2
    exit 1
fi

# Stage with the repository layout so `adb push --sync` compares one tree.
mkdir -p "$stage/android"
rsync_args=(-a --delete --prune-empty-dirs
    --exclude '*.dds' --exclude '*.blend' --exclude '*.blend1' --exclude '*.rs'
    --exclude 'models/tmp/' --exclude 'models/blender/')
rsync "${rsync_args[@]}" "$repo_root/examples/" "$stage/examples/"
rsync "${rsync_args[@]}" "$repo_root/assets/" "$stage/assets/"
rsync "${rsync_args[@]}" "$repo_root/android/examples/" "$stage/android/examples/"
rsync "${rsync_args[@]}" "$repo_root/android/shaders/" "$stage/android/shaders/"

for model in assets/models/mitten.glb \
    assets/models/local/third_party/TDA_Hatsune_Miku/tda_miku_with_bones.glb; do
    if [[ -f "$stage/$model" ]]; then echo "model: $model"; else echo "model missing (skipped): $model"; fi
done

adb shell mkdir -p "$remote"
# --sync pushes only files that are newer than the headset's copy. Files
# deleted locally are not removed from the headset.
adb push --sync "$stage/." "$remote/"
echo "project synced to $remote"

if [[ "${1:-}" == "--launch" ]]; then
    adb shell am force-stop "$package"
    adb shell am start -n "$package/android.app.NativeActivity"
fi
