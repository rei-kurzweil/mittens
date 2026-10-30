# First custom opaque renderer

Implementation work after checkpoint `09f833c0` on 2026-10-29. The checkpoint
preserves the Focus 3 Vulkan triangle, its device evidence, and the public
engine/consumer boundary test. The **new scene APK is not yet verified on the
Focus 3**. The previously installed/visually verified app is the triangle probe.

The default APK now loads two embedded MMS cubes and a generic custom fragment
definition with named `vec3 tint` / `f32 gain` inputs. `MaterialBase()` supplies
an opaque/albedo surface. The backend accepts custom definition metadata rather
than switching on built-in/legacy material handles. The demo fragment is an
example material, not a hardcoded renderer pipeline.

## Data and ownership

`xr_host.rs` owns the worker/JVM/lifecycle reporting. `graphics_device.rs`
creates the runtime-selected Vulkan device. `xr_session.rs` owns the session,
predicted eye views and image acquire/wait/release/end-frame sequence.
`frame_renderer.rs` selects either the retained triangle diagnostic or the
new `SceneRuntime`. There is exactly one XR session and one graphics device.

`scene_runtime.rs` owns a `Universe` and the new renderer. It loads embedded MMS
with an app-private fragment source file, updates the engine, calls public
`SystemWorld::prepare_render` with the application uploader, and prepares the
shared draw cache. The scene omits `XR`/`InputXR` components. Host-located poses
become renderer eye cameras after update; full engine tracking/input integration
still needs the external XR frame-input boundary in [the audit](renderer-boundary.md).

`renderer/` separates allocations/transfers, eye attachments/pass, pipeline
state, retained material resources, view conversion, and frame recording. Every
new/refactored source file is below 1,400 lines. The engine still compiles
Vulkano with desktop features disabled; this work does not remove that dependency.

## Tile-oriented attachment policy

- One classic render pass per eye, all supported opaque batches inside it.
- Direct rendering into the acquired runtime-owned color layer: `CLEAR/STORE`.
- Reusable per-eye depth images: `CLEAR/DONT_CARE`, transient attachment usage,
  preferred lazily allocated device-local memory, ordinary device-local fallback.
- One sample; no intermediate color image, post-process copy, or sampled depth.
- Depth uses supported `D16_UNORM`, falling back to `D32_SFLOAT`.
- Full eye render area and attachment clears; no explicit per-pixel clear draw.

Hardware/driver choose tiles. These policies allow tile-local work; they do not
guarantee that the driver uses a particular path or prove bandwidth savings.
See [Khronos attachment guidance](https://docs.vulkan.org/samples/latest/samples/performance/render_passes/README.html)
and [tiler guidance](https://docs.vulkan.org/guide/latest/tile_based_rendering_best_practices.html).
Classic passes are required by the measured Focus 3 firmware's lack of dynamic
rendering, not by the fact that its GPU is a tiler.

## Supported slice and limits

Static indexed triangle meshes, model transforms, per-instance colors,
per-eye view/projection, retained generic scalar/vector material parameters,
and RGBA8 sRGB textures with linear/nearest/nearest-magnification filtering.
An untextured material uses a persistent white texture. Pipeline descriptors
preserve the engine custom fragment interface: camera at set 0 binding 0,
numeric inputs at set 1 binding 0, base texture at set 1 binding 1. Reflected
descriptor metadata makes requests for unsupported light resources explicit.

The renderer rejects legacy materials, other bases, non-opaque geometry,
cutouts, skin/morph data, clips, outlines, extra phases, post-processing,
mirrors and BC7 uploads. Standard transmission values alone do not activate
transmission; material capabilities define that behavior. Light resources are
not bound by this albedo-only renderer. Features must be added deliberately.

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

The desktop GPU smoke harness compiles the **production** backend and app scene
code. On NVIDIA GTX 1080 it checks stereo parallax, near/far depth occlusion,
named uniform revision updates, RGBA sampling, batched instances, stable retained
resource counts across repeated frames, unsupported transparency/legacy rejection,
and actual embedded MMS startup. It also retains classic/dynamic triangle checks.
The host lacks lazy device memory, so lazy allocation is not verified there.

See the [development validation record](validation/opaque-renderer-development-2026-10-29.txt)
and [raw GPU output](validation/opaque-renderer-headless-2026-10-29.log).

The [desktop OpenXR A/B demo](../../examples/xr-renderer-compare/README.md)
selects this backend or the current Vulkano renderer through one shared frame
loop. It uses the same custom scene, recommended eye size and 1x sampling.
The Android demo remains capped at 1024px per eye. Performance comparisons must
use release builds and verify native cadence in the runtime, not infer it from
application timing alone.

Both release backends created 1892x1892 stereo sessions through SteamVR/OpenXR
2.17.10 on the GTX 1080, reached SYNCHRONIZED and stopped cleanly after a short
startup timeout. Neither became VISIBLE/FOCUSED, so no rendered performance
samples or 75/90Hz result were obtained. This is initialization/stop evidence,
not presentation or a benchmark.

Build the new scene with `android/packaging/build-apk.sh`. For the preserved
triangle use `--no-default-features --features vulkan-probe`; for lifecycle-only
use `--no-default-features`. Then install on the Focus 3, confirm both cubes and
world-anchored parallax, inspect attachment and `custom scene GPU complete`
logs, and explicitly test background/resume/stop/relaunch. Do not enable engine
controller/HMD drivers until the external tracking integration is implemented.
