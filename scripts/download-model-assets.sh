#!/usr/bin/env sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
models_dir="$repo_root/assets/models"
base_url="${MITTENS_MODEL_ASSET_BASE_URL:-https://raw.githubusercontent.com/rei-kurzweil/cat-engine/main}"

mkdir -p "$models_dir"

download_one() {
    rel="$1"
    dest="$repo_root/$rel"
    url="$base_url/$rel"

    if [ -s "$dest" ]; then
        echo "[download-model-assets] exists: $rel"
        return 0
    fi

    tmp="$dest.tmp.$$"
    echo "[download-model-assets] downloading $url"

    if command -v curl >/dev/null 2>&1; then
        curl --fail --location --show-error --output "$tmp" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -O "$tmp" "$url"
    else
        echo "[download-model-assets] error: install curl or wget, or place $rel manually" >&2
        rm -f "$tmp"
        return 127
    fi

    if [ ! -s "$tmp" ]; then
        echo "[download-model-assets] error: downloaded empty file for $rel" >&2
        rm -f "$tmp"
        return 1
    fi

    mv "$tmp" "$dest"
    echo "[download-model-assets] wrote: $rel"
}

if [ "$#" -eq 0 ]; then
    set -- \
        assets/models/bisket.5.5.glb \
        assets/models/bisket.8.0.glb \
        assets/models/bisket.11.0.glb \
        assets/models/color-cat.2.glb \
        assets/models/pc-rei.hoodie.glb
fi

for rel in "$@"; do
    case "$rel" in
        assets/models/bisket.5.5.glb|\
        assets/models/bisket.8.0.glb|\
        assets/models/bisket.11.0.glb|\
        assets/models/color-cat.2.glb|\
        assets/models/pc-rei.hoodie.glb)
            download_one "$rel"
            ;;
        *)
            echo "[download-model-assets] error: unsupported model path: $rel" >&2
            exit 2
            ;;
    esac
done
