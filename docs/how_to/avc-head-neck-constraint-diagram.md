# Interactive AVC head/neck constraint proposal

Run from the repository root:

```sh
cargo run --release -- load examples/avc-head-neck-constraint-diagram.mms
```

This is a diagram scene with a desktop camera. It does not implement the new
avatar behavior, create an avatar, or initialize an XR session. Operator and
algorithm labels describe the proposal. The new catalog names are proposed full
component names, with shortforms left open; `TOP.yaw_follow` is existing context.
The numeric graph is intended to execute natively after MMS construction.

Use WASD and R/F to navigate and right-mouse drag to look. The initial view
shows the full diagram; approach a section to read its cards comfortably. Click
any of the 28 main cards to populate the information panel. Component tags below
the algorithm lane are also clickable. Each of the three sections is an
independent accordion. Drag its title bar or short vertical left handle to move
the section, including its cards and wires. The arrow at the right of each title
unloads/restores that section. The transparent body leaves space between cards
without adding a large enclosing background.

Headers use `Draggable.target("../../#section_name").plane("camera")` to move the
named inner panel root. They remain alive and draggable when minimized. These
are pointer handles; no `Grabbable` marker is needed for this desktop diagram.
There is no outer layout repositioning the independent panels after a drag.

The AVC-generated graph uses 2.6 times the font size of the compact catalog, while
keeping its section and card widths fixed. Graph and future-example cards use
`word_wrap("normal")` to wrap at word boundaries, with automatic heights for the
cards, text boxes and input/output blocks. Routing lanes reserve vertical space
separately; cards align at the top and keep their own measured heights. The
auto-height card spacing uses child margins, which participate in intrinsic
height measurement, rather than flex gaps. The
future-example section starts below the enlarged graph. Use
R/F navigation or minimize a section to reach content outside the initial view.
Operator lists are simply labeled **Uses**. Filled cards and tags explicitly set
black text in both their container styles and individual text styles.
The section panels set `inherit_body_text_color = false`: their nested layouts
own the text colors. Otherwise the outer panel's white color can register after
the inner layout and overwrite rendered glyph colors despite black authored styles.

Drag the info-panel title to move the panel independently. Collapse/restore
keeps its selected card, including a selection made while collapsed.

## Collapsing and performance

Minimizing removes the entire section body: cards, text/glyph entities, layout
roots, component tags and local wires. Each section has a zero-argument content
factory; restoring creates a fresh subtree at the retained panel's current
position. Factories retain descriptions/functions, not detached live component
trees. No background evaluation is needed for minimized diagram content.

To compare runtime behavior, collapse the operator library first, then the
algorithm, then the future-example section. Restore them separately and compare
responsiveness at the same camera position/window size. All sections start
expanded. The diagram no longer requests blur/bloom postprocessing. This is a
text-heavy scene; headless entity counts establish that content is removed, but
do not measure GPU frame time or identify every rendering bottleneck.

A headless check with the initial notes visible measured the following live ECS
components. Glyphs account for many components; these are not authored card counts.

| Sections remaining expanded | Live components | Renderables |
| --- | ---: | ---: |
| All three | 72,796 | 11,571 |
| Algorithm + future example | 40,664 | 6,477 |
| Future example | 16,934 | 2,726 |
| None; retained titles and detail panel remain | 9,954 | 1,616 |

Labels, typography, the selected note and selection/transition overlays affect
the exact totals.
Repeated restoration checks
also verify fresh bodies, single copies of cards/wires, retained positions, and
working drag handles while minimized.

The reusable `info_panel` now accepts `transparent_background = true` to omit
the **body** background geometry. The title and toggle retain their independent
color options. `body_padding` defaults to `0.65`; the diagram uses zero. Existing
`body_background_color` styling and manual restore handlers remain supported.
`inherit_body_text_color = false` omits the body-level text color for content that
styles its own text, including independent nested layouts. Its default is true.
Factory-owned restoration is opt-in:

