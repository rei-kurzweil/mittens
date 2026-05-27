# MMS Conversion Candidates — Analysis

Date: 2026-03-24

Assessment of all `examples/*.rs` files for conversion to `.mms` scene description format,
following the pattern of `vr-input.mms` / `vr-input-mms.rs`.

**Converted:** `vr-input.rs` → `vr-input.mms` ✓
**Converted:** `vtuber-desktop.rs` → `vtuber-desktop.mms` ✓

---

## What MMS can express (as of this assessment)

- Component tree topology (parent → child nesting)
- Constructor calls with literal args: `GLTF.new("path")`, `I.speed(1.5)`, `BGC.rgba(...)`
- Builder/setter method chains: `position()`, `scale()`, `head_bone()`, `forward_plus_z()`
- All registered component types: T, C3D, CXR, CTLXR, InputXR, I, InputTransformMode,
  BGC, AL, DL, PointLight, R, C, EM, GLTF, AVC, BG, TXT, TextBackground, TextShadow,
  TextureFiltering, TransformPipeline (and fork/map/merge/output ops), RendererSettings,
  RendererStats, ED, XR, Raycastable (marker), QuatTemporalFilter

## What MMS cannot express (blockers)

| Blocker | Affected examples |
|---|---|
| **Loops / procedural generation** — `for i in 0..N` keyframe/grid/object loops | `animation-example.rs`, `transparent-cutout-example.rs`, `openxr.rs`, `animation-for-topology.rs`, `raycast-topology-animation.rs` |
| **Pseudo-random cloud generation** — `hash_u32`/`rand01` helpers driving `spawn_cloud_ring` | `background-example.rs`, `background-occlusion-example.rs`, `vtuber-example.rs`, `font-example.rs` |
| **Helper functions with mutable state** — `spawn_text_block` advances a mutable `y` variable across calls | `font-example.rs`, `opacity-example.rs` |
| **Post-spawn topology queries** — `find_component` to locate GLTF bones after forced spawn | `vtuber-desktop.rs` (bone marker overlay), `vtuber-joints-example.rs` |
| **`RayCastComponent` (camera raycaster) + `PointerComponent`** — not registered in MMS registry | `vtuber-desktop.rs`, `font-example.rs`, `gestures-and-gizmos.rs` |
| **Event handlers / closures** — Rust `move` closures registered as gesture/interaction handlers | `button-press.rs`, `gestures-and-gizmos.rs`, `raycast-topology-animation.rs` |
| **`AudioOscillatorComponent` scheduling, `ActionComponent` with `IntentValue`** — audio intent payloads not representable as MMS literals | `animation-example.rs`, `audio-graph-example.rs` |
| **Physics / collision setup** — `KineticBodyComponent`, force fields, perimeter mesh construction | `gravity-fields.rs`, `collision-perimeter.rs` |
| **`MeshFactoryComponent` / procedural mesh** — mesh generation APIs not registered | `mesh-factory-example.rs` |
| **`example_util` helpers** — `spawn_cloud_ring`, `spawn_desktop_camera_controls_hint` are Rust closures; the controls hint *can* be inlined in MMS (vr-input.mms does this); the cloud ring cannot | all examples using `example_util` |

---

## Per-example verdict

| Example | Lines | Verdict | Blocking reason |
|---|---|---|---|
| `vr-input.rs` | — | ✅ converted | — |
| `vtuber-desktop.rs` | 168 | ✅ converted | bone-marker loop omitted (debug-only) |
| `vtuber-example.rs` | 175 | ❌ | `spawn_cloud_ring` (pseudo-random, loop-based) |
| `text-example.rs` | 202 | ❌ | `spawn_red_cube` / `spawn_text_style` helpers (inlinable but `PointerComponent` not registered) |
| `transparent-cutout-example.rs` | 221 | ❌ | 10×10 grid loop |
| `background-example.rs` | 224 | ❌ | `hash_u32`/`rand01` random cloud scatter |
| `background-occlusion-example.rs` | 224 | ❌ | same random cloud scatter |
| `simple-demo.rs` | 295 | ❌ | `build_demo_scene_7_shapes` helper fn with loops |
| `animation-example.rs` | 367 | ❌ | 16-keyframe loop + `ActionComponent`/`IntentValue` audio scheduling |
| `font-example.rs` | 374 | ❌ | `spawn_text_block` + mutable `y` stacking + cloud ring |
| `animation-for-topology.rs` | 406 | ❌ | animation keyframe loops |
| `raycast-topology-animation.rs` | 434 | ❌ | event handler closures + animation loops |
| `opacity-example.rs` | 469 | ❌ | `spawn_cube`/`spawn_text_label`/`text_block_dimensions` helpers with computed layout |
| `collision-perimeter.rs` | 499 | ❌ | physics/collision setup, procedural mesh |
| `button-press.rs` | 586 | ❌ | `move` closure event handlers |
| `vtuber-joints-example.rs` | 793 | ❌ | post-spawn bone queries, joints loop |
| `audio-graph-example.rs` | 741 | ❌ | audio graph wiring, `ActionComponent` payloads |
| `gravity-fields.rs` | 1058 | ❌ | physics force fields, procedural object placement |
| `gestures-and-gizmos.rs` | — | ❌ | drag/gesture event handlers, `PointerComponent` |
| `mindmap-example.0.3.rs` | — | ❌ | complex interaction graph |
| `folder-text.rs` | — | ❌ | filesystem traversal |

---

## What would unlock more conversions

1. **Loop syntax in MMS** — `repeat(N) { ... }` or `for i in 0..N` — would unblock
   `transparent-cutout-example.rs` and the 16-keyframe `animation-example.rs`.

2. **Register `RayCastComponent` (camera raycaster mode) + `PointerComponent`** — would unblock
   the camera picking setup present in `vtuber-desktop.rs`, `text-example.rs`, `font-example.rs`.

3. **Register `AudioOscillatorComponent` + `ActionComponent`** — would unblock audio examples,
   though the `IntentValue` enum payloads are complex to express as MMS literals.

4. **Procedural seed / noise expression** — `rand(seed, index)` or a `CloudRing` macro — would
   unblock all the cloud-ring examples, but this is deep language territory.

5. **Named component references** — ability to bind a component to a name and reference it from
   a different subtree (e.g. `let kick_cube = ...` then reference it in an `ActionComponent`) —
   would be needed for the animation timeline visualization in `animation-example.rs`.
