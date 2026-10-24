# Shared Shading: first VR checkpoint

Date: 2026-10-22
Status: first sharing checkpoint and second internal-instance checkpoint accepted in VR by the user.
Parent: [multi-parent declarations](multi-parent-components-and-material-sharing.md).

2026-10-23: work resumed at the user's request. The
[second VR checkpoint](material-instances-anime-vr-checkpoint.md) introduces
retained family/definition/instance records and bounded GPU bindings behind the
same scene, with a new shared-pulse control. The user has now accepted that
updated scene too; generic parameters/resolution remain unfinished.

2026-10-22 headset follow-up: the original scene omitted the driven Transform
below each XRHand, leaving stationary scene-root lasers. The
[pointer bug and correction](../bugs/xr-hand-without-driven-transform-spawns-stationary-laser.md)
record the fix and avatar-free pose/ray regression. The user subsequently
confirmed that pointer interaction and the sharing workflow work in VR.

The existing single-parent automated checks are the accepted starting baseline.
Manual mittens-corp vehicle validation no longer blocks this material slice.
The first VR test is complete. The internal material-instance foundation is now
implemented for Anime/Toon. Custom programs, the LED wave and the burning shader
remain future slices after the second checkpoint.

## User-confirmed headset result

The user confirmed that the rightmost sphere remains independent, the shared
material can be detached from the other two objects individually, and both can
be reattached. Controller pointer interaction now makes the scene usable for
this test. This accepts the first shared-declaration workflow in VR.

The report does not separately establish every check below, such as panel
minimize/restore, the Move button, or a detailed stereo/rim comparison. Keep
those checks available without treating them as prerequisites to recording the
reported success. The broader multi-parent and material-system work remains open.

## Terminology and what exists today

| Term | Materials v3 responsibility | Current implementation |
| --- | --- | --- |
| MaterialBase | Shared geometry/shader interface, supported standard instance inputs, state profiles and renderer capabilities/pass contracts | Current Rust name `MaterialFamily`; initial fixed-mesh metadata implemented, broader contract pending |
| MaterialDefinition | Shader/program choice with its typed input schema within the base contract | Current Rust name `MaterialDefinition`; Anime/Toon records implemented, generic data/custom programs pending |
| Material (the instance) | Stable identity and retained mutable values/resource bindings for a definition, shared by its consumers | Current Rust name `MaterialInstance`; one retained renderer instance ID per initialized authored Shading source; bounded mutable Anime/Toon GPU bindings |

