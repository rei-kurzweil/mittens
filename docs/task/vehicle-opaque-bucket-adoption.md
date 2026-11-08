# Vehicle example opaque bucket adoption

Status: user reports improved VR performance and 90 FPS in Mittens Corp;
vehicle/example adoption checkpointed before scoped-invalidation work.

Implementation: `eb5cafc2`. API/benchmark documentation: `acf5c419`.
This follow-up updates shared vehicle fixtures rather than duplicating changes
in every caller. No new engine behavior, control loops, geometry or shader math.

| Fixture | Bucket | Covered examples/callers |
|---|---|---|
| Imported pickup and procedural pickup prototype | Near | Tracked/local ship cafeteria truck, orchard trucks and `ship_driving_vehicle_group`; already authored in the implementation commit |
| Alternate pickup Anime preset | Near | Existing fallback preset; parameters unchanged |
| Shared pink display car | Near | Mittens Corp XR/desktop/AGC, both Sakura drone conversions and toon-outline-bisket |
| `examples/car.mms` occupied car | Near | Desktop cockpit car |
| `examples/car.mms` parked scenery car | Middle | Separate non-enterable studio car |
| RV opaque cabin/body | Near | `ship_motorhome_group`, including the enterable interior |
| Motorcycle frame/body | Near | Ship vehicle groups and the rider patrol fixture |
| Shared Corp studio deck, floor, steps and back wall | Far | Corp variants, shared studio consumers and equivalent inline studio geometry in car/toon-outline-bisket |

The pink-car material copies all seven existing Bisket Anime inputs exactly:
shade color/strength, shade/lit thresholds, rim color/strength/power. Character
`bisket_anime_shading()` stays unchanged. Imported colors/textures are retained;
only its material API/ordering metadata changes. The studio now explicitly uses
standard Anime Surface material at Far, retaining per-object colors.

Ship environment preset accepts an optional bucket, defaulting to Far. RV and
motorcycle uses request Near while preserving their existing ship lighting
parameters. Generic vehicle glass remains in its existing transparent path.
No automatic mount/distance-based switching or interior/exterior model split is
added. Near is a static scheduling hint and may be less helpful from other views.

The normal local Miku ship imports the canonical truck and is covered. Historical
ignored GPU-cheap diagnostic variants remain benchmark controls; they are not
normal examples and are not migrated or committed with this adoption change.

## Validation and smoke test

Twelve existing headless tests passed: nine Mittens Corp tests (including mounted
input/attachment behavior), Sakura scene construction, Sakura desktop avatar
initialization and the outline example. The release app build and
`git diff --check` passed. Optional XR-runtime availability warnings in headless
runs are not a live headset acceptance test. No new tests were added solely to
mirror these reversible preset substitutions.

For VR review, enter/exit the pink car in `examples/mittens-corp.mms`, look across
the stage from outside, inspect floor/wall appearance, and test the usual ship
cafeteria/orchard trucks. Confirm mount/input behavior and imported color/texture
appearance along with frame time. Other affected public scenes include
`examples/mittens-corp-desktop.mms`, `examples/mittens-corp-agc.mms`,
`examples/sakura-drone-conversion.mms`,
`examples/sakura-drone-conversion-desktop.mms`, `examples/toon-outline-bisket.mms`
and `examples/car.mms`.

The user reported the Mittens Corp VR improvement after these changes. Broader
scene-specific visual review can continue; this adoption is checkpointed separately
from the renderer invalidation changes.
