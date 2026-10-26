# Task: Damageable surfaces, CSG cuts and fragments

Date: 2026-10-26
Status: proposed; documentation only. Do not start implementation yet.

## Goal

Repeated damage at the same location should eventually break a piece out of an
opted-in surface. Keep a coarse damage map per affected primitive, use the
damaging volume to cut a real hole once the local damage threshold is crossed,
and split the removed material into roughly two or three fragments. Each
fragment gets its own Transform, initial velocity and gravity.

The first motivating source is a machine-pistol impact/explosion in the Sakura
scene. Its emissive impact sphere is currently only a visual effect; it does not
apply damage or change geometry.

## Existing support and prerequisites

The engine has ray intersection events with the source raycaster, hit primitive,
world hit point, ray origin and direction. The gun's three shot raycasters are
distinct from the hand pointer's raycaster. Velocity, gravity and collision
building blocks also exist, but complete rigid-body debris behavior, angular
motion and fragment contact behavior are not established by this proposal.

CSG currently has a [roadmap](mesh-csg-operations.md), a
[first-slice design](csg-system-first-slice.md), and an authored syntax example.
There is no implemented CsgSystem, registered CSG/Subtraction component or
csgrs dependency in the current engine. The existing CSG example's parser test
does not demonstrate working mesh booleans.

The planned first slice supports subtraction of static, closed triangle meshes.
This damage feature additionally needs the removed intersection volume, fragment
partitioning, repeated runtime cuts and publication of replacement geometry.
Do not assume that completing the first subtraction slice provides those pieces.

## Opt-in and ownership

Introduce a proposed `Damageable` component that opts one or more renderable
primitives into this behavior. Its final MMS API is undecided. Decide whether a
marker covers its owner's subtree, an explicit selection, or both; imported
multi-primitive models need an unambiguous selection rule.

Each selected primitive owns independent runtime damage state: map layout,
accumulated values, threshold policy and geometry revision. An enclosing marker
may supply shared settings, while two different primitives keep separate maps.
Two renderables sharing a source mesh must not damage one another. Derived meshes
must be specific to the damaged instance rather than overwriting a shared asset.

Keep the authored source and opt-in settings separate from runtime damage maps,
replacement mesh handles and spawned debris. Persistence of accumulated damage
is a later policy decision.

## Damage maps

For a known procedural primitive, use its base-mesh identity and face layout to
create low-resolution maps for its logical faces or polygon regions. For example,
a box has six face maps; its two triangles per side can map to one logical face.
Curved primitives need a deliberate surface partition rather than treating every
shape as a six-sided box. Record sufficient provenance to associate hit geometry
with the original face layout.

For an imported or otherwise unknown mesh, use the bounding box of each selected
primitive as the fallback indexing surface. The six sides have coarse maps. A
hit selects a side and a position within that side's grid, so repeated nearby
hits accumulate in the same cells. The bounding box is an addressing proxy:
the eventual cut must affect the actual mesh, not replace it with box geometry.

Use stable primitive-local coordinates. Transform the world hit point and damage
footprint into the target's space; moving, rotating or scaling an object must not
slide its existing damage around. Define handling for nonuniform scale, collapsed
bounds, box-edge ties and hits away from the proxy box's boundary. Choosing a box
side needs a consistent ray/normal rule, not an assumption that the hit lies on
one of the box faces.

Accumulate damage over the cells covered by the source's footprint, not only its
centre point. Resolution, per-hit amount, falloff and threshold are configurable
policies to settle later. The threshold crossing is local to the damaged region;
it is not just one global health counter for the whole primitive.

## Damage source and cut volume

A damage event should identify the source/device, target primitive, hit location,
direction and damage amount, together with the damage volume and its transform.
The source raycaster distinguishes a gun shot from ordinary pointer interaction.

