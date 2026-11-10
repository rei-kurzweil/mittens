# Naive gohei prototype

This first implementation deliberately precedes P1–P5. It establishes authored
joints, mesh-tree binding, and a visible pose before a connected cut sheet or physical folds.

`examples/paper-craft.mms` places three separately reusable specimens beside one
another on the widened 6.4 m rooftop bench:

- `assets/components/paper_craft/gohei_squares.mms`: unrigged reference.
- `assets/components/paper_craft/gohei_naive_weights.mms`: automatic weights and
  a repeating joint-pose demonstration. This animation is not secondary motion.
- `assets/components/paper_craft/gohei_black_spring.mms`: seven white panels and
  reduced-gravity spring motion on a longer, thinner black staff.

The first two call `gohei_squares_common.mms`. Five 11 cm planes step 11 cm across and 8 cm down,
with a 2 mm depth step. Adjacent edges meet over 3 cm vertically without
horizontal overlap. Each panel has front/back built-in plane faces. These are
separate staggered panels, not a connected folded paper surface: this is
intentional for the Euclidean baseline. Four vertices per face also means coarse
bending; panel interior curvature and narrow folded connectors need later meshes.
The staff is an existing cylinder. The root is the grip frame, 18 cm above the
staff's bottom, with a separate mount for the paper and armature. Grabbing moves
the whole prop; only the paper subtree receives skin weights.

## Authoring API

```mms
let joints = T { Joint {} /* child joint transforms */ }
joints
T {
    joints  // optional: joint tree can also live elsewhere
    Armature.joints(joints).give_automatic_weights() {
        paper
        other_paper
        SecondaryMotion {
            SpringBone.from_root("[name='paper_joint_1']")
                .virtual_end_length_ratio(1.0)
                .stiffness(0.8).drag_force(0.55)
                .gravity(0.25, [0, -1, 0])
        }
    }
}
```

`Armature` is the ancestor pose driver for its descendant renderables. Its
`.joints(component_or_selector)` reference selects the joint-tree root independently
of parenting. Discovery excludes that referenced subtree if encountered and stops
at nested Armature boundaries. Renderables below joints therefore follow rigidly
without automatically receiving weights. The joint tree and meshes can share a
prop transform for grabbing without making the joint tree an Armature child.
There is no separate public Rigging component. Rust uses
`let mut armature = ArmatureComponent::new(root); armature.give_automatic_weights()` for the same request.
Emit let-bound components before using their live handles. `status()` returns
binding diagnostics. A live `give_automatic_weights()` explicitly requests a
new rest capture; return the prop to its intended rest pose first. Initial capture
runs before animation. Authored joint rest snapshots also feed the existing
spring solver; pose/physics updates do not regenerate weights.

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

Unsupported inputs report a status: CombineMesh,
already skinned or morphed geometry, empty meshes, duplicate joint frames, and
singular bind transforms. Unlit and other geometry programs without a supported
deformation variant are rejected at binding. Geometry/topology edits require explicit rebind. A
failed rebind currently leaves source geometry unbound; preserving the previous
valid binding across such edits remains P6 work. Picking/bounds still use the
engine's existing undeformed mesh bounds. Android skinning remains separate.

The third specimen, `gohei_black_spring.mms`, has a 1.28 m staff (1.6×),
a 12 mm diameter (half the baseline), a black anime staff with strong rim
lighting, and seven white paper panels with reduced-gravity secondary motion. The
mount joint stays outside the simulated chain; there is no pose animation on it.
The first two specimens remain unrigged/animated comparisons.

## Try it

Launch `cargo run --release -- load examples/paper-craft.mms`. Find `01 / SQUARES`,
`02 / NAIVE WEIGHTS`, and `03 / BLACK SPRING` on the rooftop bench. Watch the second specimen animate and move the third to excite its springs,
grab any staff, and inspect both sides with the portable tripod light.
Headset interaction and visual acceptance remain manual checks.

Next slices: connected/tessellated sheet, surface-flow weights around cuts,
actual folds with transported joint frames, then folded-paper secondary
motion refinement. Keep these as additional prefab versions so
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


## Ancestor Armature and black spring specimen validation

The API now places mesh trees inside `Armature.joints(root)` and removes the
public Rigging component. Joint trees can be siblings or external references;
including the referenced tree under the driver is optional. Six focused rigging
checks cover external references, nested-driver boundaries, rigid joint-tree
renderables, rest identity, independent instances, animation, and cleanup. The
black specimen adds seven joints/fourteen paper faces and a six-joint spring
chain, keeping joint zero fixed. Its gravity is 0.25 versus the mitten bow's 1.5.

49 selected rigging, imported spring, collision, visualization, scene, and host
regression checks passed. The stereo Vulkan fixture also passed with all three
specimens visible, 24 skinned paper instances, a manual pose on the white rig,
and a moved/simulated black rig. These are correctness checks, not timing results.
Headset feel and appearance remain manual acceptance.

A broader Bisket scene-count test reports 15 chains against an expectation of 17;
the unchanged preset currently comments out its two bust chains. That expectation
was left outside this armature change.


## Plane panels, hand grip, and XR walking

All paper faces now use the shared built-in `R.plane()` quad with transform
scaling. Each face still gets private generated weights when bound. One Armature
drives all ten faces of the five-panel specimen, or fourteen faces of the
seven-panel specimen. Panels step diagonally with vertical overlap, no horizontal
overlap, and a 2 mm depth stagger. Joint rest offsets and pose-demo translations
follow the same spacing. This remains a disconnected baseline, not folded paper.
Paper stays white on every staff; the third staff retains black anime shading
and its strong lavender rim.

The gohei roots use `Grabbable.to_hand(true)` at their existing grip origins.
Paper-craft uses the original hand-bone attachment mode. The controller-grip
experiment was disabled after an in-headset test showed incorrect gun orientation.
Only the black spring gohei uses a local `.hand_offset([-0.015,0,-0.055])`, pulled
back 5 cm from the initial correction and shifted 1.5 cm left. A child transform
tilts the staff and paper together 30 degrees forward (-X rotation), about the
grip origin, so hand snapping does not overwrite the tilt.
The avatar already supplies the solved hand grip attachment to each hand pointer,
as it does for the machine pistol. No extra pointer hand-bone selector is needed.

`examples/xr-settings.mms` now explicitly includes WalkingIK and the same sole
offsets used by paper-craft. Existing locomotion, static floor/cube colliders,
routed AVC slide capsule, and CapsuleReady-gated gravity remain the contact path.
Headset movement and grip alignment still require manual acceptance.

Validation for this revision: 11 focused rigging/grip/capsule checks passed,
three paper-craft/XR-settings scene checks passed, and the stereo Vulkan render
fixture passed. The XR-settings fixture exercises the real authored stage
colliders with a representative routed capsule: floor support, cube-side blocking,
and standing on a cube. It does not replace testing the tracked avatar in a headset.

## Two shide and spring tuning

The black gohei now uses two independently weighted seven-panel shide, mirrored
left/right with a small depth separation. `gohei_shide.mms` owns each strand's
geometry, binding and animation; the other gohei variants retain one strand.
Both first panels' joint zero stays anchored at the staff. Each simulated chain
starts at joint one, so the first panel is excluded from spring collision solving.
The paper chains already disable both world and avatar collision; there is no
per-joint collision exemption API added in this change.

Below the fixed attachment, stiffness changes only from 0.8 to 0.75 and drag
from 0.55 to 0.50 for a small increase in springiness. Gravity is 2.7 versus the
hair preset's 3.0. The existing local grip offset and 30-degree forward lean remain.
