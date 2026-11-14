# Scripting registry module boundaries

Status: proposed follow-up. Further splits require an explicit request or approval.

## Observation

The scripting entry points remain large after grouping focused modules in
`src/scripting/registries/`:

| File | Approximate lines |
| --- | ---: |
| `src/scripting/component_registry.rs` | 4,817 |
| `src/scripting/component_method_registry.rs` | 1,774 |
| `src/scripting/runtime_config.rs` | 1,641 |

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
