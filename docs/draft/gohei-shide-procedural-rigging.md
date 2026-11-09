# Procedural gohei, shide, and authored rigs

Status: design sketch, 2026-11-08. No implementation in this change.

## First example and intended result

Build a gohei with a rigid staff and one or more procedurally generated shide.
The staff is moved by holding/grabbing the prop root. Only the paper is skinned.
Its attachment stays fixed to the staff while the hanging sections can lag and
settle under secondary motion.

The clarified front view has tall rectangular visible segments descending
diagonally, mostly in the XY plane. Neighboring visible segments overlap in Y
but not in X. Within that overlapping height interval, paper travels horizontally
under/behind the visible segment and is folded. The hidden return/connecting
portion is real sheet geometry, not just a shared side-edge hinge between two
front rectangles. The earlier square-corner/point-contact interpretation is
superseded. Fold/connection regions should bend more easily; broad visible
panels should also bend a little.

The first rigging milestone is **binding the paper to an authored joint chain
with automatic weights**. Secondary motion follows after static posing works.
Neither automatic skeleton inference nor a cloth solver is required to reach
that milestone.

## Existing pieces and actual gaps

These are observations of the current repository, not proposed new APIs:

| Existing piece | What it provides / what needs work |
| --- | --- |
| [`CpuMesh`](../../src/engine/graphics/mesh.rs) | `joints0` and `weights0`, four influences per vertex, plus `with_skinning`. We can generate ordinary skinnable mesh data. |
| [`Skin`](../../src/engine/graphics/skin.rs) | Shared joint ordering and inverse bind matrices, currently described by glTF URI, skin index, and node indices. Authored rigs need identities independent of import metadata. |
| [`SkinnedMeshSystem`](../../src/engine/ecs/system/skinned_mesh_system.rs) | Dirty binding tracking and mesh-local palette updates. Binding keys and joint lookup currently use a glTF component as instance owner. |
| [`SecondaryMotionSystem`](../../src/engine/ecs/system/secondary_motion_system.rs) | Fixed-step spring chains, per-joint configuration, collision, and lifecycle tracking. Resolution, rest data, automatic chain discovery, and collision ownership currently depend on glTF. |
| [`RenderableComponent`](../../src/engine/ecs/component/renderable.rs) | Serializable procedural shapes including polygon, cylinder, and helix constructors. No folded-sheet authoring shape yet. |
| [`TransformParent`](../../src/engine/ecs/component/transform_operator.rs) | Existing transform inheritance redirection to examine/reuse for rigid attachments. |
| [`RestAttachment`](../../src/engine/ecs/component/rest_attachment.rs) | Imported rest-space metadata consumed by other systems; it is not a general live joint-parenting component. |
| [`android` renderer](../../android/src/renderer/mod.rs) | Currently explicitly rejects skinning/morph meshes. Android deformation support is a separate renderer slice, not something authored rigs already enable. |

The core project is an **authoring and binding layer over existing deformation**,
plus removal of import-only ownership assumptions. Do not create fake GLTF
components or export/reimport a temporary GLB just to activate those systems.

## Prop structure

Illustrative logical hierarchy; names below are proposals:

```text
GoheiRoot                         <- grabbed transform / rigid prop motion
├── Staff                         <- existing cylinder or authored mesh; unskinned
├── PaperMountA                   <- fixed offset on staff
│   ├── PaperRigA                 <- authored armature scope
│   │   └── Anchor               <- fixed to mount, excluded from simulation
│   │       └── Neck0
│   │           └── Panel0
│   │               └── Neck1
│   │                   └── Panel1 ... Tip
│   ├── PaperGeometryA            <- one mesh or a renderable subtree
│   └── PaperMotionA              <- spring config referring to this rig
└── PaperMountB ...               <- independent strip/chain, if desired
```

Geometry and joints share a mount basis but need not be structural parents of
one another. Skin bindings establish their relationship. Mount motion must
reach the paper exactly once. The mount's rigid wrapper can later carry a tie,
ornament, or other unskinned decoration.

Treat each paper strip as an independent chain initially. Joining strips into
one mesh must preserve strip identity and prevent cross-strip auto weights.

## Procedural geometry: a folded sheet, not a row of cubes

### Suggested built-ins

Start with one reusable geometry capability, with a small shide convenience
factory above it. The exact MMS syntax remains open.

