# Moon size at fixed 120°H / 110°V: new benchmark cases

Date: 2026-11-09. [Raw measurements](twilight-moon-size-comparison-2026-11-09.json).

## Scope

User requested a regular moon and a moon five times larger in the same wide
camera view, rather than changing the camera FOV to magnify the scene. This run
contains only those two new cases, at the two existing resolutions. It does not
repeat the old 70°/12° views or the away control.

Both cases use **120° horizontal and 110° vertical**, symmetric per eye. The
vertical value is an explicit test approximation following the user's correction
that the advertised 120° is horizontal. These are not captured Focus 3/ALVR
frusta; the real example continues using its runtime-supplied eye projections.
Independent horizontal and vertical projection entries retain the same angles
at both image aspect ratios.

Regular moon radius is 0.055 radians (about 6.30° outer-disk angular diameter).
Five-times moon radius is 0.275 radians (about 31.51° diameter). The entire moon,
including the glow, scales in moon coordinates. Because perspective uses tangent
projection, the larger disk is about 5.12× the projected size, rather than exactly
5× in pixels. Its bounding disk height is about 19.8% of the image versus 3.86%
for the regular moon; the crescent occupies only part of that disk.

## Result

GPU: NVIDIA GeForce GTX 1080. Median GPU graphics duration in milliseconds for
**both eyes combined**, with four-sample MSAA:

| Resolution per eye | Moon size | Sampled body/glow | Procedural body, baked glow | Procedural body/glow |
| --- | --- | --- | --- | --- |
| 960×640 | 1× | 0.154624 | 0.153408 | 0.152480 |
| 960×640 | 5× | 0.157088 | 0.158624 | 0.160224 |
| 1920×1920 | 1× | 0.688704 | 0.683808 | 0.678848 |
| 1920×1920 | 5× | 0.703296 | 0.710048 | 0.713088 |

At 1920×1920 per eye, the five-times moon costs about 0.703 ms sampled,
0.710 ms with a procedural body and baked glow, and 0.713 ms fully procedural.
The fully procedural difference from sampled is about **0.010 ms (1.4%)** in this
case. The increase from regular to five-times size is about 0.015, 0.026, and
0.034 ms respectively. These are much smaller changes than the prior camera-zoom
stress case, where the moon covered a much larger share of the image.

At regular size the modes are close; fully procedural has a small apparent lead,
as it also did in earlier away controls. This run provides no strong reason to
switch for performance alone. Block distributions show clock/timing variation,
including first high-resolution outliers; raw samples are retained. No XR2 or
whole-workshop performance claim follows from these desktop sky-only timings.

The captures confirm the camera/cloud composition stays fixed while the moon
and glow grow. Procedural body edges stay smooth; analytic glow has a slightly
different falloff near the tips/cutout. The shipped default stays sampled.

## Method and reproduction

Same engine sky path and GPU timestamp harness as the earlier comparison:
unchanged clouds, textures, palette, moon placement/orientation, MSAA, and
resolution within each size pair. No bloom, scene geometry, tracking, compositor,
or display pacing. Clears and resolve are included. Compilation, warmup and
readback are excluded. One packed texture/sampler in every mode; compiled shader
texture-sample sites are 2, 2, 1. Fully procedural samples only the clouds.

Six balanced permutations of three variants per case, 24 warmup frames and
80 measured stereo frames per block: 480 samples per variant/case. No repeated
pipeline creation in the measured interval. Material radius updates and projection
changes happen before warmup, and snapshots remain stable during timing.

```sh
cargo test --lib profile_twilight_moon_sizes -- --ignored --nocapture
```

Requires compatible GPU access and no competing GPU work. Defaults are 120°H,
110°V. Optional `TWILIGHT_PROFILE_HORIZONTAL_FOV` and `TWILIGHT_PROFILE_FOV`
(vertical) override those values. Existing warmup/frame/repeat/output environment
variables remain supported. Defaults for the older benchmark entry points retain
the old FOV setup so those cases remain reproducible.

New captures have a separate prefix:

- `/tmp/twilight-sizes-fov110-moon1x-1920x1920-sampled.png`
- `/tmp/twilight-sizes-fov110-moon5x-1920x1920-sampled.png`
- `/tmp/twilight-sizes-fov110-moon5x-1920x1920-procedural.png`
- `/tmp/twilight-sizes-fov110-moon5x-1920x1920-analytic-glow.png`

Matching 960×640 captures are also emitted. `fov110` in the filename denotes
vertical FOV; explicit horizontal and vertical angles are recorded in the JSON.
