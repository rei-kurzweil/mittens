# Kawaii twilight sky and baked crescent glow

Status: S1–S2 implemented, 2026-11-09. Shader, generator, scene, and offscreen stereo checks passed; headset art/comfort acceptance pending.

Current default: fully procedural moon body and glow, with only the original
cloud texture sampled. Earlier sections describe the initial baked implementation
and benchmark history; the latest decision/baseline is recorded at the end.

## Reuse and requested difference

The current [ship sky prefab](../../../assets/components/backgrounds/ship_space_sky.mms)
already uses [`sky-cloud-mask.png`](../../../assets/textures/space/sky-cloud-mask.png)
and a crescent in [`ship-space-sky.frag`](../../../assets/shaders/ship-space-sky.frag).
Its visible halo exists with bloom off, but is formed from radial exponentials
around the moon center, rather than following the crescent. The prefab also uses
extracted emissive output and adds gold stars. Those are existing ship choices,
not requirements to copy into the new workshop sky.

The new asset should have a pink lower sky, lilac transition, and slightly bluer
upper sky; tintable masked clouds; and a crescent whose built-in glow follows
its curved illuminated shape, including the inner curve and tips. Keep the sky
rotation-only under camera translation and correctly occluded by foreground objects.

## S1 — Reusable twilight material and sky wrapper

- [x] Add `assets/components/materials/kawaii-twilight-sky.mms` exporting a
  reusable material, with a small background wrapper if the scene needs one.
- [x] Reuse the existing monochrome cloud texture and its spherical seam/pole
  handling; preserve independent cloud tint/strength and gradient controls.
- [x] Start with art-tunable colors around pink `[1.0, 0.67, 0.78]`, lilac
  `[0.67, 0.53, 0.82]`, and upper lilac `[0.60, 0.48, 0.78]`. These are
  provisional authoring values; verify color-space interpretation and screenshots.
- [x] Expose moon direction, angular size, orientation, tint, and glow strength.
  Do not carry over the old sun or add stars by default unless the composition needs them.
- [x] Integrate the background into `paper-craft.mms` while keeping suitable
  independent scene lighting; a visible bright sky is not itself a studio light.

Acceptance: both-eye views from roof, street, and stairwell show the intended
gradient/clouds; no panorama seam, pole artifact, camera-translation parallax, or
sky picking target. Original Sakura and ship sky behavior stays intact.

## S2 — Crescent-shaped glow, independent of bloom

Implemented asset approach: an offline Rust generator following the
existing [`generate-sky-cloud-texture`](../../../examples/generate-sky-cloud-texture.rs)
example. [`generate-twilight-moon.rs`](../../../examples/generate-twilight-moon.rs)
outputs reproducible masks and a transparent moon preview under
[`assets/textures/sky/`](../../../assets/textures/sky/README.md).

- [x] Rasterize an antialiased crescent at higher resolution and downsample.
  Its dark cutout removes only moon-body radiance; it must not paint a black
  disk over the underlying sky/cloud color.
- [x] Bake a soft multi-scale glow from the actual crescent coverage, not a
  circular disk. Use sufficient transparent padding so tips and outer halo do
  not clip, and clamp sampling at atlas edges.
- [x] Choose/document the texture contract. A transparent RGBA preview can bake
  body plus glow for inspection. For runtime, separate body/glow mask channels
  may give better control of brightness/tint and avoid low-alpha fringe problems.
- [x] Composite body/glow radiance in the sky shader, preferably inside the
  existing sky draw. Texture transparency does not require a separate transparent
  scene quad, blending pass, or runtime blur target. Check available sampler bindings.
- [x] Define straight/premultiplied alpha and linear/sRGB handling explicitly;
  verify downsampling, filtering, and black/colored fringe behavior.
- [x] Keep glow present in the normal color output when bloom is disabled.
  If bloom is also enabled for the scene, tune extraction to avoid an accidental
  second oversized halo. Bloom contribution may be optional; baked glow is mandatory.

A texture can replace runtime glow evaluation with sampling, but is not
automatically faster than a small analytic crescent-distance function. Compare
image quality, sky GPU cost, texture bandwidth, and apparent resolution at the
chosen angular size. Neither candidate needs a runtime blur effect. Prefer the
simpler adequate result after that comparison; no new render pass is authorized.

Acceptance: bloom-off/on captures, crescent close-up at target resolution, dark
cutout showing sky behind it, readable inner curve/tips, no rectangular sprite
boundary, foreground building/roof occlusion, and stable appearance in both XR
eyes. Generator checks should cover deterministic output, nonempty body/glow,
transparent padded borders, and sensible mask ranges. Record actual performance
before claiming the texture path is faster.

## Implementation notes and validation (2026-11-09)

The new material and background wrapper are integrated into `paper-craft.mms`.
They use a three-stop pink/lilac gradient, tintable clouds, and independently
controlled moon direction, angular radius, rotation, color, brightness, and glow.
The sky sphere radius is 80 m to fit the workshop CXR's existing 100 m far plane.
Surface lighting remains independent of sky color.

The current material ABI allows one base sampler, so the generator packs the
unmodified cloud mask into R, supersampled moon coverage into G, and a three-scale
Gaussian blur of that crescent into B, with opaque alpha. All runtime channels
are linear UNORM data. Moon-local coordinates sample G/B independently of the
cloud panorama coordinates. There is one sky draw and no extra blur pass/target.
A separate square PNG provides straight-alpha tinted moon artwork for inspection.
The shader adds body/glow to the underlying sky; it never overlays a black disk.

