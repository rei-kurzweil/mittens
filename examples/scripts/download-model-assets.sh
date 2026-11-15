#!/usr/bin/env sh
# Fetch the .glb models omitted from the crates.io package.
#
# Models come from the git tag matching this package's version (v<version>).
# Set MITTENS_MODEL_ASSET_REF to use another tag/branch/commit, or
# MITTENS_MODEL_ASSET_BASE_URL to replace the whole download base.
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$repo_root/Cargo.toml" | head -n 1)"
ref="${MITTENS_MODEL_ASSET_REF:-v$version}"
base_url="${MITTENS_MODEL_ASSET_BASE_URL:-https://raw.githubusercontent.com/rei-kurzweil/mittens/$ref}"

# Every .glb the repository tracks; keep in sync with `git ls-files '*.glb'`.
models="
assets/components/characters/lumae/lumae.glb
assets/models/STA_launcher.glb
assets/models/STA_rocket_bullet.glb
assets/models/bisket.glb
assets/models/broom.glb
assets/models/broomstick.glb
assets/models/capsule_stick_figure.glb
assets/models/car.glb
assets/models/color-cat.2.glb
assets/models/estradiol-tablet.glb
assets/models/kawaii_wings.glb
assets/models/kawaii_wings_line_art.glb
assets/models/mitten.glb
assets/models/mitten_pickup.glb
assets/models/pc-rei.hoodie.glb
assets/models/sakura-machine-pistol.glb
assets/models/sativa-preroll.glb
assets/models/smoke-rounded-box.glb
assets/models/straw.glb
assets/models/sub-machinegun.glb
assets/models/suspension-spring.glb
"

download_one() {
    rel="$1"
    dest="$repo_root/$rel"
    url="$base_url/$rel"

    if [ -s "$dest" ]; then
        echo "[download-model-assets] exists: $rel"
        return 0
    fi

    mkdir -p "$(dirname -- "$dest")"
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
    # Word-split the newline-separated list; paths contain no spaces.
    # shellcheck disable=SC2086
    set -- $models
fi

for rel in "$@"; do
    case "
$models
" in
        *"
$rel
"*)
            download_one "$rel"
            ;;
        *)
            echo "[download-model-assets] error: unsupported model path: $rel" >&2
            exit 2
            ;;
    esac
done
