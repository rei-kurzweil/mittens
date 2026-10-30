# First custom opaque renderer

Implementation work after checkpoint `09f833c0` on 2026-10-29. The checkpoint
preserves the Focus 3 Vulkan triangle, its device evidence, and the public
engine/consumer boundary test. The first **1x MMS scene passed on Focus 3** on
2026-10-29: the user confirmed the cubes and physical headset translation and
orientation. The report records completed custom scene draws in both eyes.
The follow-up default adds 4x MSAA; see the device validation record for its
separate install/visual status. Controlled graphics lifecycle tests remain open.
See the [Focus 3 scene record](focus-3-opaque-results.md).

The default APK now loads the [lean Sakura rooftop scene](../examples/sakura-stage-lean.mms),
with the studio stage, Sakura sky and sun, tiled tower, pistol table, three
static pistol models, and eight simple neighboring building cubes. It uses custom tile, matcap and anime-style surface
definitions with ambient, directional and point lights. The old two-cube scene
remains available with `--no-default-features --features cube-demo`.

The sky uses the Sakura scene's gradient stops and sun direction/radius in a
single custom background shader. Its white, antialiased sun disc has a warm halo; bloom and
emission extraction are not implemented. Background culling is disabled for
the inside of the sky sphere; the cube's -Z face winding has been corrected.
GPU readback checks the gradient, sun and cube faces from opposite directions.

`MaterialBase()` supplies an albedo surface. The backend accepts custom
definition metadata rather than switching on built-in/legacy material handles.
The custom anime-style surface follows the existing shade/rim idea and adds
colored lighting; the built-in `MaterialDefinition.anime()` remains unsupported.

The stage APK was installed and launched on Focus 3 on 2026-10-29. Its
[startup report](validation/focus-3-sakura-startup-2026-10-29.txt) shows
OpenXR focused and 4x lazy attachments created. The user subsequently confirmed
the stage, sky, sun and corrected cube faces in the headset. [Desktop GPU checks](validation/sakura-stage-headless-2026-10-29.txt)
rendered the scene at 1x and 4x and verified sky pixels and visible changes when
ambient and direct lights change. These checks are not a performance benchmark.

## Data and ownership

`xr_host.rs` owns the worker/JVM/lifecycle reporting. `graphics_device.rs`
creates the runtime-selected Vulkan device. `xr_session.rs` owns the session,
predicted eye views and image acquire/wait/release/end-frame sequence.
`frame_renderer.rs` selects either the retained triangle diagnostic or the
new `SceneRuntime`. There is exactly one XR session and one graphics device.

`scene_runtime.rs` owns a `Universe` and the new renderer. It loads embedded MMS
with an app-private fragment source file, updates the engine, calls public
`SystemWorld::prepare_render` with the application uploader, and prepares the
shared draw cache. The host passes one predicted-time snapshot of views,
controller aim/grip poses and action state to the engine's XR update phase.
The engine's `InputXR`, `XRHand`, pointer and gamepad systems consume that frame;
they do not create another XR session. The current host binds the Focus 3
controller profile. Articulated hand tracking remains unimplemented.

The lean profiling scene uses left-stick movement without gravity, the player
capsule, editor UI, or active static colliders. `--features profile-collision`
enables the avatar-free capsule and static collision without the editor UI; the
preserved UI-and-collision variant can be built with `--features profile-ui`.
The latter two variants have an avatar-free
fixed-height slide capsule, gravity and velocity; B jumps when grounded. A synthetic
engine test covers standing, stick movement, a wall, jumping, landing and tracking
loss. The capsule follows the locomotion root, not physical room-scale head
translation or crouching. Its settings-only `EditorUI` and empty `Editor` root
are separate from scene geometry. The pistol models are static and not yet
grabbable or functional in either variant.

`renderer/` separates allocations/transfers, eye attachments/pass, pipeline
state, retained material resources, view conversion, and frame recording. Every
new/refactored source file is below 1,400 lines. The engine still compiles
Vulkano with desktop features disabled; this work does not remove that dependency.