```mms
info_panel({
    root_name = "example_section"
    width_gu = 52.0
    unit_scale = 0.075
    title = "Drag title / arrow minimizes"
    transparent_background = true
    content_factory = fn() { return T { Text { "Fresh content" } } }
})
```

With `content_factory`, do not also attach an external restore handler. Existing
callers using `content` can still handle `AccordionRestoreRequested` themselves
and rebuild via `info_panel_body`, including the detail panel in this example.

## Three sections

1. **Proposed value operator components:** sixteen catalog cards name the
   evaluation owner, sources, scalar/vector/quaternion components, stateful
   follow and transform-output bridge, plus existing yaw-follow context.
   [The component proposal](../draft/declarative-vector-operator-components.md)
   lists all names and typed inputs/outputs. These are numeric values rather
   than an assumption that every operation is a transform operator.
2. **AVC-generated constraint:** source preparation, retained rig calibration,
   shared body yaw, preferred rotation/vector math, neutral-relative measurement,
   candidate follow, geometric closure, output publication and state acceptance.
   Each detail panel names the value components that compose that stage.
3. **Future MMS example:** ordinary desktop and XR authoring shells with `AVC`,
   plus the observations needed to verify the eventual implementation. These
   are displayed outlines. The later example will request the feature through
   AVC's default behavior rather than authoring the internal operator graph.

Cards show input, output, how it works and why useful. The panel adds contracts,
limits, native execution details, implementation questions and references. The
future desktop and XR cards also expose multiline MMS outlines. Sections 01 and 03 deliberately
have no runtime dataflow wires: one is a library and the other is authoring
context, while section 02 contains the actual proposed algorithm.

## Reading the algorithm

Cyan **wires** denote pose/math flow, gold calibration, green an accepted/
connected pose, and violet retained follow state. Arrows carry shared
upstream context in addition to their named result; they do not imply separate
copies of filters or calibration.

Solid bright cards with black text identify proposed behavior and future
examples. Outlined cards identify the existing yaw-follow facility and source
input context. This status is explicit in the card notes; color identifies the
component. Catalog
fills and named component tags use one shared palette. A composite stage takes
its primary component's fill; the named tags below it show the exact colors of
the other components it composes. Detail notes also show matching tags. This
avoids assigning one ambiguous color to a stage containing several operations.

Quaternion operators use cool cyan, teal, blue and lavender tones. Vector
operators, including the stateful Vec3 `PlanarFollow`, use orange, yellow and
red tones; `VectorMeasure` uses a stronger red-orange to distinguish it from the
light pastel pink scalar operators. Wire colors continue to
describe dataflow roles independently of these component families.

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

`MathGraph` binds and checks authored nodes, then compiles typed value slots and
a flat native operation list. The graph is the authoring/validation form; a
small typed instruction enum is the preferred execution form. MMS can construct
the graph and edit its configuration; ordinary numeric updates should not traverse MMS ASTs, invoke
closures or repeatedly resolve scene selectors. `TransformChannelOutput` is the
explicit adapter into the transform system. Pure value nodes do not mutate
matrices as a side effect.

The output stage must propagate the accepted body/neck pose before dependent arm
or leg solves and skinning. Exact limit projection, connection syntax and the
state-acceptance contract remain proposal work, rather than implemented APIs.

## Files and related design

- [Runnable diagram scene](../../examples/avc-head-neck-constraint-diagram.mms)
- [Card data and detailed notes](../../assets/components/diagrams/avc_head_neck_notes.mms)
- [AVC constraint task](../task/xr-head-neck-bend-and-body-offset.md)
- [Declarative transform math](../draft/declarative-transform-math.md)
- [Named value components and native execution](../draft/declarative-vector-operator-components.md)
- [OpenXR reference diagram](../../examples/0-9-x-openxr-flowchart.mms)

The diagram uses authored flex rows within independent movable accordions,
orthogonal geometric connectors, selectable cards, and a persistent detail panel.
Group-local wires belong to the factory-owned body, so they disappear along with
the cards when minimized. Updating a selection replaces only the detail body;
all draggable panel roots and titles stay alive.
