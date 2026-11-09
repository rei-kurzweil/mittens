# Twilight moon body: sampled versus procedural

Date: 2026-11-09. Fresh complete run after the user stopped other GPU activity.
The earlier contended run is discarded. [Raw results](twilight-moon-body-ab-2026-11-09.json).

## Result

At the normal 70° vertical FOV, the two versions have essentially the same sky
cost on the available GTX 1080: roughly 0.15 ms at 960×640 and 0.67–0.68 ms at
1920×1920, for both eyes combined. The procedural shader's small apparent lead
also appears with the moon outside the view, so it should not be attributed to
replacing the sampled body. The first high-resolution sampled block has an
outlier; raw repeated-block distributions are retained.

At the deliberately enlarged 12° FOV, procedural coverage produces visibly
smoother outer/inner curves and tips, avoiding the baked body's magnified texel
steps. It costs about 6 µs more at 960×640 and 31 µs more at 1920×1920 (roughly
4% of this isolated sky draw). At normal size the difference is subtle.

| Resolution per eye | View | Sampled median, ms | Procedural median, ms | Procedural minus sampled |
| --- | --- | --- | --- | --- |
| 960×640 | moon | 0.150304 | 0.149632 | -0.67 µs |
| 960×640 | away | 0.149696 | 0.148608 | -1.09 µs |
| 960×640 | closeup | 0.155040 | 0.161056 | +6.02 µs |
| 1920×1920 | moon | 0.680928 | 0.672704 | -8.22 µs |
| 1920×1920 | away | 0.675808 | 0.669152 | -6.66 µs |
| 1920×1920 | closeup | 0.707008 | 0.737920 | +30.91 µs |

The production default remains the sampled body. The compile-time
`MOON_BODY_PROCEDURAL=1` variant is available for comparison; this is a quality
option, not a demonstrated optimization. A procedural default would be reasonable
if enlarged/zoomed views matter, but this run does not measure XR2 performance.

## Controlled variables and interpretation

- GPU: NVIDIA GeForce GTX 1080, Vulkan device API 1.2.0; GPU timestamps with a
  reported 1 ns period, four-sample MSAA, two eyes, no bloom or frame pacing.
- Isolated sky using the real engine mesh/material/upload/render path. Graphics
  timing includes clears and resolve; it excludes scene geometry, CPU authoring,
  shader compilation, readback, live tracking, compositor, and display pacing.
- Both modes use the same 1024×512 RGBA UNORM texture/mips and the same sampler,
  gradient, clouds, moon projection, radius, orientation, tint, and baked glow.
- Sampled mode gets body and glow in `.gb` from one moon-region lookup. Procedural
  mode replaces only the body's coverage with disk-minus-disk math and derivative
  antialiasing. It still samples the baked glow. Thus texture fetch count, texture
  allocation, and sampler count are unchanged; the change trades body resolution
  limits for arithmetic, rather than removing texture bandwidth.
- Four repeated paired blocks per case, alternating AB/BA order. Every block has
  24 warmup and 80 measured stereo frames: 320 samples per mode/case. GPU completion
  is fenced before timestamp retrieval/reuse. Raw samples, block summaries, and
  aggregate means/medians/p95 are in the JSON. This is a static throughput comparison,
  not a temporal shimmer test or whole-workshop frame benchmark.

## Reproduce and inspect

```sh
cargo test --lib profile_twilight_moon_body -- --ignored --nocapture
```

Requires GPU/driver access outside the session sandbox. Optional environment
variables: `TWILIGHT_PROFILE_OUTPUT`, `TWILIGHT_PROFILE_WARMUP`,
`TWILIGHT_PROFILE_FRAMES`, and `TWILIGHT_PROFILE_REPEATS`. Do not use the GPU for
other work during the timing run.

The fixture writes compile-time shader variants under `/tmp`, leaving the shipped
material's default intact. Captures are `/tmp/twilight-body-<moon|closeup>-<width>x<height>-<sampled|procedural>.png`.
The 1920×1920 closeups show the quality difference most clearly. Captures are
read back after timed blocks, outside the measured interval.
