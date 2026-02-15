# Skinned Toon Mesh shader (design notes)

This doc describes what the planned `SKINNED_TOON_MESH` pipeline needs on the GPU side.
It assumes **max 4 joint influences per vertex** (glTF `JOINTS_0` / `WEIGHTS_0`).

## Goal

Render a mesh whose vertex positions/normals are deformed by a skeleton pose (skinning),
using the same general “toon mesh” lighting model.

## Coordinate spaces (important)

- Mesh vertices (`CpuVertex.pos`, `normal`) are in **mesh local space** (the node’s space).
- The engine currently also provides a per-instance **model matrix** via instancing.

For skinning, it’s simplest to keep the existing instancing model:

1. **Skin in mesh-local space** (so we can still apply the instance model matrix after skinning).
2. Then apply the usual instance model matrix to get world space.

That implies the per-joint matrices the shader consumes should also be expressed in **mesh-local space**.

### Joint matrix derivation

glTF provides:
- A node hierarchy with TRS (or a 4×4 matrix) per node.
- A `skin` with `joints[]` and `inverseBindMatrices[]`.

Let:
- $M_{meshWorld}$ be the mesh node’s world matrix.
- $M_{jointWorld}(j)$ be joint node $j$’s world matrix.
- $IBM(j)$ be inverse bind matrix for joint $j$.

A common choice for matrices that transform **mesh-local** vertex data is:

$$
SkinMat(j) = M_{meshWorld}^{-1} \cdot M_{jointWorld}(j) \cdot IBM(j)
$$

Then the shader can do:

$$
P_{local}' = \sum_{k=0}^{3} w_k \cdot SkinMat(j_k) \cdot \begin{bmatrix} P_{local} \\ 1 \end{bmatrix}
$$

and finally:

$$
P_{world} = M_{instance} \cdot P_{local}'
$$

Notes:
- If you instead choose to output directly in world space, you can drop $M_{meshWorld}^{-1}$ but then you must not also apply $M_{instance}$.
- Rotation/scale affects normals. Either skin normals with the upper 3×3 of each matrix and renormalize, or use an inverse-transpose normal matrix for better correctness.

## What the vertex shader needs

### Per-vertex attributes

Existing toon mesh vertex attributes already used:
- `pos: vec3`
- `uv: vec2`
- `normal: vec3`

New attributes for skinning:
- `joints0: uvec4` (or `vec4` + cast), holding 4 joint indices.
- `weights0: vec4`, holding 4 weights.

Assumptions/contract:
- If a vertex has fewer than 4 influences, unused joints can be 0 and unused weights 0.
- Weights should sum to ~1.0 (optionally renormalized in shader for robustness).

Engine-side note: `CpuMesh` currently stores `joints0` / `weights0` as optional arrays separate from `CpuVertex`.
That means the renderer will likely provide these via either:
- a second vertex buffer binding, or
- an expanded `CpuVertex` for skinned meshes only.

### Per-instance attributes (already exist)

The engine already supplies an instance model matrix in `InstanceData`.
The skinned pipeline should continue to use it.

### Joint matrices buffer (new real usage)

The shader needs to read **SkinMat** for each joint index referenced by a vertex.

Recommended representation:
- `SkinMat` packed as `mat4` per joint.
- Stored in an SSBO.

The repo already has a descriptor set layout slot for this:
- set `2`, binding `1` = “bones” SSBO (see src/engine/graphics/pipeline_descriptor_set_layouts.rs)

#### How to index the bones

We need a way to map a draw/instance to the correct slice of joints in the bones SSBO.
Typical options:

1) **Per-instance bone offset table**
- SSBO A: `instance_bone_offset[instance] -> u32`
- SSBO B: `bones[]: mat4`
- Then: `SkinMat = bones[instance_bone_offset[gl_InstanceIndex] + jointIndex]`

2) **Per-draw push constants (single skinned object per draw)**
- Push constant: `bone_offset`
- Then: `SkinMat = bones[bone_offset + jointIndex]`

Given the engine is heavily instanced, (1) is usually the better long-term plan.

### Inverse bind matrices (do we need them in shader?)

Not necessarily.

You *can* upload `M_jointWorld` and `IBM` separately and multiply in the shader,
but that doubles memory bandwidth and does extra math.

Preferred:
- Compute `SkinMat(j)` on the CPU once per frame (or whenever joints change).
- Upload only `SkinMat(j)`.

## Fragment shader needs

Skinned toon fragment shader can be identical to the existing toon fragment shader,
as long as the vertex shader outputs the same interpolants (UV, normal, etc).

Only difference might be:
- Normal handling: ensure skinned normals are normalized and in the expected space.

## Minimal engine data path (summary)

To drive `SKINNED_TOON_MESH` correctly, the engine will need:

1. **Import time**
   - `CpuMesh` has optional `joints0` + `weights0` arrays (already added)
   - glTF skin info: joints list + inverse bind matrices

2. **Runtime pose evaluation**
   - Node world matrices from hierarchy (TRS or `node.transform().matrix()` + parent chain)
   - Compute `M_meshWorld` for the skinned mesh node
   - Compute each joint’s `SkinMat(j)` using the formula above

3. **GPU upload**
   - Upload `SkinMat[]` into set=2,binding=1 bones SSBO
   - Provide bone offset/count mapping per instance/draw

4. **Shader**
   - Read `joints0/weights0`
   - Accumulate skinned local position/normal from up to 4 joints
   - Apply instance model matrix and camera view/proj

## Quick note: `node.transform().matrix()` vs decomposed TRS

- `node.transform().matrix()` gives a full 4×4 matrix directly.
- `node.transform().decomposed()` gives TRS components.

For skinning, having the full matrix is convenient, but you still must compute **world matrices** by multiplying with parent world transforms.
So the engine will want something like:

- `local = node.transform().matrix()` (or TRS->matrix)
- `world = parent_world * local`

This is the core of building `M_meshWorld` and `M_jointWorld`.
