# Paper craft environment

Status: E1–E2 implemented, automated authoring/collision checks passed; headset acceptance pending (2026-11-09). E3–E4 remain planned.

## Baseline inspected

[`examples/sakura-drone-conversion.mms`](../../../examples/sakura-drone-conversion.mms)
has nine buildings: the player tower plus eight `neighborhood_specs` entries.
There are six eight-storey towers including the player tower, and three annexes
of two/three storeys. Storeys are 3.2 m; the current spawn roof is 25.6 m.
It has six cans, three pistols, a 3.2 m workbench against a wall, an invisible
street-level rooftop reset trigger, and XR flight enabled.

[`pool_building`](../../../assets/components/buildings/pool_building.mms) currently
adds stairs only from the top floor to the roof. Other slabs are complete floors;
repeating the stair mesh without creating slab openings would not create access.
[`tripod_light`](../../../assets/components/tripod_light.mms) is an existing reusable
studio fixture; [`e2.mms`](../../../examples/e2.mms) demonstrates mounted spotlights.

## E1 — New XR scene and lighter floating neighborhood

- [x] Add `examples/paper-craft.mms`, derived from the XR Sakura scene.
- [x] Retain one XR mitten rig, calibration, hand interactions, avatar/capsule
  setup, gravity readiness handling, and `enable_flying(true)`.
- [x] Keep five buildings total: main plus four neighbors. Nine → five is a
  44.4% reduction, within the requested 35–50% fewer buildings.
- [x] Tentative main tower: six storeys (19.2 m roof), down two from eight.
  Keep it at ground level for the walking route.
- [x] Tentative four neighbors: west/north tower, east/center tower, west/south
  tower, and one low annex, retaining their original XZ positions where practical.
  Use five, four, five, and two storeys respectively. Reduce most neighboring
  towers more than the main tower; a low annex can stay low.
- [x] Deterministic positive base-height fractions, e.g. 0.25, 0.75, 1.0, 0.5
  times each neighbor's own new full building height. These are floating offsets,
  not extra floors. Main tower is exempt so stairs can meet the ground.
- [x] Keep the broad ground, road, pavements, cars, and suitable Sakura planters.
  Reduced buildings should leave pleasant open views rather than require a new terrain system.
- [x] Remove the reset/teleport pit entirely, including callback/destination
  plumbing. Ground must be reachable and stay reachable. Do not replace it with
  another automatic rooftop respawn plane.
- [x] Remove all cans, can dispatch/state, and target-only UI/imports.
  Keep at most one pistol on a separate side table; never three on the craft table.
- [x] Keep a new scene namespace and update selectors, avoiding stale Sakura
  instance names. Avoid importing optional NPC/pedestal machinery unless useful.

Acceptance: scene bootstrap/count inspection shows five building roots, one XR
player, zero cans, zero reset sensors, and zero or one pistol. Heights and base
offsets match the spec. XR flight toggles and ascends/descends; landing on the
street does not teleport. Check solid collision and both-eye rendering. Run command: `cargo run --release -- load examples/paper-craft.mms`.

## E2 — Complete main-building stair route

- [x] Add an opt-in complete stairwell configuration or focused building variant,
  preserving existing `pool_building` calls and their current behavior.
- [x] Connect every adjacent level, ground floor through rooftop, with suitable
  landings, turnaround space, slab openings, and capsule/head clearance.
  Reuse a stairwell footprint with switchbacks/returns where practical.
- [x] Make a real ground-floor exit and approach from the retained ground.
- [x] Derive roof height, spawn, benches, mirror, panels, tree, and pistol table
  placement from the new main-tower configuration, not the old 25.6 m constant.
- [x] Keep floor/roof batching and collision in agreement with openings; no
  invisible complete-slab collider across the stairs. If destruction remains,
  retain sensible per-floor ownership rather than unpacking the whole building.

Acceptance: inspect all slab/collider openings, then walk down and back up with
flight disabled and without jumping through geometry or firing the pistol.
Check each landing, turn, roof transition, and street exit at tracked standing
height. Repeat the route in the headset; authored dimensions alone are insufficient.

## E3 — Walk-along craft bench

