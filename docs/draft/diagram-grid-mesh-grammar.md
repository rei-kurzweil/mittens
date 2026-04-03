# Diagram grid mesh grammar

This note is about authoring diagrams (render graphs, dataflow charts, editor graphs, etc.) with
cat-engine geometry instead of hand-authored SVG paths.

## Core idea

Treat a diagram as a **grid of cells plus adjacency metadata**, not as a set of arbitrary curves.

For a first pass, each occupied cell stores:

- a node/body tile, or
- a line tile with connections in the cardinal directions: `N`, `E`, `S`, `W`

That gives a natural 4-bit connectivity mask:

- `0000` = empty
- `0101` = vertical
- `1010` = horizontal
- `0011` / `0110` / `1100` / `1001` = corners
- `0111`, etc. = T junctions
- `1111` = cross

For many diagrams, this lookup-table approach is already enough. It is deterministic, easy to
serialize, and easy to regenerate.

## Why this seems like the right abstraction

- The authored intent is usually topological: “this box connects to that box”, not “place this
  exact Bézier curve”.
- It maps well onto current engine strengths: instanced meshes, tile-like reuse, and ECS-friendly
  authored structure.
- It keeps line routing and visual styling separate.

That suggests two layers:

- **routing/topology layer** — which cells and edges are occupied
- **presentation layer** — square corners, rounded corners, stroke width, end caps, arrows, labels

## Rounded corners

Rounded corners can be handled as a style variant of the same adjacency grammar.

For an orthogonal corner cell, instead of selecting a hard 90° elbow mesh, select a rounded-corner
mesh. There are a few plausible implementations:

- a preauthored corner tile mesh for each turn orientation
- a procedural corner mesh built from a radius and stroke width
- a quarter-annulus-like stroke segment, optionally with short straight stubs entering/leaving the
  cell

The quarter-annulus idea is especially attractive because it matches the geometry we conceptually
want: a ring sector with thickness.

If the visual language stays orthogonal and grid-based, a small tile set is probably enough:

- straight horizontal / vertical
- 4 corner orientations
- 4 T-junction orientations
- cross
- end caps / arrowheads

If we want variable corner radius or stroke width at runtime, then a more procedural mesh factory
starts to make more sense than a fixed asset set.

## Relation to existing gizmo / picking thoughts

The engine already has discussion around rings, annuli, and narrow-phase pick shapes in:

- `docs/analysis/raycast-circles-and-cones.md`
- `docs/spec/bvh-and-raycast.md`

That is useful precedent: “annulus-like geometry” is already a meaningful concept in the engine,
even though this diagram use case is about rendering/layout rather than picking.

## Selection rule

The basic mesh-selection algorithm can stay very simple:

1. Compute the cardinal-neighbor mask for each occupied line cell.
2. Choose a tile family from that mask: straight, corner, tee, cross, cap.
3. Apply a style variant: square, rounded, heavy, dashed, arrowed.
4. Derive rotation from the mask/orientation.

This is closer to autotiling than to a full Wave Function Collapse system.

WFC-style constraints may still be useful later if we want partially procedural diagram generation,
but for authored render-graph diagrams a deterministic adjacency lookup is probably the right v1.

## Likely authoring model

One plausible model is:

- node boxes authored explicitly
- line routes authored as cell chains or edge chains
- renderer converts those routes into tile instances
- labels remain text components anchored to cells / node bounds

That would let us build diagrams in MMS or regular ECS content with reusable engine-native pieces,
while still exporting to SVG later if needed.

## Open questions

- Do we want the source of truth to be grid cells, or graph edges with an auto-router that fills
  the grid?
- Is a fixed rounded-corner tile set visually sufficient, or do we want runtime radius control?
- Should arrows be separate overlay meshes, or integrated into terminal tile variants?
- Do we want nine-slice style node boxes so diagram panels and line tiles share the same grid
  grammar?