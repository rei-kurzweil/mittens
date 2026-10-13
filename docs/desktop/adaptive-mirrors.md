# Adaptive mirror detail

Date: 2026-09-02

Status: architecture decisions accepted; implementation planned after baseline
viewer-family mirror correctness and observability.

[Back to the desktop workbench](README.md)

## Outcome

Spend mirror pixels where they are visible:

- distant or small on-screen mirrors render at lower resolution;
- nearby mirrors increase useful pixel density without unstable resolution
  flicker; and
- when only a close-up subregion of a mirror is visible, render more detail for
  that visible region instead of paying for the entire authored mirror surface.

Do not move or resize authored mirror geometry as an optimization side effect.
Represent close-up concentration as a capture crop/tile, viewport/scissor, or
equivalent projection mapping whose UV transform is explicit and stable.

## Existing foundation

- Mirrors already carry an authored `quality`/resolution scale and use
  per-mirror offscreen targets.
- [Generalized LOD policy and selection](../task/generalized-lod-policy-and-selection.md)
  is the canonical tracker for reusable selection policy.
- [Mirror viewer-family captures](../task/mirror-viewer-family-captures.md)
  records the need for independent mono and stereo capture families.
- [Mirror camera orientation and tracking](../bugs/mirror-camera-orientation-and-tracking.md)
  remains a correctness issue. Adaptive quality must not conceal it.
- [Mirror rendering improvement list](../task/mirror-render-pass-status.md)
  records current capture support and planned plane clipping, off-screen
  skipping, optional MMS candidate tracking, and performance verification.
- [Importable MMS graphics settings panel and mirror controls](../task/runtime-mirror-resolution-info-panel.md)
  tracks manual `GraphicsSettings.mirrors.toggle(...)` and `.quality(...)`
  controls and their authoritative `GraphicsSettingsSystem` owner separately.
  Toggle off pauses captures while retaining the last reflection textures.
  That asset/API slice does not require adaptive LOD;
  define how its runtime quality override interacts with the authored ceiling
  before combining the two policies.

## Locked architecture decisions

- Adaptive mirror quality is mediated by the generalized `LODComponent` policy.
  `MirrorSystem` consumes its selection and remains the owner of mirror-specific
  capture behavior.
- Projected screen coverage, including size and foreshortening, is the primary
  quality signal. Raw distance is not the default because it misprices large,
  small, and obliquely viewed mirrors.
- Selection uses a small number of resolution bands with asymmetric hysteresis
  and cooldown. Mirror targets are reused within a band rather than resized
  continuously.
- `Mirror.quality(N)` remains the authored preferred/native ceiling. LOD may
  reduce the effective extent and may respect an authored minimum, but does not
  silently exceed the ceiling.
- Runtime selection is per viewer family. Mono and stereo may require different
  tiers in the same frame; the stereo family uses one coordinated selection
  based on the greater requirement across its eyes.
- LOD selects detail; it does not move or resize authored mirror geometry.
- Close-up concentration is a later mirror-owned capture crop/tile or projection
  feature with explicit UV remapping. It is not a responsibility of the generic
  LOD selector.

## Remaining design questions

1. Which concrete extent bands and projected-coverage thresholds provide good
   image quality on the target desktop and XR resolutions?
2. Should projected coverage be measured as clipped pixel area, longest visible
   axis, or both for very thin/grazing mirror projections?
3. For close-up partial visibility, should the renderer crop to the visible
   mirror polygon's screen-space bounds, use fixed tiles, or retain a full
   target with a smaller high-detail inset?
4. How are crop/projection and UV remapping published so the surface samples
   the right portion of the capture?
5. What budget caps total mirror pixels and capture count across many mirrors,
   and what is the deterministic degradation order?

## Milestones

- [ ] Add mirror counters/timings: logical mirrors, capture views, target
      extents, allocated pixels, visible projected pixels, and GPU time.
- [ ] Add a repeatable scene with one mirror viewed far, medium, near, at a
      grazing angle, and as a partially visible close-up.
- [ ] Fix/validate viewer-family orientation and parallax before judging image
      quality changes.
- [ ] Implement the first slice of the generalized LOD selector: per-family
      projected coverage, discrete tiers, hysteresis, and cooldown.
- [ ] Map LOD tiers to mirror capture extents bounded by the authored
      quality ceiling and configured minimum extent.
- [ ] Reuse targets within bands; prove normal camera motion does not allocate
      every frame.
- [ ] Design and prototype close-up visible-region cropping with explicit UV
      remapping and guard pixels for filtering.
- [ ] Add a total per-frame mirror pixel budget and a deterministic degradation
      order for multiple mirrors.
- [ ] Verify no left/right stereo mismatch, edge shimmer, stale crop, or visible
      resolution pumping.
- [ ] Compare GPU time and image error against a fixed high-resolution baseline.

## First safe slice

Land generalized LOD selection plus mirror resolution bands only. Keep the full
mirror capture region and select a target extent from projected on-screen
coverage. Close-up cropping changes the capture projection and sampling
contract, so it remains a separate second slice after banding is stable.
