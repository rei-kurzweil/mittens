# Cloud-only sky baseline and procedural moon cost

Date: 2026-11-09. [Raw GPU samples](twilight-no-moon-baseline-2026-11-09.json).

## Production change

The example now uses fully procedural moon body and glow by default. Both
`MOON_BODY_PROCEDURAL` and `MOON_GLOW_PROCEDURAL` default to 1. The background
wrapper binds only the original `assets/textures/space/sky-cloud-mask.png`;
the packed moon texture remains available for old sampled benchmark variants.
There is one cloud texture lookup and no moon/glow texture lookup in the shipped
shader. No runtime blur, new render targets, or numeric material schema changes.
Moon settings, palette, runtime XR projections, and scene layout remain intact.

## Measured baseline

NVIDIA GeForce GTX 1080, symmetric per-eye **120° horizontal / 110° vertical**
test projection, four-sample MSAA. Vertical FOV is an explicit approximation,
not a captured headset projection. Values below are median GPU graphics durations
in milliseconds for **both eyes combined**:

| Resolution per eye | Gradient + clouds only | Regular procedural moon | 5× procedural moon |
| --- | --- | --- | --- |
| 960×640 | 0.134656 | 0.152320 | 0.160160 |
| 1920×1920 | 0.575840 | 0.675360 | 0.711712 |

At 1920×1920 per eye, the cloud-only sky takes about **0.576 ms**. The regular
procedural moon adds about **0.100 ms**; the five-times moon adds about
**0.136 ms**. At 960×640 the baseline is about 0.135 ms, with additions around
0.018 and 0.026 ms respectively. Costs include clears and resolve, not just
fragment arithmetic. These are isolated sky timings on a desktop GPU, not XR2
or full-scene/compositor frame times.

The cloud-only variant compiles out the entire moon path using `MOON_ENABLED=0`.
It is therefore different from looking away while moon code remains present,
or setting body/glow strengths to zero without eliminating work. The regular
moon still includes projection/setup arithmetic across the sky, while the
crescent/body/glow work is restricted to its forward footprint. The measured
increment encompasses all shader/compiler differences, including register use.

## Method and validation

All three variants use the same original cloud texture/mips, palette, camera,
resolution, sphere, lighting-independent background path, and MSAA. Compiled
SPIR-V has one texture-sample site in every variant. No bloom, scene geometry,
tracking, display pacing, or readback is included in the timing interval.
Six balanced permutations, 24 warmup and 80 measured stereo frames per block,
produce 480 samples per mode at each resolution. There are first high-resolution
outliers and normal clock variation; all blocks and samples are retained.

The scene/material bootstrap and shader-layout checks pass after the default
switch. The real workshop also passes the offscreen stereo capture suite with
bloom off/on, translation, cloud seam/pole views, and foreground occlusion.
That GPU test and the baseline benchmark were run serially to avoid contention.
Captures confirm a moon-free sky in the baseline and the procedural moon in the
example.

```sh
cargo test --lib -- paper_craft twilight_sky_has
cargo test --lib -- --include-ignored --test-threads=1 twilight_sky_stereo profile_twilight_no_moon
```

GPU tests need driver access outside the session sandbox, with competing GPU
work idle. The baseline test uses the same FOV/frame/repeat/output environment
options as the size test. Historical comparison entry points remain available.

Review captures include:

- `/tmp/twilight-baseline-fov110-baseline-1920x1920-no-moon.png`
- `/tmp/twilight-baseline-fov110-baseline-1920x1920-moon1x.png`
- `/tmp/twilight-baseline-fov110-baseline-1920x1920-moon5x.png`
- `/tmp/paper-craft-twilight-moon-off-0.png` (updated production sky)
