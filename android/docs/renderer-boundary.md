# Application-owned Android renderer

Source audit and public-API consumer check on 2026-10-29. This describes the
recommended next implementation, not an already implemented scene renderer.

## What is installed

The connected Focus 3 reported **Mittens Host 0.1.0**, package
`org.mittens.engine.dev`, updated at device time `2025-12-08 12:52:06`. Its
latest report identifies the successful Vulkan enable2 / classic render-pass
triangle probe. It has not created a `Universe`, loaded MMS, or drawn engine
geometry; its only shared engine call is the OpenXR loader initialization hook.
See [device results](focus-3-vulkan-results.md). The device clock differs from
the development machine's date.

## Crate ownership

Put the first mobile scene renderer in `android/src/renderer/`, alongside
the Android/OpenXR host. Keep shared engine scene preparation and contracts in
`mittens-engine`. The dependency direction remains:

```text
mitten-droid (android/)
  -> mittens-engine (repository root)
       -> meow-meow-script (crates/meow-meow-script/)
       -> mittens-query (crates/mittens-query/)
```

The engine must not depend on the Android application. It defines the common
data and upload contracts; the application implements them for its own types.

| Owner | Responsibilities |
| --- | --- |
| `mittens-engine` | ECS, retained MMS sessions, simulation, CPU assets, material identities/values, transforms, visual instances and shared ordered draw streams. |
| `mitten-droid` host | Android lifecycle/JVM, loader distribution, OpenXR instance/session/frame timing, tracking acquisition and runtime-owned image acquire/wait/release/submission. |
| `mitten-droid` renderer | Vulkan resources on that same runtime-selected device, mesh/texture upload, classic render passes/framebuffers/pipelines, per-eye GPU recording and completion. |
| Shared engine interfaces | Portable inputs and capability contracts where the application needs to supply data or implementations. No dependency on Android renderer types. |

Classic render passes are portable Vulkan functionality; their location here
is an implementation/ownership choice for this mobile renderer. The code can
still be exercised by desktop GPU validation tools, as the current probe is.

## Existing engine boundary: usable for the first scene

The following are already public:

- `Universe::load_mms_source_at_path` and `Universe::update`.
- `SystemWorld::prepare_render(..., &mut dyn RenderUploader, ...)`.
- `MeshUploader` and `TextureUploader`; an implementation of both also
  implements `RenderUploader` through the existing blanket implementation.
- CPU meshes and renderer-owned integer handles. Android can allocate its own
  GPU resources and return those handles; it need not expose Ash resources to ECS.
- `VisualWorld` accessors for instances, material values, lights/cameras and
  ordered phase streams (`RenderOp` / `DrawBatch`), including clip operations.

The [external consumer test](../../tests/external_renderer_consumer.rs) loads
two cubes through MMS, updates the real engine, and prepares the scene with an
application-defined uploader. It checks that geometry uploads once, both
instances use that uploader's handle, transforms/colors survive, and the engine
draw stream includes both cubes. It initializes no graphics device or XR
session. It passed with desktop features disabled:

```sh
cargo test --test external_renderer_consumer --no-default-features --locked
```

This validates the scene/upload interface; it does not prove an Android scene
renderer, material shader parity, or actual GPU uploads.

## Coupling that still matters

`Universe` owns a private concrete `VulkanoRenderer`; `Universe::render()` calls
it directly and passes it to `OpenXRSystem::render_xr`. Those convenience paths
cannot select an application renderer today. For the first scene, the host can
use the lower-level public preparation call instead. A later
`Universe::prepare_render_with(&mut dyn RenderUploader)` would be a convenience
wrapper, not a prerequisite for obtaining scene data. Do not call the built-in
renderer initialization/render methods on this host path.

OpenXR also needs a deliberate ownership boundary. The engine's current XR
system owns its own session/frame wait and publishes tracking and eye cameras
during `SystemWorld` updates. Enabling normal `XR`/`InputXR` components may
initialize that engine runtime; its update can invalidate or overwrite eye
camera/pose state when there is no engine-owned session. Supplying external
camera data once before `Universe::update` is therefore insufficient.

For the first static scene, omit engine XR/input-driver components and let
the Android host supply view/projection data to its renderer after the engine
update. For full engine HMD/controller/avatar integration, add an explicit
external XR frame-input path in the engine: predicted time, poses, validity,
and reference-space information must enter at the correct update phase.
Keep exactly one session/frame wait and one shared graphics device. Avoid
building a second independent XR session just to satisfy existing engine APIs.

### Consumer-defined systems and tracking order

`ecs::system::System` is a public trait, so a consuming crate can implement
it and call its `tick` from the application loop today. There is no public
`add_system`/`register_system` scheduler: `SystemWorld` stores concrete built-in
systems as fields and invokes them explicitly in its update function. Public
trait implementation alone does not insert a consumer's system into that order.

XR processing sits between camera/transform preparation and raycasting,
gestures and avatar consumers. The existing XR system also clears per-frame
pose validity and publishes eye cameras. Calling an external tracking system
only before or after the complete `Universe::update` does not provide the same
ordering or prevent those resets.

