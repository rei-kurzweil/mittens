# Task: Desktop multi-layer Bloom

Status: scoped — implement desktop first; XR is explicitly deferred.

## Goal

Add `BloomComponent::layers(n: u8)` and MMS `Bloom.layers(n)` to opt into a
small, shared multi-scale Bloom pyramid. Preserve the existing single-level
Bloom path exactly as the default and lowest-cost option.

`layers(1)` is the compatibility mode: one blur level at the configured
`half_res` scale, using the existing extraction, two-pass blur, and composite
behaviour. It must remain the default when the builder is absent.

For `layers(n > 1)`, each additional level halves both dimensions relative to
the previous Bloom level. A three-level, half-resolution configuration is:

```text
emissive source -> 1/2 blur (tight glow)
                -> 1/4 blur (medium glow)
                -> 1/8 blur (wide glow)
                -> weighted upsample/add -> one Bloom contribution
```

This is Bloom's own pyramid; it is unrelated to the rough-transmission scene
colour pyramid. Bloom must be completed before a transmissive snapshot is
made, so frosted and refractive materials receive the final multi-scale glow.

## Authoring contract

```mms
RenderGraph {
    Bloom.layers(1) // default and current visual/cost behaviour
    Bloom.layers(3).intensity(1.0).radius_ndc(0.045)
}
```

- `layers` accepts integer `u8` values in `1..=3` for this first slice.
- Invalid values (`0`, values above `3`, non-integers, non-finite values) are
  rejected by the same component-builder validation path used by other Bloom
  inputs.
- `layers(1)` means one total filtered Bloom level, not one *additional*
  level.
- `intensity` remains the final Bloom contribution strength. It does not
  implicitly change layer count or allocation.
- `radius_ndc`, `half_res`, `emissive_scale`, source selection, and debug
  texture publication keep their established meaning.
- The first version uses renderer-defined, documented layer weights. Do not
  expose per-layer weights or material-specific layer counts yet.

## Desktop implementation

1. Extend `BloomComponent`, `BloomConfig`, component registry/configured
   registry, MMS serialization, and round-trip tests with `layers: u8`,
   defaulting to `1`.
2. Replace the fixed `bloom_a`/`bloom_b` target pair with a bounded collection
   of per-level ping-pong targets. Keep the single-level allocation and pass
   sequence unchanged when `layers == 1`.
3. Build level 0 using the current Bloom source and configured resolution
   (`half_res` means 1/2 width and 1/2 height). Build every later level from
   the preceding final level at half its width and height, clamped to at least
   one pixel per dimension.
4. Run the existing horizontal and vertical blur at every requested level.
   Use a defined radius policy that produces progressively wider screen-space
   glow without enormous full-resolution kernels; document the policy next to
   the layer weights.
5. Add a fullscreen multi-input Bloom composite that linearly samples each
   finished level at output resolution and sums it with its fixed weight before
   applying the existing global `Bloom.intensity`. `layers(1)` must continue
   to use the current single-input composite/pipeline when practical, to make
   its compatibility and cost easy to prove.
6. Keep current pass ordering: emissive extraction, Bloom filtering,
   opaque-plus-Bloom snapshot when transmission is active, transmissive phase,
   and final output. Bloom must still contribute exactly once per final pixel.
7. Define `Bloom.output_texture` for multiple layers. Recommended first
   contract: it publishes the final weighted Bloom contribution, not an
   arbitrary internal level. Add named per-level debug output only if the
   existing render-image registry can express it without ambiguous ownership.
8. Recreate/release targets on desktop resize, format changes, post-processing
   configuration changes, and layer-count changes. Keep the no-Bloom and
   no-emissive early-outs allocation-free.

## Fixture and validation

Use [`examples/multi-layer-bloom.mms`](../../examples/multi-layer-bloom.mms):

- It is a black-background `LayoutRoot` 12 x 12 grid, repeating all eighteen
  shapes from `assets/components/primitives.mms`.
- Every shape is emissive at `1.5..=2.5`, with saturated but non-primary
  colours.
- Its desktop camera has local +Z movement and local-Z roll, without
  `fps_rotation()`.

Before implementation it intentionally uses the working one-level Bloom
configuration. As part of this task, make `Bloom.layers(3)` the fixture's
default and add an easy `layers(1)` comparison configuration or documented
toggle.

Required checks:

- MMS parse/materialize/serialization round trip for absent `layers`,
  `layers(1)`, and `layers(3)`; invalid builder inputs fail clearly.
- `layers(1)` has the previous single-level target allocation, blur pass
  count, output, and no-emissive skip behaviour.
- `layers(2)` and `layers(3)` allocate only their requested levels, are
  resize-safe, and do not sample uninitialized texels.
- The fixture visibly retains a tight halo while adding medium/wide glow at
  three layers; the output is stable as the camera moves.
- Test Bloom with and without MSAA, with Bloom disabled, with no emissive
  objects, and with sharp/rough transmission enabled.
- Record desktop GPU time, image memory, pass count, and image read/write
  bandwidth for 1, 2, and 3 layers at both half and full Bloom resolution.

For half-resolution Bloom, the two ping-pong targets for `layers(1)` consume
`2 * 1/4 = 0.50` full-resolution-image equivalents. Adding 1/4 and 1/8 levels
adds `2 * (1/16 + 1/64) = 0.15625`, roughly 31% more Bloom working-buffer
pixels. Measure actual device allocation and bandwidth rather than treating
this estimate as a budget proof.

## XR follow-up — do not implement in this task

XR must retain the same `layers(1)` compatibility mode, but it needs separate
pyramids for the left and right eye. The current renderer renders and
post-processes eyes independently, so target memory and filtering bandwidth
are effectively incurred once per eye. Do not share a Bloom image between
eyes: it would break stereo registration.

After the desktop exit criteria are met, create a separate XR task to:

1. allocate/reuse the bounded per-eye target collections;
2. run and validate every layer per eye with the exact eye viewport and
   orientation;
3. measure 1/2/3-layer cost and memory on representative headsets; and
4. consider multiview only as a later CPU-recording optimization, not as a
   reason to weaken per-eye output validation.

## Exit criteria

- `Bloom.layers(1)` preserves current desktop behaviour and avoids extra
  pyramid allocation or passes.
- `Bloom.layers(2)` and `Bloom.layers(3)` produce stable, weighted,
  multi-scale desktop glow with bounded targets.
- The new fixture and automated authoring/resource-lifecycle coverage pass.
- Desktop cost data is recorded before XR work begins.