| Proposed capability | Purpose | Priority |
| --- | --- | --- |
| `R.folded_sheet(pattern, folds, options)` | Tessellated 2D cut pattern with finite crease segments, folded into a 3D rest mesh. Retains material coordinates and region labels for rigging. | Core papercraft primitive |
| `shide(config)` prefab / pattern factory | Generates the chosen step silhouette, cut pattern, fold sequence, attachment region, and suggested joint guide. Uses the folded-sheet capability. | First example |
| `R.ribbon(stations, options)` | Width-varying strip using explicit positions and width directions, useful for ribbons, streamers, and simplified paper prototypes. | Optional later convenience |

Avoid implementing all three general-purpose systems before the example works.
Implement the folded-sheet subset needed by one validated shide pattern first;
keep shide-specific layout rules in a focused pattern generator. A ribbon sweep
alone does not guarantee that a shape can be cut and folded from flat paper.

### Pattern and folding contract

For the first shide pattern, think in terms of **visible panels and hidden folded
horizontal connections**. A front silhouette alone does not describe the material
path. Each repeat needs a tall front-facing panel, the finite-width horizontal
paper run beneath/behind it in the vertical-overlap interval, and the crease(s)
that route the continuous sheet onward to the next panel. Use a small Z separation
for layers rather than coplanar overlapping triangles. Which face lies on top,
the signed fold angles, and the exact unfolded cut pattern remain to be worked
out with an unfolded/front/edge-view prototype; do not invent a canonical fold
sequence from the silhouette alone.

The smallest useful mesh-authoring prototype is a tessellated cut sheet with
explicit crease lines and face labels. Keep the horizontal runs in its material
topology even when hidden in the front view. That topology drives the joint guide
and weights; a guide drawn only through the centers of visible panels would
shortcut the actual paper path. Controls should distinguish visible panel width
and height, downward step/vertical overlap, horizontal return extent, crease
placement, and layer clearance. These must obey the eventual cut/fold recipe
rather than being independent dimensions that stretch paper to fit.

Author the paper in material space `(u, v)`: a flat cut outline, internal cuts
where required, crease segments, and named faces/regions. Define finite-width
bridges explicitly. Do not make a projected corner contact the actual material
connection. Start with one chosen shide variant; the user's silhouette is the
target, not a claim that every shide uses this exact pattern.

For the first pattern, use an ordered face/hinge chain. Folding rotates each
downstream face about the shared crease segment, maintaining edge lengths and
shared endpoints. This avoids needing a general origami constraint solver.
Reject cyclic/ambiguous fold graphs initially. Fold order and sign are explicit.

Keep two concepts separate:

- **Rest folds:** the authored 3D shape generated from the flat pattern.
- **Pose deformation:** small joint-driven changes around that folded rest shape.

The initial geometry can have ideal sharp creases, which are valid finite line
hinges. A finite-width connection is mandatory; a rounded crease radius is an
optional later refinement. A rounded fold needs an actual bend band and length
accounting, not a bevel that stretches the paper arbitrarily.

### Rig and weight before folding

The preferred construction order is:

```text
flat cut pattern + material topology
    -> tessellation + regions + joint guide
    -> create joints and calculate weights in flat/material space
    -> fold sheet and transport joint positions/orientation frames together
    -> capture folded geometry and joint transforms as the runtime bind/rest pose
    -> animate or simulate small departures from that folded rest pose
```

This preserves the intended neighborhoods during automatic weighting. Two
panels brought close by folding are not thereby adjacent along the paper.
Material connectivity must also respect cuts: even on the flat sheet, Euclidean
distance alone can accidentally cross a slit or reach another strip. Use the
ordered strip coordinate/region graph for this example, or distances along the
material topology for a later general method.

Joint IDs and vertex weights survive folding unchanged; joint local rest
transforms and inverse bind matrices are finalized afterward. Transport complete
joint frames using the fold/face mapping, with an explicit owning face or crease
frame for joints on hinges. Do not merely move joint positions and leave their
axes in the flat orientation.

The initial folds should come from the piecewise face/hinge geometry operation,
not from smoothly blended skinning of the flat sheet. Skin blending used to make
the folds themselves can round creases and shrink the pattern. Runtime skinning
then starts from the actual folded mesh and its matching folded rig.

Rigging before folding is a convenient authoring order, not a requirement to
throw away the flat pattern afterward: retained material coordinates allow the
same topology-aware weights to be regenerated while the visible mesh is folded.
Changing only fold angles may reuse weights if material topology and joint IDs
are unchanged, but still requires new folded rest transforms/bind matrices and
a simulation reset. Keep a flat preview for inspecting weights independently
of the folded preview.

