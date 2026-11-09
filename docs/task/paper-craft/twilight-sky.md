# Kawaii twilight sky and baked crescent glow

Status: planned. Depends on environment E1; independent of paper rigging.

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

- [ ] Add `assets/components/materials/kawaii-twilight-sky.mms` exporting a
  reusable material, with a small background wrapper if the scene needs one.
- [ ] Reuse the existing monochrome cloud texture and its spherical seam/pole
  handling; preserve independent cloud tint/strength and gradient controls.
- [ ] Start with art-tunable colors around pink `[1.0, 0.67, 0.78]`, lilac
  `[0.67, 0.53, 0.82]`, and upper blue-lilac `[0.35, 0.40, 0.70]`. These are
  provisional authoring values; verify color-space interpretation and screenshots.
- [ ] Expose moon direction, angular size, orientation, tint, and glow strength.
  Do not carry over the old sun or add stars by default unless the composition needs them.
- [ ] Integrate the background into `paper-craft.mms` while keeping suitable
  independent scene lighting; a visible bright sky is not itself a studio light.

Acceptance: both-eye views from roof, street, and stairwell show the intended
gradient/clouds; no panorama seam, pole artifact, camera-translation parallax, or
sky picking target. Original Sakura and ship sky behavior stays intact.

## S2 — Crescent-shaped glow, independent of bloom

Tentative preferred asset approach: an offline Rust generator following the
existing [`generate-sky-cloud-texture`](../../../examples/generate-sky-cloud-texture.rs)
example. Proposed outputs are a reproducible moon texture under
`assets/textures/sky/` and a documented generation command. No generator or PNG
has been created by this tracker.

- [ ] Rasterize an antialiased crescent at higher resolution and downsample.
  Its dark cutout removes only moon-body radiance; it must not paint a black
  disk over the underlying sky/cloud color.
- [ ] Bake a soft multi-scale glow from the actual crescent coverage, not a
  circular disk. Use sufficient transparent padding so tips and outer halo do
  not clip, and clamp sampling at atlas edges.
- [ ] Choose/document the texture contract. A transparent RGBA preview can bake
  body plus glow for inspection. For runtime, separate body/glow mask channels
  may give better control of brightness/tint and avoid low-alpha fringe problems.
- [ ] Composite body/glow radiance in the sky shader, preferably inside the
  existing sky draw. Texture transparency does not require a separate transparent
  scene quad, blending pass, or runtime blur target. Check available sampler bindings.
- [ ] Define straight/premultiplied alpha and linear/sRGB handling explicitly;
  verify downsampling, filtering, and black/colored fringe behavior.
- [ ] Keep glow present in the normal color output when bloom is disabled.
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
