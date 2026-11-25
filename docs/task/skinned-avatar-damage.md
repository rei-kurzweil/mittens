# Partial destruction of skinned avatars at low health

Notes, 2026-11-25. Not started; needs a choice of approach.

## Idea

In `sakura-twilight-simpler` and `truck-bed-demo`, once an avatar (player or
NPC Mitten) is at 20% health or less, hits should visibly destroy pieces of
it during combat, as `Destroyable` does to crates, walls and the truck.
The hook is `practice_health` in `assets/components/target_practice.mms`:
`damage()` knows the remaining points and both scenes use it.

## What happens today

Attaching `Destroyable` to an avatar's renderables does nothing, silently.
`destroyable_imported.rs` (`ensure_source`) refuses any source mesh that has
`joints0`, `weights0` or morph targets, or more than 16,384 triangles. That
is deliberate: the imported path is "rigid imported geometry", and
`damageable-surfaces-csg-and-fragments.md` leaves skinned cutting as its own
slice. Lifting the guard is not an experiment worth running. The cut path
would:

- cut the bind-pose mesh with csgrs, which carries positions, normals and
  UVs but not joint indices, weights or morph deltas, so the cut mesh can
  no longer be skinned;
- swap in a mesh with a different vertex count under a renderable whose
  compute-cached deformation buffers were sized for the original;
- map the hit (a posed world point) into the mesh as if the mesh were rigid,
  so cuts land in the wrong place once the avatar is out of bind pose;
- run CSG on a 20k to 60k triangle body, which is slow even on the worker.

## Approaches

### A. Shader holes (recommended first slice)

Don't change the mesh. Keep a small list of hole spheres per avatar
(about 16), each stored **relative to the bone** nearest the hit: bone index,
offset in that bone's space, radius. The skinned vertex shader already has
each vertex's joints; it also passes the bind-pose position, and the fragment
stage discards inside any hole (in bind space, so a hole stays on the
forearm as it moves), with a burnt, glowing rim just outside it. Draw the
damaged body two-sided so the inside shows dark through a hole.

- Works with skinning, morphs, secondary motion and the outline pass
  (which needs the same discard).
- Cheap: a uniform array and a loop per fragment, only on damaged avatars.
- Hit mapping: take the hit point, find the nearest bone (capsule colliders
  already exist for spring bones and hit tests), and store the offset in that
  bone's space. Approximate but stable.
- No debris from the body itself. Spawn a few small fragments (cubes in the
  avatar's colour) at the hit, using the existing debris path.
- Holes clear on revive.
- Multiplayer: hole list as a small reliable event (bone, offset, radius).

### B. Skinned CSG

Cut in bind pose and carry skin data through the boolean: surviving vertices
keep their weights; new cut-face vertices take the weights of the nearest
original triangle (blending the top four joints). The deformation system
rebuilds the avatar's buffers on a vertex-count change. The removed piece
becomes rigid debris posed at the moment of the cut. Needs csgrs attribute
passthrough (or a remap afterwards by nearest triangle), the hit mapping
from A, and a triangle budget (cut a low-poly proxy per body part, or only
small regions). Real holes with real interiors, but a large slice.

### C. Pre-split parts

Author or generate per-limb pieces (by dominant bone) and hide, detach or
swap a piece for a damaged variant when its region takes enough damage.
It reads as dismemberment more than erosion, and needs asset work per
avatar.

## Recommendation

A first: it answers "can a skinned avatar be partially destroyed in combat"
in a day or two and costs almost nothing when unused. B can follow for one
avatar if real cut geometry is wanted.

## Validation for A

- Holes stay attached to the right limb through walking, IK reach and
  secondary motion.
- Only damaged avatars pay for the discard path.
- Revive clears holes; the 20% threshold applies to both scenes.
- No change to the 16-bit / 32-bit skinning variants on Android, or a note on
  which shader variants carry the discard.
