# Path finding system

`PathFindingSystem` answers one question for scripts: of these goals, which is
cheapest to walk to from here, and by what route. Choosing which objects are
goals (loaded weapons, cover, a seat) stays in MMS. The system only measures
and routes.

The first user is the interceptor NPC, whose A* currently lives in
`assets/components/targets/interceptor_paths.mms`. That file becomes data plus
a small wrapper once this lands.

## MMS API

A graph is a `PathGraph` component. Building it is the up-front work, and
queries are synchronous methods on it, like `UdpSocket.ready()`.

```mms
let graph = PathGraph.new({
    nodes = [[-5.9, 28.0, -13.5], [5.9, 28.0, -13.5]]
    obstacles = [[-2.9, 2.65, -3.05, 3.2]]
    bounds = [-13.3, 13.3, -16.3, 16.3]
}) { name = "interceptor_paths" }

let route = graph.route(here, goal)
// { points = [[x, y, z], ...] length = 12.4 } or null

let best = graph.nearest(here, [{ point = grip_a cost = 0.4 }, grip_b])
// { index = 1 points = [[x, y, z], ...] length = 9.1 } or null
```

### `PathGraph.new(table)`

| Field | Value |
|---|---|
| `nodes` | waypoints, `[x, y, z]`. Usually the corners of padded obstacles. |
| `obstacles` | ground-plane rectangles, `[min_x, max_x, min_z, max_z]`, already padded by the walker's radius. |
| `bounds` | the walkable rectangle, `[min_x, max_x, min_z, max_z]`. |

The constructor validates the table and builds the edges between nodes once.
Later queries never rebuild them.

### `graph.route(here, goal)`

Returns the shortest walk from `here` to `goal`, or `null` if there is none.
It behaves exactly like `nearest(here, [goal])` without the `index` field.

### `graph.nearest(here, goals)`

Each goal is either a point `[x, y, z]` or `{ point = [x, y, z] cost = n }`.
`cost` is a nonnegative extra amount added to that goal's walking length. It
defaults to `0`. The armory uses it for the distance from a stand's approach
point to the grip.

Returns `{ index, points, length }` for the goal with the lowest
`length + cost`, or `null` if no goal is reachable or `goals` is empty. Equal
totals pick the lowest index. `index` is that goal's position in `goals`.

### Results

- `points` lists the waypoints after `here`, ending with the goal's own point.
  It is never empty. A goal equal to `here` gives `[goal]` with length `0`.
- `length` is the walking length along `points` in metres, measured in X and Z
  only. It does not include `cost`.
- Waypoints keep the `y` they were given (node `y` or goal `y`). The system
  never invents heights.

## Geometry

- Everything is world space. The graph does not follow its entity's
  transform.
- Walking is on one level, so only X and Z matter. `y` is carried through but
  never tested.
- A straight segment between two points is walkable when its far end is inside
  `bounds` and it touches no obstacle. Touching an edge or corner counts as
  blocked. Only the far end is checked against `bounds`, because the rectangle
  is convex. This also keeps a walker who has been pushed just past the edge
  able to walk back in.
- A start point inside an obstacle has no walkable segments, so it gets no
  route. This matches the MMS version. Unsticking a walker that was pushed
  into padding is out of scope.

## Search

- The edges between nodes are computed by the constructor and weighted by
  their X/Z distance.
- For each query, `here` and every goal become temporary nodes. They connect
  to every node they can reach in a straight line, and `here` connects
  directly to each goal it can reach.
- The search is A* from `here`. The heuristic for a node is the lowest
  `distance(node, goal) + cost` over all goals, which never overestimates.
  The search stops the first time it takes a goal off the open set.
- One `nearest` call is one search, however many goals it is given. Today's
  MMS armory runs one search per weapon.
- Each graph keeps scratch buffers (open heap, scores, parents, the temporary
  node's edges) and reuses them between queries. A query allocates only the
  MMS result.

## Limits and errors

- Up to 1024 nodes, 1024 obstacles and 256 goals per query.
- Every number must be finite. Obstacle and bounds rectangles need
  `min <= max` on each axis. A negative `cost` is an error.
- Bad input fails the call with `PathGraph.new`, `PathGraph.route` or
  `PathGraph.nearest` and says what was wrong. Nothing is clamped or silently
  dropped.
- Calling a method on a removed graph fails with a stale-graph error.

## Lifetime

`PathGraph` is a runtime-only component. Its entity owns it, it is removed
with that entity, and it is not serialized. Scripts rebuild it when the scene
loads. `PathFindingSystem` has no per-frame work in this version. It is the
query service over `PathGraph` components, and the place later features will
go.

## Interceptor migration

- `interceptor_paths.mms` keeps its node, obstacle and bounds data and builds
  a `PathGraph`.
- Its `plan(here, goal)` wraps `graph.route` and keeps the
  `{ count, point(i), length }` shape that `target_practice.mms` follows.
- `retreat` keeps its current choice of cover point and calls `graph.route`.
- `mitten_armory` builds one goal per loaded weapon nobody is holding, with
  the grip distance as `cost`. It calls `graph.nearest` once and maps `index`
  back to the weapon. The per-weapon search loop and the MMS A* are deleted,
  with no fallback.

## Validation

- **Rust unit tests on the interceptor data:**
  - a direct route;
  - routes around the hull and through a door;
  - an unreachable goal, out of bounds or inside an obstacle;
  - `nearest` choosing the goal that is closer to walk to over one that is
    closer in a straight line;
  - `cost` changing the choice;
  - ties choosing the lowest index;
  - each validation error.
- **Script test:** build a `PathGraph` from MMS and check one `route` and one
  `nearest` result.
- **Benchmark:** `examples/path-finding-benchmark.mms`, recorded in
  `docs/benchmarks/path-finding.md`. The route fingerprints must match the MMS
  A* run, and the timings are compared on the GTX 1080 desktop and the
  Focus 3.

## Not in this version

- Obstacles from collidables, such as a `PathObstacle` marker component.
- Generating corner nodes automatically.
- Moving obstacles, re-planning, and updating the graph when a destroyable
  breaks.
- More than one level (stairs, ramps, links between floors).
- Steering around other walkers.