Plain `BG` keeps this radiance out of bloom extraction. With bloom enabled for
other scene content, the sky itself retains the same baked glow. The longitude
sampling uses wrapped gradients: visual review caught an implicit-LOD seam,
which was fixed and covered by a pixel-difference check.

Checks:

- Generator test: deterministic output, retained cloud channel, antialiased body,
  padded transparent border, empty body cutout, and asymmetric crescent-following glow.
- Scene/material test: default and overridden values, shared typed definition,
  reflected 128-byte uniform block, supported descriptors, texture binding, and
  non-raycastable background without extraction.
- Paper-craft scene/collision regression and existing Sakura scene bootstrap.
- Offscreen Vulkan: both eyes for moon bloom-off/on, translated sky, roof, street,
  stairs, pole, longitude seam, and foreground-occlusion views. Compares sky color
  across bloom and translation; verifies the visible crescent without bloom.
  Review images use `/tmp/paper-craft-twilight-<view>-<eye>.png`.

Commands:

```sh
cargo run --release --example generate-twilight-moon
cargo test --example generate-twilight-moon
cargo test --lib -- paper_craft twilight_sky_has sakura_scene_has
cargo test --lib twilight_sky_stereo -- --ignored --nocapture
cargo run --release -- load examples/paper-craft.mms
```

The Vulkan capture needs access to a compatible GPU driver (outside this
session's sandbox). Existing editor-settings adapter warnings occur during full
scene capture; the sky shaders compile and the captures complete. No renderer
production code changed. No performance advantage over an analytic crescent
has been measured or claimed; comparative GPU timing remains optional follow-up.

Headset follow-up:

- [ ] Inspect the sky from roof and street, including the moon tips/inner curve.
- [ ] Toggle scene bloom and confirm the moon's built-in glow remains.
- [ ] Look overhead/across the panorama seam and move around nearby buildings.
- [ ] Tune the exposed palette, moon size/orientation, or glow if desired.

Palette refinement: the upper stop is now lighter lilac `[0.60, 0.48, 0.78]`
at direction Y `1.0`, replacing the darker blue stop at `0.90`. The transition
therefore stays brighter and reaches its final color closer to the zenith.

The existing implementation already uses one packed texture and one sampler,
with cloud and moon/glow in separate channels. It performs one cloud lookup,
plus a moon lookup only inside the moon/glow footprint. Combining them into a
single panorama lookup is possible, but would fix the moon's position/size and
sacrifice its independently allocated resolution unless the panorama grew.
Retain the packed-channel version for now; no measured bottleneck justifies
that tradeoff yet.


## Moon-body A/B (2026-11-09)

Compared the sampled crisp body against a procedural antialiased crescent while
retaining the same baked glow. [Benchmark and quality report](../../analysis/twilight-moon-body-ab-2026-11-09.md)
uses only the fresh run requested after other GPU activity stopped. Normal-size
cost is essentially unchanged on the GTX 1080; enlarged procedural edges are
smoother, with roughly 31 µs extra stereo sky cost at 1920×1920 per eye. Both
versions still make the same texture lookups. The sampled production default
remains in place; `MOON_BODY_PROCEDURAL=1` selects the experimental quality variant.


## Analytic glow comparison (2026-11-09)

The [three-way benchmark](../../analysis/twilight-moon-glow-abc-2026-11-09.md)
compares sampled body/glow, procedural body with baked glow, and procedural
body/glow. The third mode uses one cloud lookup and no moon lookup, verified
from the compiled shader. Normal-size GPU cost is similar on the GTX 1080;
fully procedural costs about 0.068 ms extra stereo GPU time in the enlarged
1920×1920-per-eye case. Its softened-circle glow is an artistic approximation
of the bake, with slightly different tips/falloff. Defaults remain sampled;
`MOON_BODY_PROCEDURAL=1` and `MOON_GLOW_PROCEDURAL=1` select the experimental
fully procedural mode. No runtime blur pass or production renderer changes.
The final benchmark run also fixes the test camera's vertical orientation.


## Wide-view moon-size comparison (2026-11-09)

The [new size benchmark](../../analysis/twilight-moon-size-comparison-2026-11-09.md)
keeps a 120°H/110°V test projection fixed and changes only moon/glow angular size
from 1× to 5×. It runs only those two new cases; 110°V is an approximation,
not a captured headset value. At 1920×1920 per eye the 5× moon takes roughly
0.703 ms sampled, 0.710 ms with procedural body/baked glow, and 0.713 ms fully
procedural for both eyes on the GTX 1080. The difference is small; defaults stay
sampled. The fixture `profile_twilight_moon_sizes` retains old benchmark entries
and records both projection axes and actual moon radius in the raw report.


## Procedural default and cloud-only baseline (2026-11-09)

User chose fully procedural moon/glow. Defaults are now analytic and the scene
binds the original monochrome cloud texture. Sampled assets/compile switches
remain for reproduction. The [cloud-only baseline report](../../analysis/twilight-no-moon-baseline-2026-11-09.md)
compares compiled-out moon code against regular/5× procedural moons in the same
120°H/110°V view. At 1920×1920 per eye, both eyes take about 0.576 ms without a
moon, 0.675 ms with the regular moon, and 0.712 ms at 5× size on the GTX 1080.
Scene/layout and actual-workshop stereo checks pass, including bloom independence.