An `android_xr_system` implementation inside the engine is therefore not
required. Keep Android/OpenXR acquisition in `mitten-droid`; introduce a generic
external XR snapshot/provider input in the engine and consume it at the existing
XR update phase. Reuse the engine's component-driving, coordinate conversion,
validity and downstream input behavior. Arbitrary application systems could
later use generic phase hooks or a registration API, but that is a separate
scheduler extension rather than something already supplied by the `System` trait.

Sequence the work as scene graphics first, then full ECS tracking/input.
Rendering a world-anchored stereo scene already needs the host's predicted eye
poses, which the probe obtains; that is distinct from driving `InputXR`, hands,
controllers, gestures and avatars through the engine. The first scene can use
those host-provided eye views while the fuller XR input boundary is developed.

`default-features = false` currently disables Winit, not Vulkano. The engine
still compiles its built-in renderer/shaders, and `CpuVertex` derives Vulkano
traits despite exposing plain CPU data. Making the built-in renderer optional
and conditionally deriving those traits is a later dependency cleanup; it is
not necessary to start consuming the public scene API.

GPU mesh handles are cached inside `RenderAssets` for its uploader. Keep one
backend/resource lifetime per running engine instance, or explicitly rebuild
those caches when replacing/recreating the backend. Do not mix handle tables
from the existing renderer and the Android renderer.

## Shared OpenXR and rendering responsibilities

The existing engine window and XR paths are not independent renderers.
`VulkanoState::submit_xr_eye_to_view` and `render_visual_world` both call the
same `build_draw_batches_command_buffer`. They share the renderer's resources,
material/shader pipelines, deformation machinery and recording implementation.
The XR path supplies per-eye cameras and runtime-owned output views instead
of the window target. Some optional pass inputs differ, so sharing that
recorder does not imply identical effects in every output path. Both engine
paths currently require dynamic rendering.

`graphics/xr_renderer.rs` mainly supplies borrowed XR image views and raw
clear/completion helpers; it is not a second complete scene renderer. The
current Android Ash triangle renderer is a separate diagnostic implementation
and does not consume `VisualWorld` yet.

The existing `OpenXRSystem` combines several responsibilities that can be
separated without duplicating them by operating system:

| Responsibility | Common core | Platform or backend adapter |
| --- | --- | --- |
| Scene tracking | Rig/floor transforms, frame validity, head/controller component updates, eye cameras, button edges, hand-frame delivery. | Receives samples and device capabilities from the selected driver. |
| OpenXR protocol | Instance/system enumeration, session state machine, actions and interaction profiles, reference spaces, predicted poses, wait/begin/end frame, swapchain acquire/wait/release and projection submission. | Android/desktop bootstrap and the graphics-device binding supply prerequisites. |
| Platform lifecycle | Coordinate common session lifetime and recovery. | Android Activity/JVM/thread attachment, loader initialization, packaging/runtime discovery; desktop loader/runtime launch and application lifecycle. |
| Graphics integration | Target descriptions, GPU completion before XR image release, shared scene/material/draw preparation. | Device creation/import, actual image views, command recording and completion primitives for Vulkano/Ash and dynamic/classic rendering. |
| Optional device features | Capability negotiation and normalized results. | Enable and test advertised HTC/other extensions. Device features are not determined by the operating system alone. |

A useful eventual structure is a common engine `XrSceneSystem` plus a portable
OpenXR session/driver core, selected/configured by thin desktop and Android
hosts. The driver should not require `VulkanoRenderer` just to apply tracking or
manage an XR session. Platform systems registered by a consuming application
can wrap that shared core rather than copying the complete state machine.
Registration/phase integration still needs the engine extension described above.

Graphics backend and operating system are separate choices. Adding a classic
path to the shared Vulkano renderer could preserve more existing rendering
features than building a second full scene renderer in Ash. The small Android
renderer remains a reasonable first-scene route using the public upload/visual
API, but full desktop rendering parity there would require substantial GPU
implementation work beyond sharing `VisualWorld`. The module location for
the first renderer should not force duplication of portable XR or scene logic.

## First implementation slice

1. Preserve the successful triangle probe as a diagnostic build mode. Promote
   only its reusable device/session/resource plumbing into the host/renderer.
2. Add an Android renderer implementing the existing upload traits, initially
   for static opaque unlit geometry, model transforms, colors, and two eye
   cameras. Use the engine's prepared streams rather than walking the ECS again.
3. Load a tiny embedded MMS cube scene. After each host-owned XR frame sample,
   update the engine, prepare uploads, and render the resulting visual state
   into the acquired eye layers on the same Vulkan device.
4. Add depth, RGBA textures and a clear unsupported-material/capability policy.
   Preserve shared material meanings and shader interfaces; do not assume the
   desktop backend's private descriptor/buffer helpers can be imported unchanged.
5. Add external XR frame input before enabling the engine's HMD/controller
   drivers, then extend clipping, skinning, transparency and optional effects
   deliberately. The simple renderer is not full desktop feature parity.

Coding, APK builds, the consumer test, and desktop GPU validation can proceed
with USB disconnected. Real driver compatibility, presentation, and graphics
lifecycle changes still need a subsequent Focus 3 run.
