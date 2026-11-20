#!/usr/bin/env python3
"""Seamless grayscale Perlin fBm noise textures, as PNG and BC7 DDS.

    python3 assets/images/noise/generate-perlin-noise.py [--seed N] [--sizes 128 256 ...]

Writes assets/images/noise/perlin-<size>.png,
assets/textures/noise/perlin-<size>.dds (BC7, desktop) and
assets/textures/noise/perlin-<size>-astc<WxH>.astc (ASTC LDR, Android/Adreno).
Requires numpy and Pillow; ASTC needs astcenc from scripts/install-texture-tools
(or on PATH). Pass --no-astc to skip it.

Every size has the same layout: 8 gradient cells across at the base octave,
with octaves added until a cell is 2 px, so larger textures add fine detail
rather than more blobs. Each octave is periodic, so the texture tiles exactly.

The DDS is BC7 (DX10 header, linear UNorm, one mip) because that is the only
compressed format the desktop texture loader accepts. It is desktop-only:
Adreno/XR2 has no BC support and the Android renderer rejects BC7; use the
PNGs there. compressonator is not
needed: grayscale fits BC7 mode 6 directly (one subset, 16 index levels per
4x4 block). Mode 6 shares one p-bit between colour and alpha; opaque alpha
forces odd endpoints, so 0 encodes as 1 (at most 1/255 error).
"""
import argparse
import os
import shutil
import struct
import subprocess
from pathlib import Path

import numpy as np
from PIL import Image

REPO = Path(__file__).resolve().parents[3]
PNG_DIR = REPO / "assets/images/noise"
DDS_DIR = REPO / "assets/textures/noise"
BASE_CELLS = 8
ASTC_BLOCKS = ["4x4", "8x8"]
PERSISTENCE = 0.5


def fade(t):
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0)


def perlin(size, cells, rng):
    """One octave of gradient noise, periodic over `cells` cells per side."""
    angles = rng.uniform(0.0, 2.0 * np.pi, (cells, cells))
    grads = np.stack([np.cos(angles), np.sin(angles)], axis=-1)
    coords = (np.arange(size) + 0.5) * cells / size
    x, y = np.meshgrid(coords, coords)  # x varies along columns
    x0 = np.floor(x).astype(int)
    y0 = np.floor(y).astype(int)
    fx = x - x0
    fy = y - y0

    def corner(dx, dy):
        g = grads[(y0 + dy) % cells, (x0 + dx) % cells]
        return g[..., 0] * (fx - dx) + g[..., 1] * (fy - dy)

    u = fade(fx)
    v = fade(fy)
    top = corner(0, 0) * (1.0 - u) + corner(1, 0) * u
    bottom = corner(0, 1) * (1.0 - u) + corner(1, 1) * u
    return top * (1.0 - v) + bottom * v


def fbm(size, seed):
    rng = np.random.default_rng(seed)
    total = np.zeros((size, size))
    amplitude = 1.0
    norm = 0.0
    cells = BASE_CELLS
    while size // cells >= 2:
        total += perlin(size, cells, rng) * amplitude
        norm += amplitude
        amplitude *= PERSISTENCE
        cells *= 2
    total /= norm
    # Stretch to the full range so every size uses all 256 levels.
    lo, hi = total.min(), total.max()
    return np.round((total - lo) / (hi - lo) * 255.0).astype(np.uint8)


WEIGHTS = np.array([0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64])


