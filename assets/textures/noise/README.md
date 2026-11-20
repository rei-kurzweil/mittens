# Perlin noise

Seamless grayscale Perlin fBm at 128, 256, 512 and 1024 px, in three forms:

| File | Format | Bits/px | Where it loads |
|---|---|---|---|
| `assets/images/noise/perlin-<size>.png` | uploaded as RGBA8 | 32 on GPU | desktop and Android |
| `perlin-<size>.dds` | BC7 UNorm, one mip | 8 | desktop only (Adreno has no BC) |
| `perlin-<size>-astc4x4.astc` | ASTC LDR 4x4, one level | 8 | Android only (GTX 1080 has no ASTC) |
| `perlin-<size>-astc8x8.astc` | ASTC LDR 8x8, one level | 2 | Android only |

Decoded error vs the PNG: BC7 ≤ 3/255; ASTC 4x4 ≤ 2/255; ASTC 8x8 ≤ 3/255 at
1024 and ≤ 14/255 at 128.

Regenerate all of them with:

```sh
scripts/install-texture-tools   # pinned astcenc into tools/texture/ (gitignored)
python3 assets/images/noise/generate-perlin-noise.py [--seed N] [--sizes 128 256 512 1024] [--no-astc]
```

All sizes share the same layout (8 cells across at the base octave); larger
sizes add finer octaves. The script encodes BC7 itself (grayscale fits BC7
mode 6), so compressonator is not required; ASTC uses astcenc.
`assets/convert_images_to_dds --force` would also pick up the PNGs and
overwrite the `.dds` files with compressonator output.
