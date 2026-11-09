# Naive gohei prototype

This first implementation deliberately precedes P1–P5. It establishes authored
joints, mesh-tree binding, and a visible pose before real cuts/folds or physics.

`examples/paper-craft.mms` places two separately reusable specimens beside one
another on the widened 6.4 m rooftop bench:

- `assets/components/paper_craft/gohei_squares.mms`: unrigged reference.
- `assets/components/paper_craft/gohei_naive_weights.mms`: automatic weights and
  a repeating joint-pose demonstration. This animation is not secondary motion.

Both call `gohei_squares_common.mms`. Five 11 cm squares step 8 cm across/down,
leaving 3 cm of overlap at the joins. They have front/back faces. These are
separate overlapping panels, not a connected folded paper surface: this is
intentional for the Euclidean baseline. Four vertices per face also means coarse
bending; panel interior curvature and narrow folded connectors need later meshes.
The staff is an existing cylinder. The root is the grip frame, 18 cm above the
staff's bottom, with a separate mount for the paper and armature. Grabbing moves
the whole prop; only the paper subtree receives skin weights.

## Authoring API

```mms
let paper = T { /* one or more renderables, with their own transforms/materials */ }
paper
let armature = Armature {
    T { Joint {} /* child transforms with Joint markers form the joint chain */ }
}
armature
T {
    armature
    paper
    Rigging.new(armature, paper).give_automatic_weights()
}
```

Emit a let-bound component before using it as a live handle with `attach`.
`Rigging.new` accepts component handles or selector strings. `status()` returns
binding diagnostics. Calling `give_automatic_weights()` on a live Rigging requests
another bind; first return the joints and geometry to the intended rest pose.
Rest capture runs before the animation phase of the initial simulation tick.

`RiggingSystem` computes inverse squared world-space distance to joint origins,
keeps at most four influences, and normalizes them. A vertex exactly at a joint
gets full weight there. It does not consider connectivity, cuts, bone segments,
or visibility. Nearby disconnected surfaces can influence each other.

Each renderable gets its own inverse bind matrices and private transient weighted
mesh; shared source meshes stay unchanged. Pose updates use the existing dirty
joint/palette path in `SkinnedMeshSystem`, with no mesh regeneration. Per-mesh
transforms/materials remain in their original hierarchy. Removing the binding
restores source geometry; removing the prop retires its skins and generated meshes
on the next update/render preparation. Losing a member suspends the rig until an
explicit new request, rather than recapturing the current animated pose.

Unsupported inputs report a status: nested armatures/bindings/CombineMesh,
already skinned or morphed geometry, empty meshes, duplicate joint frames, and
singular bind transforms. Unlit and other geometry programs without a supported
deformation variant are rejected at binding. Geometry/topology edits require explicit rebind. A
failed rebind currently leaves source geometry unbound; preserving the previous
valid binding across such edits remains P6 work. Picking/bounds still use the
engine's existing undeformed mesh bounds. Android skinning remains separate.

## Try it

Launch `cargo run --release -- load examples/paper-craft.mms`. Find `01 / SQUARES`
and `02 / NAIVE WEIGHTS` on the rooftop bench. Watch the second specimen bend,
grab either staff, and inspect both sides with the portable tripod light.
Headset interaction and visual acceptance remain manual checks.

Next slices: connected/tessellated sheet, surface-flow weights around cuts,
actual folds with transported joint frames, then authored-armature secondary
motion with reduced paper gravity. Keep these as additional prefab versions so
the original baseline stays available for comparison.

## Validation

- `cargo test --lib -- rigging_system paper_craft configured_ --skip
  typed_material_inputs_and_builtin_definitions_share_one_runtime_path
  --test-threads=1`: 19 passed. Includes finite normalized naive weights,
  disconnected-vertex behavior, transformed mesh-tree rest identity, root motion,
  independent instances, pose-demo callbacks, source isolation, cleanup, staged
  initial-bind failure, and existing paper-craft scene/collision checks.
- The skipped existing material test expects the shared builtin anime definition
  ID but receives a separate definition ID. The same assertion fails with the
  committed runtime/registry implementations restored for comparison.
- `cargo test --lib twilight_sky_stereo -- --ignored --nocapture
  --test-threads=1`: passed with GPU access. The fixture now captures bench/rest
  and explicitly posed views in both eyes, verifies palette changes and more
  than 100 changed pixels, and retains the existing sky checks. Both-eye captures
  are `/tmp/paper-craft-twilight-gohei-{bench,posed}-{0,1}.png`.
- These are correctness checks, not a performance benchmark. Headset grabbing,
  lamp interaction, and final appearance still need user inspection.