- [ ] Replace the short wall workbench with a long table, tentatively 6–8 m long,
  0.9–1.1 m deep, top near 0.9 m above the roof. Tune to actual rooftop fit.
- [ ] Leave a clear walking aisle, tentatively at least 1.2 m, and keep the
  stairwell entrance and settings/mirror access unobstructed. “Walk along” means
  walking beside the table to inspect stages; standing on it is not required.
- [ ] Add stable named station mounts for flat pattern, weight visualization,
  folded rest shape, manual pose, and moving gohei. Empty labeled placeholders
  are acceptable until their corresponding rigging slices land.
- [ ] Fit stationary specimens and a manipulable final prop without overlap;
  use restrained lighting/materials so cuts, folds, and normals are readable.
- [ ] Put the optional single pistol on its own table away from the specimens.

Acceptance: walk the full station sequence in XR, inspect at hand distance,
reach the mount locations, and walk to the stairs without props/panels blocking
the route. Keep labels legible without obscuring the paper.

## E4 — Carryable inspection light

- [ ] Reuse/adapt the existing tripod fixture or make a compact studio-light
  variant with a practical grip; retain a supported local spotlight path.
- [ ] Grab and aim the complete light root. Visible housing and actual light
  must move together; its beam follows the held orientation, not a fixed world target.
- [ ] Tune neutral light, cone softness, reach, and ambient fill so turning the
  light toward/away from the paper visibly changes its shading.
- [ ] Check front/back paper lighting and culling. Do not promise new shadows,
  translucency, or paper transmission as part of the portable-light slice.

Acceptance: grab, translate, rotate, carry between stations, release, and regrab
in XR. Compare the same paper view with the lamp aimed on/off it. Test the light
under the sky with bloom disabled too; glowing housing alone is not illumination.

## E1–E2 implementation and handoff (2026-11-09)

Implemented `examples/paper-craft.mms` with the five-building layout above,
19.2 m main roof, one pistol, no cans/NPC pedestal/reset pit, and the existing
Sakura sky and short bench at the E1–E2 handoff. The subsequent sky slice now
uses the kawaii twilight material; the long bench remains a later slice. The four neighbor bases are
4.0, 9.6, 16.0, and 3.2 m respectively. A small ceramic entrance apron connects
the main building doorway to the existing pavement.

The new opt-in `pool_building_with_stairs` retains per-floor batches. Each
storey has two 1.8 m wide open-riser flights, twenty 0.16 m rises in total,
0.28 m treads, and a 1.2 m deep intermediate turnaround. Matching floor/roof
openings leave front landings and a ground-floor exit. Thin treads preserve
headroom beneath stacked flights; solid step blocks did not.

Automated scene checks cover expected transforms/floor counts, one XR camera,
flight enabled, one pistol, absence of old player/reset/can roots, derived spawn
height, actual collidable tread/landing support, and a 0.6 m wide by 2.0 m high
sampled standing envelope along every flight, turnaround, front landing, and
street exit. They also check solid ordinary roof and open stairwell slabs.
These are authoring geometry checks, not a simulated capsule traversal or a
claim of completed headset acceptance.

Validation: `cargo test --lib paper_craft -- --nocapture` passed (one new
scene/collision test). `cargo test --lib -- sakura_scene_has destruction_bindings
flight_` passed (nine selected regression tests). Existing compiler warnings
remain unrelated to these changes.

Headset handoff:

- [ ] Launch `cargo run --release -- load examples/paper-craft.mms`.
- [ ] Walk to the roof stair opening toward the rear-left of the roof
  (world X roughly -23.8 to -19.4, Z 1.6 to 5.6).
- [ ] With flight off, descend the east lane, turn at each half-storey landing,
  and continue through all six storeys; exit toward the road through the ground
  floor doorway. Walk back up to the roof without shooting or flying.
- [ ] Check both-eye rendering, step climbing, headroom, and turn clearance at
  tracked standing height; note any snag location and floor.
- [ ] Double-tap B to toggle flight; hold B/A to rise/descend. Land on the street
  and confirm there is no automatic rooftop reset.
- [ ] Check rooftop placement and the four floating neighbors from roof and street.