Useful shide controls: panel size/count, finite connector width/length, cut
spacing, fold angles/signs, mount tab length, panel/connector subdivisions, and
optional visual thickness. Keep overall prop transforms outside these controls.
Validate dimensions and reject degeneracy, inverted faces, and obvious rest
self-intersections with useful diagnostics. Full continuous self-collision is
outside the first geometry milestone.

### Mesh output

- One connected logical sheet per strip, with triangles through every bridge.
- Enough longitudinal rows in connectors to bend; several rows through panels
  so broad regions can curve too. Two triangles per panel cannot show that.
- UVs and rig coordinates from the unfolded sheet, unaffected by fold pose.
- Region IDs (`mount`, `connector`, `panel`, strip ID), crease IDs, and an ordered
  material-space joint guide. This metadata belongs to the generated asset;
  do not overload UVs as the only rigging record.
- Split shading normals at sharp creases. Duplicate render vertices there may
  share a logical material vertex and must receive identical weights.
- For a zero-thickness sheet, use a verified two-sided opaque material path, or
  explicit reverse faces if necessary. Reverse faces need matching weights and
  opposite normals. Do not assume that imported/default material culling works.
- Optional thickness is a thin closed shell with edge walls, not stacked panels.
  Every shell copy inherits the same material-space weights. Thickness is visual,
  not a requirement for simulating a sheet.

Inspect the pattern flat, folded from the requested side, obliquely, and edge-on
before accepting the generator. These views catch fake corner connections and
missing depth much earlier than motion testing.

## Authored armatures and binding a renderable tree

Proposed responsibilities, independent of final component names:

1. **Armature:** scoped stable joint IDs, parent relationships, local rest TRS,
   and resolved transform handles. Ordinary transforms carry the animated pose.
2. **Joint creation:** explicit joints or chain generation from an ordered guide.
   The folded-sheet guide supplies positions and crease frames deterministically;
   arbitrary mesh-to-skeleton inference is deferred.
3. **Skin binding:** armature reference, explicit geometry roots, exclusions,
   weighting method/settings, and bind-space records per mesh.
4. **Rigid attachment:** joint/transform target and retained offset, with no
   vertex weights. Reuse ordinary parenting or TransformParent where suitable.

Binding recursively collects supported mesh renderables under the supplied
roots, snapshots their transforms, and reports the resolved list. Preserve
materials, child hierarchy, and mesh identities; merging meshes is unnecessary.
Exclude staff geometry explicitly. Stop at nested armatures/bindings and reject
already-skinned targets unless an explicit replacement operation was requested.
Reject unsupported geometry with a target path rather than silently skipping it.

Keep source meshes immutable. A derived skinned asset may be shared only when
source geometry, bind transforms, rig definition, and weight settings match.
Each live armature instance still has separate pose palettes and simulation.

### Coordinate-space invariant

For each mesh `m`, capture mesh and joint transforms in the same bind frame:

```text
IBM[m,j]       = inverse(J_bind[j]) * M_bind[m]
palette[m,j]   = inverse(M_now[m]) * J_now[j] * IBM[m,j]
vertex_world   = M_now[m] * sum(weight[j] * palette[m,j] * vertex_bind_local)
```

At rest the palette is identity. Distinct mesh local frames can require distinct
inverse bind matrices even when the meshes use the same joints. Share the
armature definition; do not incorrectly force one mesh's bind matrices onto all
renderables. Baking geometry into a common bind frame is another explicit option,
not an implicit mutation during parenting.

Moving the entire prop should apply one rigid motion to staff and paper without
rebinding. Posing joints changes palettes, not the rest mesh or source asset.
Reject singular bind transforms; initially restrict rig ancestors to positive
uniform scale to avoid unresolved shear/reflection and spring-length semantics.

### Automatic weights for this example

Use a **material-coordinate chain method** first. Each generated vertex retains
its strip ID and ordered longitudinal coordinate along the cut sheet. Place
joints at connector transitions and inside broad panels; map their guide
locations into the folded 3D rest pose.

Interpolate between neighboring joints using that coordinate, with short blend
regions around connectors and a broader, gentler distribution through panels.
Two influences are sufficient for the first method; fit the existing four-slot
format. Normalize finite nonnegative weights, pad unused slots with zero weight,
and validate joint indices. The attachment tab is fully weighted to the fixed
anchor. The terminal guide must cover the tip with no unassigned vertices.

This is still automatic weighting: the user chooses the rig and paper, and the
binder generates the weights. Using generation metadata makes it more reliable
than nearest joints in folded 3D space, where unrelated panels can lie close.

Weights determine how joint poses blend across vertices. **Mechanical stiffness
comes from joint motion settings**, not from weights alone. Narrow connector
joints get greater compliance; panel joints are stiffer but remain movable.
Use adequate mesh subdivision and joint placement for both effects.