Use the damaging object's bounding box as the initial cutter volume, positioned
at the impact. Capture its dimensions and pose for that damage event. The visual
mini-explosion expands and fades; that animation must not silently resize the
damage volume or apply the same hit again every frame. A visible explosion's
size and its gameplay damage footprint can be authored separately.

Convert the cutter and current target geometry into a common calculation space.
Define whether the cutter follows its oriented source box or a world-aligned
bounding box. Ensure it overlaps the intended surface depth; the hole depth
comes from the cutter volume rather than from the damage map resolution.

## Threshold crossing and fragments

When accumulated damage crosses the local threshold:

1. Resolve the current target geometry and a snapshot of the cutter volume.
2. Compute the surviving geometry as target minus cutter.
3. Compute the removed material as target intersect cutter. Using the cutter
   itself as debris would invent material that was never part of the target.
4. Partition the removed volume into approximately two or three valid pieces.
   Existing disconnected pieces may already provide useful fragments. Otherwise
   this requires geometric slicing, not just triangle subdivision on the surface.
5. Publish the remaining surface as the damaged primitive's new visual and
   collision geometry, preserving its authored placement and logical identity.
6. Spawn each fragment in a separate Transform at its original world position,
   with an appropriate material, collision shape, outward initial velocity and
   gravity. Later work can add spin, bounce and more complete rigid-body behavior.
7. Update the damage state and geometry revision so the same threshold crossing
   cannot repeatedly detach the same material.

Later hits operate on the surviving mesh. Areas already removed must not produce
new fragments from empty space. Decide whether fragments themselves inherit
Damageable; do not enable recursive fragmentation implicitly.

## Geometry and lifecycle questions

- **Open surfaces:** planes and non-watertight imported meshes do not define a
  removable solid. Establish thickness/capping rules or reject these operands.
- **Skinned humanoids and morphs:** bounding-box maps do not solve cutting a
  deformed mesh. Decide how a hit maps back to a stable surface, whether cuts
  operate on a posed snapshot or retained mesh, and how surviving vertices and
  new cut faces retain skin weights/morph behavior. Static GLTF primitives are
  a separate, earlier scope; live skinned cutting needs its own slice.
- **Materials and UVs:** retain exterior materials and attributes where possible,
  and choose material/UV generation for exposed interior faces. The existing CSG
  first-slice design uses placeholder UVs and does not promise texture preservation.
- **CombineMesh:** a combined output may lose source-primitive/face identity.
  Preserve hit provenance, make the combined mesh the explicit damage target, or
  keep damageable geometry outside the combine. Sakura's combined buildings make
  this an explicit integration decision.
- **Publication:** update rendering, bounds, raycasts and collision together.
  A visible hole must also allow later rays and moving bodies through it. Preserve
  the previous mesh if a boolean fails; publish fragments only for a successful cut.
- **Concurrent hits:** serialize or version work per primitive. A slow cut must
  not overwrite a newer revision or publish after the target has been removed.
- **Resource budgets:** cap map memory, queued cuts, mesh complexity and debris
  count/lifetime. Retire obsolete meshes, colliders and fragment resources.

## Suggested slices when implementation is authorized

1. Establish the CSG backend and static subtraction contract already documented.
2. Add Damageable selection and local damage accumulation, with a way to inspect
   the maps, before replacing geometry.
3. Make one box lose a piece at a repeated-hit threshold; obtain both remainder
   and removed volume and update visual/raycast/collision geometry together.
4. Split that piece into two or three debris objects with velocity, gravity and
   a defined collision policy.
5. Connect the machine-pistol impact volume to the damage event, then cover
   repeated cuts and independent instances sharing source assets.
6. Extend to eligible static imported meshes, combined geometry and eventually
   deforming/skinned models under separately agreed contracts.

Acceptance should include nearby hits accumulating together, different faces and
different primitives staying independent, correct behavior under transforms,
no fragments before threshold, a real hole after threshold, valid fragments with
no duplicated volume, repeat cuts, failed cuts, target removal and cleanup.
