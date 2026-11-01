# Mittens 0.10.0 release notes

This release pairs `mittens-engine 0.10.0` with `meow-meow-script 0.10.0`.
`mittens-query` remains at `0.6.0`; the private Android host keeps its own
`0.1.0` package version. The desktop window title follows the engine package
version and now displays `mittens 0.10.0`.

Documentation media and design sources are excluded from the crates.io archive
to meet its 10 MiB upload limit. Bundled runtime component assets, textures,
audio, and shaders remain included; the repository retains documentation media.

These highlights cover changes since the `v0.9.0` tag.

## Avatars and locomotion

- Improved head/neck constraints distinguish head and neck bending from body
  translation. Pitching or rolling can translate the neck along X and Z
  without treating all tracked displacement as a bend.
- Added opt-in OpenXR hand tracking, finger-joint humanoid mappings, and
  retargeting to avatar hands.
- Added procedural lower-body walking IK with ground contacts, crouching,
  motion-driven NPC walking, and configurable `WalkingIK` presets. Airborne
  legs respond to movement direction rather than assuming local backward motion.
- Added optional XR flying through `.enable_flying(true)`: double-tapping jump
  within 250 ms toggles flight, with twice the normal movement speed.
- Added inheritable `Gravity.on()`, `Gravity.off()`, and `Gravity.auto()` modes
  with runtime changes. Flight controls gravity through the component rather
  than adding flight-specific behavior to the velocity integrator.
- Added climbing and improved desktop/XR avatar-control integration.

## Materials and rendering

- Materials v3 separates `MaterialBase` renderer capabilities,
  `MaterialDefinition` shader/input definitions, and `Material` instances.
  Built-in and custom materials share the material system, with typed inputs
  and instance overrides.
- Added planar reflection captures and extended reflective/refractive material
  examples, custom texture sampling, and animated material effects.
- Added cylinder, torus, helix, and helix-tube geometry.
- OpenXR can render directly into swapchain images. XR-only startup can omit
  the desktop window, and Winit is optional for library consumers.
- Added the native Android Vulkan/OpenXR scene renderer and MMS shell prototype.
  Initial custom opaque MMS scenes and head-driven views were verified on
  Focus 3. Android currently supports the custom opaque/albedo surface path;
  desktop material features do not imply Android support. See the
  [Android status and validation notes](../../android/README.md).

## Destruction, editor, and examples

- Added asynchronous mesh subtraction for destroyable surfaces, grouped
  destruction for combined geometry, and transient debris. The Sakura drone
  conversion demo includes destroyable floors and walls across its buildings.
- Retired empty damaged regions and batched surviving surface cells to reduce
  retained geometry and scene-management work after repeated destruction.
- Added editor BVH snapshots with explicit refresh and an optional traversal
  animation that highlights successive tree levels.
- Built-in editor panels support dragging and grabbing from their title bars.
  The Sakura demo exposes the settings panel and keeps weapons out of editor
  selection.
- Expanded the Sakura scene, weapon firing modes and effects, avatar examples,
  and Android renderer comparison/validation tools.

Destruction remains an initial implementation: detached structural collapse,
solid debris collision/resting, persistent debris pools, and general volume-based
remnant filtering are follow-up work. The BVH display is a bounded snapshot;
refresh it to inspect later geometry changes.

## Meow Meow Script

- Added type-expression support used by material input definitions.
- Component handles support live arbitrary fields, including function values,
  with native-field protection and lifetime cleanup.
- Extended custom event payloads and runtime host configuration.

## Upgrading

Use `mittens-engine = "0.10.0"` and, for standalone script hosts,
`meow-meow-script = "0.10.0"`. Update both direct dependencies together when
using both crates: Cargo treats 0.10 as a new compatibility boundary from 0.9.

Custom material authors should use the base/definition/instance API. See the
[material flow guide](../how_to/material-shader-flowchart.md),
[humanoid rigging guide](../how_to/rigging_and_controlling_humanoid_characters.md),
and [MMS documentation](../../crates/meow-meow-script/README.md).
Historical release notes and task documents retain their original version
references; unchecked task items are not release promises.

## Release validation

The workspace compiles with all targets, and all 84 MMS tests and 10 editor
visualization/transform integration tests pass. The full engine suite is not
green: it includes existing documentation/runtime regression failures, sandbox
network restrictions, and an audio-device test that stalls without working host
audio. This release does not claim a clean full engine test run or new headset QA.