### Topology-aware automatic weighting mode

Expose the distance/propagation policy as an explicit automatic-weighting mode.
The desired generalization for cut paper is **surface-flow weighting** (working
name): influence travels along connected material from a bone's seeded attachment
region, decaying with distance along that surface. It must travel around the end
of a slit, not jump across it. Two edges can occupy the same position and still
be opposite sides of a cut. Folding also never creates material adjacency.

A tentative algorithm:

1. Build adjacency from logical sheet topology, retaining cuts, connected-component
   IDs, and strip/region restrictions. Edge lengths are measured in flat material
   space for paper. An arbitrary mesh can use its undeformed rest surface.
2. Seed each bone from its explicit guide/attachment region on that surface.
   A bone can have a supported interval, not just a single start vertex. Resolve
   seeds through retained material IDs where available. For generic geometry,
   request or preview ambiguous associations; do not independently snap the bone
   onto every nearby folded layer.
3. Compute shortest-path surface distances from those seeds, initially using
   multi-source graph traversal with nonnegative edge lengths. Propagation crosses
   actual shared material edges only. A cut blocks passage across the gap but
   permits a longer route around its endpoint. Disconnected pieces are unreachable
   unless they have their own seeds/explicit binding association.
4. Convert distances to smooth decreasing influence scores using a configurable
   falloff radius/curve, optionally restricted to neighboring bones in the guide.
   Keep at most four influences and normalize. Unreachable or uncovered vertices
   produce diagnostics; do not fill them using a hidden Euclidean fallback.
5. Any weight smoothing uses the same material adjacency and restrictions. Copy
   the resulting logical weights onto normal/UV seam duplicates, front/back faces,
   and thickness copies. Render-only seams are not cuts; separately authored cut
   banks must never be welded just because positions coincide.

The user's “from where the bone starts” is the seed/association part of this
policy. The rest is distance along connected material, rather than straight-line
distance to a bone. Starting influence at bone roots only is an optional policy;
the paper guide can seed a bone's full supported span for more even panel weights.

An edge-graph shortest path is a useful first approximation, but its distances
can depend on triangulation direction/density. Compare on retessellated patterns
before promising isotropic surface distances. Triangle-surface geodesics or a
boundary-aware diffusion solver can improve quality later. Topology preservation
is required from the first implementation; a sophisticated solver is optional.

This mode prevents shortcuts across cuts; it does not automatically model
mechanical stiffness or guarantee that a narrow bridge blocks influence. A bridge
is real connectivity. Material-region restrictions and optional propagation costs
can refine influence deliberately; spring settings still control compliance.

For the first shide, the ordered material-coordinate method above is a cheaper
specialized option. Keep both modes named explicitly, with surface-flow as the
intended general cut-paper mode. A later `bone_distance` mode may use Euclidean
distance to bone segments for simple unrestricted meshes, but must not silently
replace either paper mode. Preview influence colors, seed locations, cut
boundaries, and uncovered regions before accepting a bind.

### Parenting without skinning

An ornament can follow a joint as a rigid subtree:

```text
offset = inverse(joint_world_at_attach) * object_world_at_attach
object_world_now = joint_world_now * offset
```

Offer preserve-world attachment and explicit local-offset attachment. Resolve
through the existing effective transform basis, reject dependency cycles, and
define detach to preserve world pose. No skin data or deformation draw variant
is needed. The gohei staff simply stays under the grabbed root; it does not
need a joint attachment or a one-bone skin.

## Secondary motion after binding works

Generalize spring ownership/reference resolution to an authored armature scope,
including rest lookup, joint membership, collision exclusion, rebuilds, and
removal. Imported glTF rigs should adapt to the same runtime contract. Preserve
the current compact chain iteration and dirty binding paths.

The fixed mount/anchor is outside the simulated chain. Simulate from the first
flexible connector and use explicit per-joint settings to distinguish connectors
from panel interiors. Generate that configuration from semantic regions rather
than fragile globally unique names. Each strip gets its own chain and state.

Current tuning references:

- [`soft_hair_chain` / `_4`](../../assets/components/secondary_motion/spring_bone_presets.mms):
  stiffness `1.0`, drag `0.35`, gravity power `3.0`.
- [`mitten_bow_secondary_motion`](../../assets/components/secondary_motion/mitten-bow.mms):
  stiffness `1.5`, drag `0.45`, gravity power `1.5`.

Proposed paper tuning **starting points, not validated physical constants**:

| Region | Stiffness | Drag | Gravity power |
| --- | --- | --- | --- |
| Flexible connector | 0.8–1.5 | 0.45–0.60 | 0.15–0.40 |
| Panel interior | 2.5–5.0 | 0.45–0.60 | 0.15–0.40 |
| Mount tab | Fixed; not simulated | — | — |

Lower gravity gives less downward acceleration; drag damps velocity and can
help settling but is not a gravity multiplier. Tune at the actual paper size
and joint spacing. The requested slower falling is a stylized paper preset,
not a mass-only parameter change or a complete aerodynamic model.

Test staff translation, rotation, release, rapid reversals, and teleport/reset.
Preserve mount-driven inertia; do not cancel it by choosing a simulation center
that removes all prop motion. Reset previous/current simulation positions on
teleport and regeneration. Fixed-step behavior should stay consistent across
render rates.

The current spring chain is an approximation: it does not enforce paper
inextensibility, crease-axis-only bending, torsional stiffness, or sheet
self-collision. Start with modest deflections. If it visibly destroys fold shape,
add a separately reviewed hinge/angular-limit constraint slice; do not conceal
that limitation by increasing stiffness indefinitely. Air flutter, wind, true
cloth, and paper-paper collision remain later work.

## Lifecycle, saving, and renderer boundaries

Persist pattern parameters, folds, stable joint IDs/rest data, binding recipe,
explicit rigid attachments, and motion configuration. A deterministic generated
cache can be rebuilt; live palettes, resolved handles, and simulation state are
runtime data. Use versioned generator/cache keys. Save/reload and multiple prop
instances must retain independent poses and bindings.

Geometry/topology edits invalidate weights and bind data. Rest-joint edits require
an explicit rebind, not automatic capture of the current simulated pose. Stage
regeneration and commit geometry, rig, binding, and spring reset together. Failed
generation should leave the last valid result available with a visible error.
Ordinary pose changes never trigger geometry generation or weight solving.

Reuse desktop deformation and bounds/raycast integration; verify deformed paper
does not disappear because only its rest bounds were retained. Decide explicitly
whether picking follows deformation; grabbing the first example can target the
staff/root collider independently of paper picking. Paper has no separate rigid
body merely because it has spring motion.

Android requires explicit deformation support before this renders as skinned
paper there. Keep generic custom materials with an opaque/albedo surface base;
no legacy material pipelines or silent static fallback. Any Android skinning
slice must review existing deformation architecture and XR2 attachment lifetimes,
load/store operations, transient memory, and copies. This proposal does not
authorize extra render targets, paper transmission, or a cloth render pass.

## Suggested implementation slices and acceptance

| Slice | Deliverable | Acceptance |
| --- | --- | --- |
| 1. Static papercraft | One folded-sheet/shide generator and unskinned staff prefab | Flat/folded/edge views show finite bridges, intended step silhouette, correct normals, and usable tessellation. |
| 2. Authored rig + automatic bind | Explicit/guide-created joints, mesh-tree binding, material-coordinate weights, manual posing | Rest identity; bending at connectors and slightly within panels; staff untouched; translated/rotated child meshes bind correctly. This is the first requested rigging milestone. |
| 3. Rigid attachment | Reuse/extend transform attachment semantics | Decorations follow a joint without skinning; preserve-world attach/detach; cycles rejected. |
| 4. Paper motion | Authored-armature spring ownership and paper preset | Fixed mount, visible lag, slower downward settling than reference hair/bow, stable grab/release and reset, independent strips/instances. |
| 5. Polish / broader support | Diagnostics, cache/save hardening, optional generic weights; Android as an explicit separate slice | Reload/regenerate/removal work; measured CPU/GPU cost; supported platforms and limitations documented. |

This is several coordinated engine/content slices, not just a new constructor.
The main uncertainty is whether current spring rotations preserve the desired
folded-paper character; test one strip before expanding the generic API. A
calendar estimate should follow that prototype and a decision on Android scope.

For implementation, extract responsibilities before expanding oversized files:
`graphics/mesh.rs` is already over 1,800 lines and the spring system over 2,600
at this draft's date. Put sheet generation, rig binding/weights, and spring
owner resolution in focused modules. Retain batched uploads and palette updates;
no per-draw allocations, geometry rebuilding, or new pipelines per paper strip.

Meaningful verification should include bind-space math for multiple mesh frames,
seam weight agreement, malformed input diagnostics, subtree exclusions, instance
isolation, disposal/rebind, and visual deformation. Measure generator/bind time
separately from steady-state spring/deformation cost on a representative multi-strip
prop. No code or tests are changed by this draft.
