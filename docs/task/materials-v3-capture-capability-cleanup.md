# Materials V3: capture lifetime and renderer capabilities

Date: 2026-10-25
Status: implemented for the current renderer contracts; public API review pending.

## Capture storage and ownership

Runtime publications reuse an existing image when extent, format and transfer
usage match. All capture writes and surface reads use the renderer-wide ordered
submission chain, so an earlier read finishes before that storage is overwritten.
Resizing allocates a replacement; retained command buffers/descriptors still own
the old image through their Arc references. This avoids per-publication image
allocation and repeated descriptor replacement during steady-state captures.

RenderToTextureSystem reconciles authored consumer registrations against the
world. Published handles live while an explicit consumer or a live mirror
producer owns the selector. A producer's capture prefix is retained separately
from its current requests: pause or tracking loss does not destroy frozen images
or allocate another set when tracking returns.

Removing the last owner queues texture retirement. The renderer drops cached
images, pending publications, descriptor bindings/snapshots and unused mirror
offscreen targets. Dropping cache ownership does not invalidate in-flight GPU
work. Renderer-side managed handles also retire when a replacement VisualWorld
no longer owns them. Asset textures are outside this runtime-image policy.

Capture ownership is per reflective surface, even when surfaces share a material
instance: each surface can define a different reflection plane. The mirror system
rebuilds requests from active renderable bindings during updates; retirement is
not a material-instance reference-count decrement inside renderable removal.
Detached material instances do not retain captures by themselves.

## Capability resolution

MaterialDefinition now owns an explicit capability contract for transmission,
emission source policy and supported state profile. Its cached-deformation
program is optional: custom static definitions no longer pretend that their
static program also supports skinned geometry.

MaterialRegistry resolves those capabilities together with the selected geometry
program and the base's planar-capture requirement. Draw-phase/resource detection,
emission eligibility, outline deformation selection and mirror requests consume
that resolved result. The legacy Toon intensity adapter still selects its
emissive variant; custom extracted radiance keeps the current opaque, unclipped
foreground eligibility rules.

MaterialEmission::ShaderProvided names the custom extraction path: the fragment
shader supplies emitted color and brightness, including values above 1.0. The
ToonIntensity variant preserves the built-in intensity-driven extraction path.

Legacy built-in program identities map to capabilities at one compatibility
boundary. Concrete shader/pipeline dispatch and built-in UBO packing still need
their program identities; they are not additional authoring APIs. Existing
transmission, Unlit, Mirror and Shading components remain compatibility authoring.
Unsupported custom cached-deformation bindings receive a diagnostic and keep the
compatible geometry path rather than binding static custom parameters to it.

This does not implement custom skinned shaders, new cutout/clipped/background
variants, generic authored render state, or a new transmissive material base.

## Validation

- Release build and formatting checks pass.
- Runtime consumer sharing/last-reference retirement and frozen capture ownership.
- Existing mirror lifecycle, clipping, quality and stereo-view tests.
- Definition-driven feature resolution, built-in emission activation and optional
  geometry program selection; material/scene and draw-cache checks.
- Existing CombineMesh identity/update tests.
- Vulkan runtime image reuse across 1,000 publications, resize replacement and
  retirement while an old view remains retained.
- Vulkan bounded descriptor revisions, real stereo room rendering, per-eye water
  capture bindings and front/back reflection landmark checks.

The broad material filter also selects previously documented legacy failures
(unsupported legacy Animation evaluation and undefined is_string). Its microphone
fixture stalled after device errors, so that broad run was stopped and is not
reported as passing. Focused checks above are the validation gate for this slice.

The next review is [built-in material authoring](../draft/material-builtin-authoring-api.md).
Shading's public API has not changed. No replacement constructors are implemented.
