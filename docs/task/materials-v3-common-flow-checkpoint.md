# Materials v3: common source and value flow

Date: 2026-10-25
Status: implemented and regression-tested; headset acceptance pending.

The later [capture/capability cleanup](materials-v3-capture-capability-cleanup.md)
adds runtime image reuse/retirement and a resolved renderer capability contract.
The [built-in authoring API](../draft/material-builtin-authoring-api.md) is proposed
for review; Shading remains unchanged. Basic custom alpha blending and planar
reflection are already implemented in the pool checkpoint.

MaterialSystem now resolves and registers both explicit Material components and
the Shading.anime()/toon() compatibility adapters. Definitions own named typed
schemas and generic defaults; all instances use the same retained float-value
store and revision updater. Generic records no longer contain AnimeShadingParams.
Logical field slots are separate from GPU packing: the built-in UBO adapter
packs fields by name, while custom GLSL reflection supplies physical offsets.
The registry now uses MaterialBase/MaterialBaseId and base references throughout.

RenderableSystem retains resolved material IDs and selects static/deformed
programs from their definitions. MaterialSystem owns retirement as well as
registration; compatibility Shading intents publish through it. Detached authored
Shading sources queue initialization just like explicit custom material resources.
Scalar edits do not refresh draw topology; definition changes refresh consumers.
Fixed legacy transmission/unlit/emissive phase behavior remains compatible.

CombineMesh uses the common resolver and retains the first source's same
material instance for built-in and custom materials. Its declaration survives
source collapse. Scalar edits and Anime/Toon model changes keep the combined
mesh and instance identity. Mixed source materials still use first-source-wins;
this slice does not introduce validation or multiple output material groups.

At the user's request, the temporary lighting debug input, shader branches,
builder/live methods and youtuber-room mode-selection panel are removed. Anime
keeps facing-gated distance lighting and its rim. The room keeps Rei(mu), her
pose/secondary motion/ambient eyes, LEDs/Bloom, furniture and avatar-free XR.
The resized straw remains unscaled; its obsolete 28 m recenter offset is replaced
with 0.429 m based on the current asset. The user is checking perceived size in VR.

Validation:

- Named-schema packing is checked with fields reordered relative to the GPU block.
- Focused registration, source sharing, 10,000 edits, detach/reattach, GLTF
  projections, local overrides, transmission/unlit boundaries, CombineMesh
  collapse/model changes, retirement, host and scene tests pass.
- Five headless Vulkan tests pass: bounded built-in snapshots, independent
  stereo instances, bounded custom snapshots, actual room LED rendering and
  extraction, and shared static/skinned Anime shading with the seated NPC.
- The broader ECS-system run has 571 passing, 27 failing and 3 ignored tests.
  The same 27 failures reproduce at commit e38f8012 (568 passing, 27 failing,
  3 ignored); none are new in this slice. They include editor/selection,
  asset-label, layout, IK, raycastability, HTTP and transition fixtures.
- Formatting and diff whitespace checks pass.

Next work remains custom skinned/GLTF support, broader renderer feature/state
capability migration, public built-in MaterialDefinition conveniences,
reference-preserving serialization, richer custom input types and transmission.
MMS custom GPU fields remain f32-only; internal Vec3 schema fields serve existing
built-in inputs and do not enable custom vector uploads.
