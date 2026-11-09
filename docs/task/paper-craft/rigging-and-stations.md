# Papercraft and rigging: testable workshop stations

Status: connected-sheet/folding/physics slices remain planned. The
[naive baseline](naive-gohei.md) now implements ancestor Armature/Joint authoring,
Euclidean automatic weights, mesh-tree binding, an animated squares specimen, and a seven-panel reduced-gravity black spring specimen.
It does not satisfy surface-flow or folded-paper acceptance below. Implements the [design sketch](../../draft/gohei-shide-procedural-rigging.md)
after the [environment](environment.md) can host the work.

Each station keeps the previous stages visible. Do not replace the flat pattern
with the final moving prop: comparing the stages is the point of the long bench.

## P1 — Flat cut sheet and inspection station

- [ ] Implement the minimal tessellated cut-sheet representation with logical
  material vertices, explicit cuts/creases, region IDs, and front/back rendering.
- [ ] Make one proposed shide repeat, then a short repeated pattern. Include tall
  visible panels and the horizontal material runs that will fold behind them.
- [ ] Keep logical topology distinct from UV/normal/render vertex duplication.
- [ ] Display the flat sheet, cut edges, crease lines, and material-path guide.

Acceptance: cuts are real boundaries; finite material connects the pattern;
triangles are nondegenerate; lamp inspection works from both sides. Subdivision
is adequate in both connectors and panel interiors. Before freezing the recipe,
show flat/front/edge views that establish the intended actual folding sequence.

## P2 — Authored joints and surface-flow auto weights

- [x] Add scoped authored joint/rest identities and explicit or guide-created
  joints, independent of GLTF ownership.
- [ ] Implement explicit surface-flow weighting: seed attachment regions, follow
  logical material adjacency around cuts, apply distance falloff, cap/normalize
  up to four influences. Keep disconnected regions explicit and diagnosed.
- [ ] Show seed locations and selectable bone influence colors at the flat station.
- [ ] Include a slit/U-shaped test coupon and two nearby disconnected strips.
  Optionally show Euclidean weighting beside surface-flow for comparison.

Acceptance: no shortcut across the slit; measurable influence/distance takes the
route around its end. Disconnected strips receive no influence without seeds.
Normal/UV seam copies agree. Small retessellations do not radically alter weights;
record edge-graph anisotropy limits. All weights are finite/nonnegative, normalized,
and indexed to valid joints. Deterministic inputs reproduce the result.

## P3 — Fold construction and folded rest station

- [ ] Fold the face/hinge chain geometrically and transport joint frames with it.
  Preserve logical IDs, topology, and P2 weights.
- [ ] Capture new bind matrices/rest transforms only after folding; use small
  physical layer separation rather than coplanar hidden return faces.
- [ ] Display the folded rest shape beside its flat counterpart, with a way to
  inspect behind/edge-on without losing the guide correspondence.

Acceptance: front view has tall stepped panels with vertical overlap and hidden
horizontal folded runs; edge view proves their continuity. Rest skinning is
identity, crease edges match, and the pattern is not stretched to fit a silhouette.
No need for a general origami solver or smoothly skinned fold construction.

## P4 — Mesh-tree binding, manual pose, and rigid attachment

- [x] Bind Armature descendant mesh trees, preserving child transforms/materials; exclude the referenced joint subtree and nested Armatures.
  Report exclusions, unsupported targets, and conflicting bindings.
- [ ] Drive paper joints with a small repeatable pose control/animation before
  introducing physics; demonstrate flexible fold regions and gentler panel curvature.
- [ ] Add the unskinned staff under the grabbed prop root, plus a rigid ornament
  attached to a joint without weights. Test preserve-world attach/detach.

Acceptance: multiple meshes with different local transforms retain rest identity
and pose together. Moving the root applies once. Staff does not deform; ornament
follows its target. A second instance has an independent pose. Cycles/singular bind
frames fail clearly. Bounds/picking behavior is recorded and checked.

## P5 — Paper secondary motion and complete gohei

- [x] Generalize spring scope/rest lookup and lifecycle to authored armatures with referenced joint trees.
- [ ] Fix the mount outside simulation; use softer connector and stiffer panel
  joints. Start with the lighter-gravity ranges in the design draft.
- [ ] Add independent chains for each strip and reset simulation on teleport/rebind.
- [ ] Expose a final grabbable gohei beside the manual-pose specimen.

Acceptance: grab/move/rotate/release the staff in XR; attachment stays fixed while
paper lags. Compare reduced downward settling with the reference hair/bow preset
under equivalent motion. Check multiple render rates, rapid reversals, and reset.
If folds visibly collapse, record the failure and add a separate angular/hinge
constraint slice before calling the result accepted. This is not cloth/self-collision.

## P6 — Lifecycle, storage, and measured cost

- [ ] Save recipes, folds, rest joints, bindings, and motion configuration; rebuild
  derived caches without persisting live simulation or runtime handles.
- [ ] Rebind explicitly after rest/topology edits. Stage regeneration so failures
  preserve the last valid mesh/rig rather than leaving a half-updated prop.
- [ ] Verify removal, repeated regeneration, independent copies, and source-mesh isolation.
- [ ] Measure generation/binding separately from steady-state spring/deformation;
  compare one and several multi-strip props in the workshop.

Acceptance: reload matches the authored rest result; shared sources remain unchanged;
removed props release their bindings/state; moving only the pose never regenerates
geometry or weights. Preserve compact hot paths and batched uploads. Extract sheet
generation/binding/owner resolution into focused modules before expanding the
already oversized mesh and secondary-motion files.
