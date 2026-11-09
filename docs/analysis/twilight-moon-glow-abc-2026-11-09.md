# Twilight moon and glow: three-way GPU comparison

Date: 2026-11-09. [Raw timings and block summaries](twilight-moon-glow-abc-2026-11-09.json).
This records the final run with the corrected benchmark-camera Y orientation;
the preliminary run is discarded. The actual example camera was unaffected.

## Result and recommendation

On the GTX 1080, analytic glow is not a useful speed optimization for this sky.
The normal-size versions have similar cost. Although fully procedural moon/glow
has a small apparent lead, that lead also occurs when the moon is out of view;
it cannot be credited specifically to avoiding the moon's texture lookup.

At 1920×1920 per eye in the deliberately enlarged 12° view, the fully procedural
version costs about 0.068 ms more for both eyes, roughly 10% of the isolated sky
cost. It costs about 0.037 ms more than procedural body plus baked glow. This
suggests the extra arithmetic outweighs the removed cached lookup in this case.
This is a desktop GPU measurement, not an XR2 result or whole-scene timing.

The analytic glow looks similar at normal size and stays smooth when enlarged,
with a slightly different halo shape/falloff near the tips and cutout. The
procedural crisp body plus baked glow is the useful middle option if smooth
zoomed edges matter. Keep the fully sampled production default until choosing
that quality tradeoff; this experiment does not change the example's default.

| Resolution per eye | View | Sampled body + glow, ms | Procedural body + baked glow, ms | Procedural body + analytic glow, ms |
| --- | --- | --- | --- | --- |
| 960×640 | moon | 0.151744 | 0.150912 | 0.150176 |
| 960×640 | away | 0.151008 | 0.150048 | 0.148768 |
| 960×640 | closeup | 0.150208 | 0.155456 | 0.161216 |
| 1920×1920 | moon | 0.681184 | 0.676352 | 0.671648 |
| 1920×1920 | away | 0.678176 | 0.671264 | 0.663456 |
| 1920×1920 | closeup | 0.704288 | 0.735424 | 0.772160 |

All values are aggregate medians of GPU graphics timestamps for **both eyes**.
Read the raw block distributions when assessing small differences: some first
high-resolution blocks have outliers, and there is normal clock/timing drift.
The large-view cost ordering repeats across the balanced blocks.

## What changed

- All variants retain the same gradient, clouds, moon projection/size/orientation,
  tint, intensity, texture allocation, sampler, and four-sample MSAA.
- Fully sampled: cloud lookup plus conditional moon-region lookup returning body
  and baked glow together.
- Procedural body: same lookups, with antialiased disk-minus-disk body coverage.
- Analytic glow: reuses the two circle distances for softened coverage at scales
  0.06, 0.18, and 0.35, blended with weights 0.35, 0.45, 0.20. Each circle rolls
  off via smoothstep over ±2 scale units; multiplying coverages follows the
  crescent, including the inner curve and tips. It approximates the baked blur;
  it is not an exact Gaussian convolution. No runtime blur or extra targets.
- The compiled SPIR-V is checked: texture-sample instruction sites are **2, 2, 1**.
  Thus the third variant really removes the conditional moon/glow lookup. It
  still samples the cloud channel of the same packed RGBA texture. Memory size
  and sampler count are unchanged, isolating lookup/math differences.

## Method

NVIDIA GeForce GTX 1080, Vulkan device API 1.2.0, reported timestamp period 1 ns.
Isolated sky through the actual engine material, mesh, upload, and stereo draw
path. Graphics duration includes clears/resolve. Shader compilation, warmup,
CPU scene work, image readback, bloom, scene geometry, tracking, compositor,
and display pacing are excluded.

Two resolutions, normal moon view (70° vertical FOV), away view (70°), and
magnified view (12°). Six repeated groups per case cover all six permutations
of the three variants. Each block warms for 24 frames and measures 80 stereo
frames: 480 samples per mode/case. Every frame is fenced before retrieving and
reusing its timestamp queries. No pipeline rebuild occurs inside timed blocks.
Normal/closeup images are saved after timing; this is not a motion/shimmer test.

## Reproduce and inspect

```sh
cargo test --lib profile_twilight_moon_glow -- --ignored --nocapture
```

Needs compatible GPU access outside the session sandbox, with other GPU work
idle. Optional `TWILIGHT_PROFILE_OUTPUT`, `TWILIGHT_PROFILE_WARMUP`,
`TWILIGHT_PROFILE_FRAMES`, and `TWILIGHT_PROFILE_REPEATS` configure output/run size.
The earlier two-mode `profile_twilight_moon_body` entry point remains available.

Shader switches default to zero. For this experiment, temporary shader copies
set `MOON_BODY_PROCEDURAL` and `MOON_GLOW_PROCEDURAL` at compile time; no runtime
mode branch is introduced into the shipped sky. Turning on both removes the
moon lookup. The shared mask asset and numeric material schema remain intact.

Review files:

- `/tmp/twilight-glow-moon-1920x1920-sampled.png`
- `/tmp/twilight-glow-moon-1920x1920-procedural.png`
- `/tmp/twilight-glow-moon-1920x1920-analytic-glow.png`
- `/tmp/twilight-glow-closeup-1920x1920-sampled.png`
- `/tmp/twilight-glow-closeup-1920x1920-procedural.png`
- `/tmp/twilight-glow-closeup-1920x1920-analytic-glow.png`

The fixture also emits corresponding 960×640 captures. The three variant names
are sampled, procedural (body only), and analytic-glow (body plus glow).