def bc7_mode6_gray(gray):
    """Encode an (H, W) uint8 image (H, W multiples of 4) as BC7 mode 6 blocks."""
    h, w = gray.shape
    blocks = gray.reshape(h // 4, 4, w // 4, 4).transpose(0, 2, 1, 3).reshape(-1, 16).astype(int)
    # Endpoints must be odd (p-bit 1 keeps alpha at 255).
    lo = blocks.min(axis=1)
    hi = blocks.max(axis=1)
    lo = np.where(lo % 2 == 1, lo, np.maximum(lo - 1, 1))
    hi = np.where(hi % 2 == 1, hi, hi + 1)
    palette = ((64 - WEIGHTS)[None, :] * lo[:, None] + WEIGHTS[None, :] * hi[:, None] + 32) >> 6
    indices = np.abs(blocks[:, :, None] - palette[:, None, :]).argmin(axis=2)
    # Anchor (pixel 0) index must have its top bit clear: swap endpoints.
    swap = indices[:, 0] >= 8
    lo, hi = np.where(swap, hi, lo), np.where(swap, lo, hi)
    indices = np.where(swap[:, None], 15 - indices, indices)

    out = bytearray()
    for e0, e1, idx in zip(lo >> 1, hi >> 1, indices):
        bits = 1 << 6  # mode 6
        pos = 7
        for value in (e0, e1, e0, e1, e0, e1, 127, 127):  # R0 R1 G0 G1 B0 B1 A0 A1
            bits |= int(value) << pos
            pos += 7
        bits |= 1 << pos  # P0
        bits |= 1 << (pos + 1)  # P1
        pos += 2
        bits |= int(idx[0]) << pos
        pos += 3
        for value in idx[1:]:
            bits |= int(value) << pos
            pos += 4
        out += bits.to_bytes(16, "little")
    return bytes(out)


def write_dds(path, width, height, data):
    header = struct.pack(
        "<4s7I44x8I5I",
        b"DDS ",
        124,
        0x1 | 0x2 | 0x4 | 0x1000 | 0x80000,  # caps, height, width, pixelformat, linearsize
        height,
        width,
        len(data),
        0,  # depth
        1,  # mip count
        32,  # pixel format size
        0x4,  # DDPF_FOURCC
        int.from_bytes(b"DX10", "little"),
        0, 0, 0, 0, 0,
        0x1000,  # DDSCAPS_TEXTURE
        0, 0, 0, 0,
    )
    dx10 = struct.pack("<5I", 98, 3, 0, 1, 0)  # BC7_UNORM, TEXTURE2D, array size 1
    path.write_bytes(header + dx10 + data)


def astcenc():
    local = REPO / "tools/texture/bin/astcenc"
    if os.access(local, os.X_OK):
        return str(local)
    found = shutil.which("astcenc")
    if found:
        return found
    raise SystemExit("astcenc not found: run scripts/install-texture-tools (or pass --no-astc)")


def write_astc(encoder, png, path, block):
    # -cl: linear LDR (data texture, not sRGB). -thorough is cheap at these sizes.
    subprocess.run(
        [encoder, "-cl", str(png), str(path), block, "-thorough", "-silent"],
        check=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--sizes", type=int, nargs="+", default=[128, 256, 512, 1024])
    parser.add_argument("--no-astc", action="store_true")
    args = parser.parse_args()
    encoder = None if args.no_astc else astcenc()
    PNG_DIR.mkdir(parents=True, exist_ok=True)
    DDS_DIR.mkdir(parents=True, exist_ok=True)
    for size in args.sizes:
        if size % 4 or size < BASE_CELLS * 2:
            raise SystemExit(f"size {size} must be a multiple of 4 and at least {BASE_CELLS * 2}")
        gray = fbm(size, args.seed)
        png = PNG_DIR / f"perlin-{size}.png"
        dds = DDS_DIR / f"perlin-{size}.dds"
        Image.fromarray(gray, "L").save(png, optimize=True)
        write_dds(dds, size, size, bc7_mode6_gray(gray))
        print(f"wrote {png.relative_to(REPO)} and {dds.relative_to(REPO)}")
        if encoder:
            for block in ASTC_BLOCKS:
                astc = DDS_DIR / f"perlin-{size}-astc{block}.astc"
                write_astc(encoder, png, astc, block)
                print(f"wrote {astc.relative_to(REPO)}")


if __name__ == "__main__":
    main()
