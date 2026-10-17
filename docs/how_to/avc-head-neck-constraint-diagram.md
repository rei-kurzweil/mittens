# Interactive AVC head/neck constraint proposal

Run from the repository root:

```sh
cargo run --release -- load examples/avc-head-neck-constraint-diagram.mms
```

This is a diagram scene with a desktop camera. It does not implement the new
avatar behavior, create an avatar, or initialize an XR session. Operator and
algorithm labels describe the proposal; only `TOP.yaw_follow` is an existing
operator API. The math/patch and bounded-follow constructor names are not settled.

Use WASD and R/F to navigate and right-mouse drag to look. The initial view
shows the full diagram; approach a section to read its cards comfortably. Click
any of the 15 cards to populate the information panel. Drag its title bar to
move it. Collapse/restore keeps the selected card, including a new selection
made while the panel is collapsed.

## Three sections

1. **Operator facilities:** existing yaw follow, proposed declarative math/output
   patching, and proposed bounded translation follow. The math facility contains
   reusable scalar, Vec3 and quaternion operations; these are not separate ECS
   components for every arithmetic step.
2. **AVC-generated constraint:** source preparation, retained rig calibration,
   shared body yaw, preferred rotation/vector math, neutral-relative measurement,
   candidate follow, geometric closure, output publication and state acceptance.
   Repeated facilities are shown in their specific algorithm context.
3. **Future MMS example:** ordinary desktop and XR authoring shells with `AVC`,
   plus the observations needed to verify the eventual implementation. These
   are displayed outlines. The later example will request the feature through
   AVC's default behavior rather than authoring the internal operator graph.

Cards show input, output, how it works and why useful. The panel adds contracts,
limits, implementation questions and documentation references. The future desktop
and XR cards also expose multiline MMS outlines. Sections 01 and 03 deliberately
have no runtime dataflow wires: one is a library and the other is authoring
context, while section 02 contains the actual proposed algorithm.

## Reading the algorithm

Cyan denotes pose/math flow, gold calibration or authored context, green an
accepted/connected pose, and violet retained follow state. Arrows carry shared
upstream context in addition to their named result; they do not imply separate
copies of filters or calibration.

Solid bright cards with black text identify proposed behavior and future
examples. Outlined cards identify the existing yaw-follow facility and source
input context. This status is explicit in the card notes; the color identifies
the card's role rather than whether the feature is implemented.

The middle section's main lane reads left to right: source, neck arc/head target,
planar measurement, candidate follow, geometric closure, then publication.
Calibration and body yaw feed that lane from above. State acceptance sits below
publication and returns underneath the lane. A gap at a crossing distinguishes
the source-to-yaw branch from the calibration input.

The violet return from **Commit the accepted anchor** to **Propose the body
anchor** is delayed until the next intended update. It is not a circular read
of the current solved skeleton. The final geometrically accepted result becomes
history; an unvalidated candidate does not.

The initial range calculation uses:

```text
a = (desired_head_position - neutral_root_to_head_displacement).xz
e = a - previous_accepted_body_anchor.xz
R_follow >= maximum_supported_rotation_only_XZ_offset
```

Both source types use the same calibrated range and follow rule. Small generated
desktop head arcs fit within that allowance from the neutral anchor; extra XR
translation can exceed it. The following closure stage still has to preserve
joint lengths and satisfy limits. A circle containing rotational motion does not
make every position inside it reachable. The panel explains this distinction
and the boundary case of rotating after already leaning near the threshold.

The output stage must propagate the accepted body/neck pose before dependent arm
or leg solves and skinning. Exact limit projection, expression syntax and the
operator state acceptance contract remain proposal work, rather than hidden
implemented APIs.

## Files and related design

- [Runnable diagram scene](../../examples/avc-head-neck-constraint-diagram.mms)
- [Card data and detailed notes](../../assets/components/diagrams/avc_head_neck_notes.mms)
- [AVC constraint task](../task/xr-head-neck-bend-and-body-offset.md)
- [Declarative transform math](../draft/declarative-transform-math.md)
- [OpenXR reference diagram](../../examples/0-9-x-openxr-flowchart.mms)

The diagram uses the reference scene's authored flex rows, orthogonal geometric
connectors, selectable cards, and a persistent `info_panel`. Updating a selection
replaces only the panel body; the draggable panel root and title stay alive.