## Tile-oriented attachment policy

- One classic render pass per eye, all supported opaque batches inside it.
- The Android scene defaults to 4x MSAA. Its color and depth attachments are
  transient and `CLEAR/DONT_CARE`, with lazy device-local memory preferred.
- An in-subpass color resolve writes directly into the acquired single-sample
  OpenXR color layer: `DONT_CARE/STORE`. No separate resolve command or copy.
- The 1x path draws directly into that layer with `CLEAR/STORE`; the desktop
  comparison retains this explicit 1x baseline for both renderers.
- Reusable per-eye depth images: `CLEAR/DONT_CARE`, transient attachment usage,
  preferred lazily allocated device-local memory, ordinary device-local fallback.
- No sampled depth, post-process copy, or persistent multisample color image.
- Depth uses supported `D16_UNORM`, falling back to `D32_SFLOAT`.
- Full eye render area and attachment clears; no explicit per-pixel clear draw.

Sample counts are checked against the actual color/depth format and transient
usage before allocation. The pipeline uses the same sample count as the pass.
Depth is discarded without a depth resolve. This follows the
[Khronos MSAA guidance](https://docs.vulkan.org/samples/latest/samples/performance/msaa/README.html).

Hardware/driver choose tiles. These policies allow tile-local work; they do not
guarantee that the driver uses a particular path or prove bandwidth savings.
See [Khronos attachment guidance](https://docs.vulkan.org/samples/latest/samples/performance/render_passes/README.html)
and [tiler guidance](https://docs.vulkan.org/guide/latest/tile_based_rendering_best_practices.html).
Classic passes are required by the measured Focus 3 firmware's lack of dynamic
rendering, not by the fact that its GPU is a tiler.

## Supported slice and limits

Static indexed triangle meshes, model transforms, per-instance colors,
per-eye view/projection, retained generic scalar/vector material parameters,
and linear RGBA8 textures with linear/nearest/nearest-magnification filtering.
An untextured material uses a persistent white texture. Pipeline descriptors
preserve the engine custom fragment interface: camera at set 0 binding 0,
numeric inputs at set 1 binding 0, base texture at set 1 binding 1. Reflected
descriptor metadata validates the supported interface. A shared retained buffer
at set 0 binding 1 supplies up to 64 engine lights using the desktop shader ABI;
ambient light is in the camera block. Plain sky backgrounds render first inside
the same eye pass, with translation removed and depth writes disabled.

The renderer supports single-layer alpha-blended custom surfaces in the same
render pass, used by the settings UI. It rejects legacy materials, other bases,
multilayer transparency, cutouts, skin/morph data, clips, outlines, extra phases, post-processing,
mirrors and BC7 uploads. Standard transmission values alone do not activate
transmission; material capabilities define that behavior. Directional, point
and spot light records are passed to custom shaders, which decide how to shade;
shadow maps are not implemented. Features must be added deliberately.

Mesh/texture handles and pipelines persist for the renderer's lifetime. Material
UBOs/descriptors persist by retained instance and texture/filter variant; only
changed parameter revisions repack uniforms. Retired instances free descriptor
sets. The initial pool has an explicit capacity of 256 material bindings and
256 custom pipelines, with errors rather than silent fallbacks. CPU/GPU instance
buffers grow by capacity, are reused, and preserve engine draw batching: one
indexed instanced draw per prepared batch per eye. There are no per-instance
draw-loop allocations or per-frame pipeline recreation.

The bring-up path waits for GPU fence completion before releasing the XR image.
It is intentionally conservative. Textures use a one-time staging transfer;
mesh and uniform allocations require host-visible memory. Memory placement and
frame pipelining are later performance work, especially on discrete desktop GPUs.

## Texture mipmaps

The Android renderer and the existing Vulkano desktop/XR renderer generate a
full mip chain for uploaded RGBA8 material textures, including the rooftop tile.
The shared CPU filter includes odd-sized edges and averages sRGB color in linear
light when a texture is marked sRGB. Both current engine RGBA8 upload paths use
linear UNORM, preserving their existing interpretation. Linear filtering blends
between mip levels; nearest filtering selects one level. The nearest-magnification
mode keeps sharp magnification and blends levels when minifying.

Generation and upload happen once per texture, with temporary CPU/staging memory
and load-time cost. A full chain for a square texture uses approximately one
third more texture memory than its base level. Mips can reduce distant shimmer
and improve texture locality, but extra filtering and memory mean a performance
gain is not guaranteed. Filtered normal maps are normalized by shaders that use
them; alpha-coverage preservation is not implemented. Desktop BC7 compressed
uploads still contain only the base level because that upload interface does not
yet supply a compressed mip chain. At 1024×1024 per eye the user reported much
less texture flicker in the headset. Headset performance measurements remain
outstanding.

## Intended useful headset feature set

The agreed target is custom opaque materials with roughly the complexity of
the anime material, skinning, morph targets, and mirrors at **at least 90Hz** on
the headset. Bloom and transmission are outside this initial feature set.
Mirrors are the planned high-end reflection feature; arbitrary reflective or
transmissive material bases are not prerequisites for the first renderer.
90Hz is a performance target, not a result already demonstrated by this scene.

Optional later bloom work is tracked separately in
[fast bloom research for desktop and XR2-class GPUs](../../docs/task/bloom-performance-desktop-and-mobile.md).
It does not expand the current opaque slice or make bloom a prerequisite for
the animated/anime/mirror feature set.

Build toward it in measured slices: validate this opaque scene first, add
skinning/morph deformation and representative animated geometry, add the light
and normal inputs needed by an anime-complexity **custom** shader, then mirror
captures and composition. Preserve the generic material definition/instance
path throughout; do not add legacy Anime/Toon pipeline handle switches.

Deformation should produce reusable per-frame geometry for both eyes and mirror
views. Mirror captures need images that survive their capture pass for later
sampling, plus an explicit resolution, view-count/stereo, scene-inclusion and
update-rate policy. Those persistent color images are distinct from transient
depth; store only the outputs actually consumed. Measure the full animated,
mirrored scene at headset resolution and native cadence, with no assumption
that passing the tiny static desktop comparison establishes the 90Hz target.

### Compute deformation cache

Use the current cache design: `graphics/deformation.rs` supplies public jobs,
buffer layouts, stable output ranges and workgroup records, and
`assets/shaders/mesh-deformation.comp` applies sparse morph deltas followed by
skinning. It writes a 16-byte position/packed-normal vertex record. Each
independently posed instance needs its own output range; sharing a source mesh
does not mean its deformed output can always be shared.

The application backend should reuse these contracts and shader, with a focused
compute resource/recording module. Extract reusable scheduling from the current
large Vulkano renderer when needed rather than copying the whole renderer or
growing it further. Keep GPU output persistent and dispatch dirty deformation
jobs at most once for a scene state, before its eye/mirror draws. Reuse cached
output when bone/morph inputs are unchanged. Animated inputs can dirty it every
frame; caching saves repeated per-view deformation, not the animation itself.

The existing cached vertex shader reads output as a storage buffer. Its key
dependency is compute shader writes becoming visible to vertex shader reads;
later writes must also wait until prior view reads finish. Start on the graphics
queue with an explicit dependency and the existing conservative frame completion,
then consider buffering/pipelining from measurements. Do not add a CPU readback
or assume compute output is transient tile memory. Validate the 8 compute storage
bindings and 64-invocation workgroups and run the actual kernel on Focus 3; the
earlier probe checked basic limits, not this kernel's execution.

The current new renderer still rejects skin/morph meshes. This is the planned
animated-mesh slice, not functionality already enabled in its opaque APK.

## Verification and next device run

The instrumented scene reports `FRAME_PROFILE` after 90 warmup frames, then
every 120 rendered frames. It records the predicted presentation interval,
CPU scene update, asset preparation, renderer preparation and command recording,
CPU submit/fence-wait time, and GPU timestamps spanning the two eye passes.
GPU timestamps can be unavailable if the queue lacks timestamp support. Fence
wait includes queue scheduling and submission; it is not itself a GPU timer.
These are debug builds unless `--release` is passed to the build script.

On 2026-10-30, while the user wore Focus 3 at 1712×1712 per eye and 4x MSAA,
the [lean variant](validation/focus-3-lean-profile-2026-10-30.txt) measured
approximately 3.4–4.0 ms in engine update and 9–15 ms on the GPU for both eyes.
The [UI-and-collision variant](validation/focus-3-ui-collision-profile-2026-10-30.txt)
measured approximately 9.6–10.9 ms in engine update and 11–15 ms on the GPU.
The [collision-only variant](validation/focus-3-collision-profile-debug-2026-10-30.txt)
measured approximately 5.7–7.0 ms in engine update and 9.7–15 ms on the GPU.
These runs used debug builds and were not view- or motion-matched, so the
differences are suggestive rather than controlled attribution. The current host waits for
GPU completion before releasing the image, so CPU work and GPU completion are
largely serialized. Predicted intervals do not establish displayed frame rate
or compositor reprojection behavior.

The desktop GPU smoke harness compiles the **production** backend and app scene
code. On NVIDIA GTX 1080 it checks stereo parallax, near/far depth occlusion,
named uniform revision updates, RGBA sampling, batched instances, stable retained
resource counts across repeated frames, single-layer blending and legacy rejection,
and actual embedded MMS startup. It also retains classic/dynamic triangle checks.
It now exercises both 1x and 4x with single-sample output readback: the flat-color
test found 0 partially covered pixels at 1x and 281 at 4x. These are coverage
checks, not timing measurements. The desktop lacks lazy device memory; the
Focus 3 1x run selected lazy depth successfully.

See the [development validation record](validation/opaque-renderer-development-2026-10-29.txt)
and [raw GPU output](validation/opaque-renderer-headless-2026-10-29.log).

The [desktop OpenXR A/B demo](../../examples/xr-renderer-compare/README.md)
selects this backend or the current Vulkano renderer through one shared frame
loop. It uses the same custom scene, recommended eye size and 1x sampling.
The Sakura demo uses the runtime's recommended per-eye resolution (1712×1712
on the tested Focus 3). An explicit request can be made through
`RendererSettings { desired_xr_resolution([1712, 1712]) }`. Android reads this
once before creating the XR swapchain and clamps the request to the runtime's
reported limits. With no request, the mobile scene uses the runtime recommendation;
changing it after session creation does not resize the existing swapchain.
The desktop OpenXR path does not yet use this setting. Performance comparisons
must use release builds and verify native cadence in the runtime, not infer it
from application timing alone.

On the tested Focus 3, the runtime reported a 1712×1712 system swapchain maximum
and a 3440×3440 maximum view rectangle. An opt-in diagnostic
(`android/packaging/build-apk.sh --features resolution-probe`) nevertheless
created a 2448×2448 two-layer, single-sample RGBA8 sRGB swapchain and enumerated
three images. A separate `resolution-render-probe` build rendered the Sakura
stage at 2448×2448 with 4x MSAA; the user confirmed it appeared but could not
tell whether it looked sharper. This experiment exceeds the reported system
limit and does not establish performance or portable behavior. The default
scene remains at the runtime recommendation.

Both release backends created 1892x1892 stereo sessions through SteamVR/OpenXR
2.17.10 on the GTX 1080, reached SYNCHRONIZED and stopped cleanly after a short
startup timeout. Neither became VISIBLE/FOCUSED, so no rendered performance
samples or 75/90Hz result were obtained. This is initialization/stop evidence,
not presentation or a benchmark.

Build the Sakura stage with `android/packaging/build-apk.sh`. For the preserved
triangle use `--no-default-features --features vulkan-probe`; for lifecycle-only
use `--no-default-features`. Install on the Focus 3, confirm the stage and
world-anchored parallax, inspect attachment and `custom scene GPU complete`
logs, and explicitly test background/resume/stop/relaunch. The current controller,
settings-panel and capsule prototype still needs headset interaction validation.
