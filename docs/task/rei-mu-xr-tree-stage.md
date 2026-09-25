# Rei(mu) XR tree stage

## Goal

Create `examples/rei(mu).mms`: an XR-only Rei(mu) scene with a moving two-sided hair-bow ribbon, a mirror, a procedural tree in a raised square garden, and two instances of the reusable Mittens Corp stage.

This document tracks the work as the scene grows. The first implementation pass should establish the complete scene and expose any model or runtime issue that prevents the ribbon from moving.

## Model inspection (2026-09-25)

`assets/models/rei(mu).glb` contains one skin with 142 joints. Its Bisket-style hair joints include `J_Sec_Hair1_01` through `J_Sec_Hair3_14`, with a fourth joint on chain 13. The existing Bisket spring presets and collider setup are the starting point for those chains.

The bow skeleton branches from one `head_bow` joint into two eight-joint chains:

- `head_bow.001` → `head_bow.002` → … → `head_bow.008`
- `head_bow.009` → `head_bow.010` → … → `head_bow.016`

Those 17 bow nodes are in the skin. However, the visible `head_bow_ribbon` mesh is a separate node with **no `skin` property**; its primitive has no joint attributes. The visible `head_bow` mesh is also unskinned. The skeletal `head_bow` is a direct child of `Armature.003`, rather than a child of `J_Bip_C_Head`. Before tuning springs, verify how the visible bow follows the avatar head and make the ribbon geometry follow the two chains. That may require correcting and re-exporting the GLB, or another explicit mesh deformation/attachment approach. Spring configuration alone cannot deform the current unskinned ribbon mesh. Preserve the original asset for comparison while investigating.

## Work plan

- [ ] Establish a visible, correctly attached bow/ribbon on the XR avatar. Inspect weights, bind transforms, and the head relationship in the source model or import path; document the chosen fix here.
- [ ] Add a Rei(mu) secondary-motion component under `assets/components/secondary_motion/`. Reuse the Bisket hair behavior where joint names match, and configure the two bow halves as separate spring chains. Tune stiffness, drag, gravity, virtual ends, and head collision after the mesh follows the chains. Check that each side lags and settles without stretching or clipping badly.
- [ ] Extract the Mittens Corp stage deck/platform, upper and lower steps, back wall, and ceiling truss into a reusable component under `assets/components/`. Keep the three-section hanging `suspended_platform` walkway outside it. Update `examples/mittens-corp.mms` to use the component while retaining its current stage placement and separate floor, mirror, lighting, and walkway.
- [ ] Add `examples/rei(mu).mms` with two instances of that stage. Put the player, mirror, and garden on the first. Offset the second farther down and to the side and rotate it 90° around the vertical axis. Keep each stage's children and names distinct if selectors require it.
- [ ] Build a tree using a bounded recursive MMS function with a trunk and progressively smaller branches. Put it in brown/beige soil inside four low, long, thin cube borders forming a raised square garden on the first stage. Set branch depth and size limits so startup and rendering stay manageable.
- [ ] Use a mirror with `Mirror.quality(1440)` for a 1440 × 1440 reflection, and place it for a useful full-body XR view.
- [ ] Set a slightly medium-dark gray sky/background and include `star_kawaii_background` from `assets/components/backgrounds/star_kawaii_background.mms`. Add suitable lighting so the avatar, bow, tree, and stages remain readable.
- [ ] Control Rei(mu) with the XR input/avatar topology (`InputXR`, `AVC`, `CXR`, and `XR.on()` as appropriate). Do not add a desktop or 3D scene camera. Use `assets/models/rei(mu).glb` and verify asset-path handling for the parentheses.
- [ ] Load the example through the regular MMS launcher, resolve script/runtime errors, and inspect it in XR. Confirm the mirror, both stage placements, garden, and both ribbon halves visually. Recheck Mittens Corp after the stage extraction.

## References and decisions

- `examples/mittens-corp.mms`: source stage geometry and existing 1440-quality mirror; its suspended walkway is separate.
- `examples/vtuber-secondary-motion.mms`: XR avatar and mirror topology.
- `assets/components/secondary_motion/bisket.mms`, `bisket-shirt-physics.mms`, and `spring_bone_presets.mms`: existing Bisket hair motion.
- `assets/components/backgrounds/star_kawaii_background.mms`: requested star field.

The exact tree silhouette, garden dimensions, stage offset, and spring values can be tuned in the implementation pass. Record the final values and any remaining visual issue here.