These are exactly three layers. MaterialBase is the new name for MaterialFamily;
the later 2026-10-23 authoring decision names the middle layer MaterialDefinition
and the instance Material. See the [constructor/type-table contract](material-component-and-resolved-contract.md#authoring-decision-2026-10-23).
A renderable/GPU
instance identifies a drawable (geometry, transform, color, opacity, emissive
intensity) that consumes a material instance; it is not a fourth material layer.

For the three test objects, there are two authored Shading sources: one shared
by the cube and middle sphere, and one owned by the rightmost sphere. All three
use the same built-in Anime shader path and retain separate renderable instances.
At the first checkpoint, Anime settings were copied into per-renderable state
and GPU bindings were cached by parameter values. The second checkpoint now
stores a material-instance ID in renderables/batches and reads retained shared
values through that ID. Scalar revisions no longer create value-keyed bindings.
The retained values/defaults and upload packing still use the Anime-specific
schema; the Shading-to-material integration is a compatibility bridge, not a
completed generic authoring/resolution system.

Agreed later consumers remain the LED and burning materials. The LED wave uses
shared time/scalar controls and continuous strip UVs on combined LED geometry,
with a separate backing material and no per-LED CPU animation. Burning needs
independent progress and airflow values per cigarette Material instance, a UV burn direction,
and matching cutout/glow boundaries in color, depth and emission. These are
future slices after the instance foundation and custom-program/emission work.

## Run and inspect

Start the OpenXR runtime, then run from the repository root:

```sh
cargo run --release -- load examples/shared-materials-xr.mms
```

The scene uses built-in primitives and the existing Anime program. The cube and
middle sphere share the exact same authored Shading component through two
attachments. The rightmost sphere owns an independently constructed source with
identical starting values. Objects are in separate transform subtrees.

Use either controller's laser and the panel to:

1. Change shade/rim sliders. Cube and middle sphere change together; rightmost
   sphere stays unchanged. Move your head to inspect the rim in both eyes.
2. Detach the middle sphere. It falls back to Toon. Sliders still change the cube.
3. Detach both. Both use Toon; the retained source can still be edited.
4. Restore both. Both immediately use the source's current values. Repeat the
   detach/restore cycle, including pressing Restore when both are attached.
5. Move the middle sphere. Its pose changes independently; the cube stays put.
6. Minimize and restore the panel. Fresh controls reconnect to the same source.

## Implemented boundary

- Only leaf `Shading.anime()` / `Shading.toon()` declarations support multiple
  parents. Ordinary components continue to reparent. Sharing a wrapper with
  spatial descendants, or adding children to an already-shared declaration,
  is rejected. Separate material sources are never deduplicated by value.
- `parents_of` enumerates attachment parents without allocating. `parent_of`
  and `attachment_of` return no singular answer for multiple attachments;
  `attachment_between(parent, child)` identifies an exact edge. Removing one
  edge preserves other edge references; recreating it does not revive old ones.
- `parent.detach(child)` removes one edge; `child.detach_all()` removes all
  incoming edges without destruction. Bare `child.detach()` rejects ambiguity.
  Duplicate shared edges and cycles are rejected before mutation.
- Subtree deletion preserves declaration leaves still attached outside that
  subtree. Explicit source destruction unlinks all consumers. Runtime system
  cleanup uses the same ownership set as World deletion.
- Shared source registration visits each affected consumer once. Edge edits
  refresh former consumers and restore their current fallback. Runtime shared
  attachment operations publish `AttachmentChanged { parent, child, attached }`
  rather than inventing a singular reparent event.
- Ordinary parent reads keep their inline fast path. Additional edge storage
  is allocated only for declarations with multiple parents.

## Evidence and remaining work

Focused graph and authored-scene tests cover unique initialization/query results,
three-parent edge revisions, rejected cycles/duplicates/spatial sharing,
detach/reattach, deletion with surviving consumers, equal-valued independent
sources, actual slider/button callbacks and renderer state after mutations.
Repeated Restore keeps the component count and source identity stable.

Final focused validation: 158 tests passed, one benchmark ignored. This covers
graph, Anime, attachment/mount cleanup, intent handling, live component methods,
material round-trips and refraction compatibility. Three unrelated round-trip
failures (`roundtrip_animation_paused`, `roundtrip_keyframe`,
`roundtrip_music_note_c5`) were excluded after reproducing them on unchanged
`7e4d0999` in a temporary archive. The broad library suite is not clean on that
baseline either; four audio-device/microphone tests were skipped after stalling.
The new attachment event's payload/documentation check passes. Formatting and
diff whitespace checks pass.

The second checkpoint adds material IDs, bounded mutable GPU uniform storage
and source-linked GLTF material consumption. CombineMesh sharing, broad graph
event/query semantics, graph performance benchmarks and custom materials remain
subsequent slices. The original automated evidence above records the first
checkpoint; new renderer validation is recorded in the second checkpoint.

Graph-preserving save/clone is pending. Serialization explicitly rejects a
subtree containing a multiply attached declaration, preventing silent conversion
of shared state into independent copies. Reload this authored MMS fixture to
restart the test. The user-confirmed headset result above supplies hardware
evidence for the reported sharing/controller workflow; the remaining individual
visual and panel checks are not inferred from headless callback tests.
