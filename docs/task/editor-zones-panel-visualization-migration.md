# Task: Optional editor Zones panel and diagnostic organization

Date: 2026-10-12
Status: deferred panel-organization work. Zone and collision-region
visualization already works through Show Zones in Editor Settings.

## Current behavior

`ZoneVisualizationSystem` renders enabled, resolved `ZoneComponent`s under the
effective editor roots. These include contact geometry used by `Collidable`,
runtime AVC capsules, and vehicle-entry regions. This is implemented and has a
real Settings-panel off/on/materialize/off cleanup regression.

The Settings panel's full diagnostic control set is Show Armature, Show Bounds,
Show Cameras, Show Spring Bones, and Show Zones. All controls are included by
default, while live visibility starts off. Explicit `EditorUI` configuration
can omit a control. See the [EditorUI settings contract](../spec/editor-ui.md#settings-panel-controls).

Show Colliders and Show GLTF Colliders, including their typed config fields,
parser/serializer keys, row factories, and click handling, have been removed.
They do not need to be recreated in a new panel. There is no outstanding task
to make contact geometry visible, and removing redundant Settings controls
must not depend on implementing this optional panel.

Show Spring Bones still controls secondary-motion diagnostics separately.
Nothing in this task changes collision, zone queries, mounting, or simulation.

## Remaining optional scope

If a dedicated Zones panel is still useful, move the existing Show Zones
control there using the shared panel/accordion shell. Reuse the same
owner-scoped request and authoritative visibility state; do not create another
visualizer or independently mutable flag.

A later panel can add role/source filtering of the existing zone markers or
split secondary-motion collider spheres from chain segments. These are
organization/filtering features, not prerequisites for collision visualization.
Do not label spring chain segments as spatial zones or reintroduce separate
all-collider/GLTF-collider Settings controls.

## Decisions for that future implementation

1. Whether to add an optional `EditorPanel::Zones` / authored `zones` name,
   and whether a bare `EditorUI {}` includes it. Explicit panel omission must
   avoid materializing its shell or installing a request owned only by it.
2. Which existing diagnostic controls move there. Show Spring Bones remains
   in Settings until a separate migration explicitly changes its contract.
3. Whether simple zone-role/source filters are needed. Implement only filters
   with a concrete use case; preserve one marker per source zone across owners.
4. Whether spring collider and chain visibility should be independently
   selectable. Keep secondary-motion visualization a read-only solver consumer.

## Lifecycle and verification

- Build through the existing `PanelKind`/`PanelBodySpec` factory path and shared
  accordion minimization behavior.
- Minimizing suspends the panel body without disabling an active overlay.
  Restore the current toggle/filter values without duplicate listeners.
- Removing the sole request owner cleans up its markers; late refreshes must
  not recreate a removed panel or overlay.
- Preserve exact query frame/shape semantics for cubes, spheres, and Y
  capsules, including resolved references, rotations, and non-uniform scale.
- Preserve runtime-only, non-selectable, non-raycastable markers and predictable
  request union across editor owners.
- Verify desktop/XR readability, source/owner removal, live zone edits,
  minimize/restore cycles, and stable marker/handler counts.
- Contact correction, grabbing, mounting, and secondary-motion simulation must
  behave identically with diagnostics off and on.

## Related work

- [EditorUI settings contract](../spec/editor-ui.md#settings-panel-controls)
- [Interaction zones on the collision-query foundation](interaction-zone-collision-query-foundation.md)
- [Rider + Mountable attachment-system first slice](rider-mountable-attachment-system-first-slice.md)
- [Editor panel minimize and render suspension](editor-panel-minimize-and-render-suspension.md)
- [Live panel factory and stopgap adapter seams](live-panel-factory-spawn-and-stopgap-adapter-seams.md)
