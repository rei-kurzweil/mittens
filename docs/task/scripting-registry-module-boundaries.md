# Scripting registry module boundaries

Keep drop-receiver authoring and event payloads with Zone and grab interaction
routes in a future split. Preserve direct payload conversion and scoped event
delivery. Validate both MMS builder round trips and release-only ZoneDropped
delivery for zone and point probes.

Group Collidable constructors and builders with the Zone interaction registry.
Keep the direct component construction path and validate MMS round trips for
static, slide, and sensor collidables, including their movement-target references.

Group the AmbientOcclusion constructor/builders (added for
`docs/task/ambient-occlusion-baking.md`) with the rendering/materials family.
Validate MMS round trips for `texcoord0`, the numeric builders and debug modes.

Status: proposed follow-up. Further splits require an explicit request or approval.

## Observation

The scripting entry points remain large after grouping focused modules in
`src/scripting/registries/`:

| File | Approximate lines |
| --- | ---: |
| `src/scripting/component_registry.rs` | 4,933 |
| `src/scripting/component_method_registry.rs` | 1,774 |
| `src/scripting/runtime_config.rs` | 1,701 |

These are review signals, not automatic refactoring thresholds. The current
organization change moves existing focused modules and groups all Renderable
constructors/signatures together. It does not implement the wider proposal below.

## Proposed split

- Keep central dispatch and CE tree materialization entry points stable.
- Group constructor/builder implementations by coherent component family,
  such as rendering/materials, avatar/rigging/tracking, audio, input, and editor
  controls. Avoid one file per individual primitive or builder.
- Keep the corresponding MMS API declarations grouped at the same family level.
  Consolidate declarations where that makes ownership clearer; do not replace
  one large registry with a large number of tiny files.
- For runtime methods, separate substantial family implementations from the
  central dispatch only when their responsibility is independently useful.

Before implementation, inventory the actual functions and dependency direction,
then agree on the concrete boundaries. Moving a file does not create a new
registry mechanism; preserve current module/API relationships where practical.

## Constraints and validation

Retain direct/batched paths and typed host bindings. Do not add per-draw
allocations, dynamic dispatch, repeated pipeline/resource construction, or new
per-frame work solely to separate files. Measure if a proposed boundary changes
meaningful runtime work; directory-only moves need no performance redesign.

Compare API names, aliases, signatures, defaults, and optional argument counts
before and after. Run registry/schema checks, component roundtrips, and relevant
scene loads. Keep unrelated API migrations separate.

Current broader checks expose four failures: the marker-schema test
does not exempt `Joint`, and the legacy roundtrip helper cannot handle Animation,
Keyframe, or the current MusicNote representation. Record/fix those independently
instead of changing semantics to make a file reorganization pass.

## Note (2026-11-13)

The multiplayer `MultiplayerSession`/`MultiplayerSync` bindings were added as one family pair,
`registries/multiplayer_registry.rs` and `registries/multiplayer_runtime_config.rs`, with
single-line dispatch hooks in the central files. New families should follow
the same shape rather than growing the central registries.

## Naming review (2026-11-13)

Rename the multiplayer family and central dispatch references consistently.
This scoped rename adds no runtime work; the responsibility-based split above
remains proposed. Validate with multiplayer and documentation catalog tests.

## MMS channel integration

UDP authoring stays in a cohesive registry/config pair; central dispatch receives only routing hooks. Preserve batched/direct dispatch and avoid per-target per-frame allocations. Validate with channel MMS and avatar tests.

Instance `.from(...)` uses the same contextual member-name parser as constructor chains. No new block or factory syntax is introduced.

### Host method-family follow-up

The scoped channel slice also touches the large scripting/host.rs dispatcher.
A future responsibility split should group transform/pose/morph host methods
behind one coherent pose-data adapter, leaving handle validation, lease ownership
and operation routing in host.rs. Keep typed direct calls, shared value conversion
and existing phase gating; avoid per-operation runtime dispatch or new per-frame
allocations. Validate language tests, partial transform execution, morph ownership
and two-peer avatar tests. This proposal does not perform the split.

## Authored morph-animation API

Group GLTF morph read/write/clear operations in a cohesive morph API adapter,
separate from UDP authoring. Keep host.rs and runtime_config.rs dispatch-only
for this family. Resolve slots per GLTF instance and preserve direct factor
updates; no per-draw allocation or new renderer pipelines. Validate script
weights, tracker/network precedence, animation callbacks and the two-peer demo.

## Shared camera-free avatar eye calibration

The multiplayer proxy needs the same eye-origin calibration as a camera-bearing
local rig. Add a generic explicit eye_offset vec3 to AVC, preserving the existing
camera-wrapper fallback when omitted. The large AVC component/initialization and
component_registry dispatch should eventually group pose/eye calibration config,
serialization, and initialization coherently; keep direct scalar/vector math and
no per-frame allocations or dynamic dispatch. This scoped addition does not
perform the proposed split. Validate finite inputs, MMS serialization, camera
view/driver agreement, and local/proxy foot contact.

## MultiplayerSync removal

Remove the superseded automatic replication component and its lifecycle intent
from the large component registry, runtime configuration, signal routing and
SystemWorld teardown modules. Keep this a scoped deletion. Existing proposed
responsibility boundaries remain; any later split must preserve direct matches,
batched teardown and typed-channel hot paths without new allocation/dispatch.
Validate MMS rejection of the removed API, session lifecycle/messages, typed UDP
bindings and the two-peer avatar example.

Removal verification: 14 multiplayer checks pass (session lifecycle/messages,
stale rosters, removed MMS API rejection, desktop/XR configuration and two-peer
Mitten typed poses/blinks/IK), plus 25 UDP checks (1 intentionally ignored).
The session no longer stores, forwards, publishes or applies poses. Legacy v2
pose wire fields remain recognized by the codec for compatibility and codec
regressions; the live endpoint ignores them.

## Scripting test organization

Group the scene integration tests in `src/scripting/tests.rs` by feature and
keep legacy evaluator parity tests separate from configured runtime scene tests.
Reuse the existing scene setup and callback delivery helpers. Preserve the
number of scene loads and per-frame updates so the split adds no work to the
test suite or engine. Validate with the focused scene filters and the full
library suite.

## Transform operator bindings

Group TOP constructors, builders, and runtime method declarations into one
transform-operator family. Keep angle reads and writes on the typed host
dispatch path and refresh the affected transform subtree through the existing
intent. Validate constructor signatures, live angle updates, and scene
round trips after moving the family.
