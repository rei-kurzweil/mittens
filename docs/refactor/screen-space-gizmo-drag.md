
# True screen-space gizmo dragging (proposal)

This doc is *separate* from `docs/refactors/gesture-refactor.md`.

It explores an alternative mapping for the mode currently named `StartPlaneProjection` (formerly `ScreenSpaceCoords`).
Today that mode is really “project onto a drag-start plane using the pointer ray”. That works well in many cases, but it can feel oddly inconsistent from some camera angles because the mapping depends on the ray/plane geometry (and can become ill-conditioned when the ray grazes the plane).

The idea here is to offer a mode that is *actually screen-space driven*:
- use the pointer delta in pixel coordinates as the primary input
- still do enough 3D math to decide sign (“left vs right”, “up vs down”) consistently for each gizmo handle type (axis vs ring)
- keep behavior stable under camera orbit, without requiring continuous hit tests

## Goals

- Axis translation drags should feel like a 1D slider aligned to the on-screen axis.
- Ring rotation drags should feel like a 1D dial around the on-screen ring.
- Sign should be consistent (no “sometimes dragging left rotates clockwise, sometimes counter-clockwise” surprises).
- The mapping should be device-agnostic in principle (mouse, touch, XR), but this doc focuses on mouse/cursor screen deltas.

## Non-goals

- This does *not* replace raycasts for selection, hover, or drag start.
- This does *not* claim to be physically “correct” in world space; it is an editor UX mapping.
- This doc doesn’t decide where the mapping lives (Gesture vs Gizmo); it just sketches math.

## Why the current plane-projected approach can feel inconsistent

The current `StartPlaneProjection` behavior is effectively:

1. On `DragStart`, capture a plane $(P_0, n)$.
   - In the current implementation, $n$ is derived from the drag-start ray direction.
2. On `DragMove`, intersect the *current* pointer ray with that plane.
3. Convert the intersection point delta into `delta_world`, then the gizmo projects onto the axis.

This can get weird when:
- the pointer ray becomes nearly parallel to the plane (small cursor changes produce huge world deltas)
- the “captured normal” isn’t meaningfully related to the gizmo constraint (especially for axis/ring handles)
- after converting to `delta_world`, projecting onto the axis can flip sign depending on how the ray/plane mapping behaved

So: the *update policy* (don’t require continuous hits) is good, but the *mapping surface* (start-ray normal) isn’t always the best fit for gizmo constraints.

## Proposal: make gizmo handles screen-driven

Key idea: once a handle is grabbed, treat the cursor delta $(\Delta x, \Delta y)$ in pixels as the primary driver.

The handle type (axis vs ring vs plane) determines:
- which 1D scalar we extract from $(\Delta x, \Delta y)$
- how we turn that scalar into a world-space translation/rotation

This keeps the “don’t require continuous hits” property, but avoids ray/plane intersection instability during the drag.

### Common drag-start cached values

On drag start (for a specific handle), cache:

- `pivot_world`: gizmo origin / rotation pivot in world space
- `pivot_screen`: projection of `pivot_world` to screen pixels
- `axis_world`: unit axis in world (for axis/ring handles)
- `camera_basis_world`: `camera_right_world`, `camera_up_world`, `camera_forward_world`
- `viewport`: width/height in pixels
- enough camera projection info to estimate “world units per pixel” at the pivot depth

Then on drag move:
- compute `mouse_delta_pixels = current_mouse - prev_mouse` (or from drag start)
- map to a 1D scalar `s_pixels`
- convert `s_pixels` into world delta (translation or angle)

## Axis translation: “screen-axis slider” mapping

We want: dragging along the *on-screen direction* of the axis moves the gizmo along that axis.

### Step 1: compute the on-screen axis direction

Project a short segment of the axis into screen space:

- choose a small world length $L$ (e.g. 0.25m in gizmo scale space, or any stable constant)
- `p0 = project(pivot_world)`
- `p1 = project(pivot_world + axis_world * L)`
- `axis_dir_screen = normalize(p1 - p0)`

If `|p1 - p0|` is tiny, the axis is close to the camera forward direction and is nearly a point on screen (degenerate). In that case, fall back to a different mapping (see “Degeneracy + fallbacks”).

### Step 2: extract a 1D pixel delta

Option A (best feel): use both x and y:

- $s_{px} = \Delta\vec{m} \cdot \hat{a}_{screen}$

where $\Delta\vec{m} = (\Delta x, \Delta y)$ and $\hat{a}_{screen}$ is `axis_dir_screen`.

Option B (what you described: “either x or y depending on axis”):

- pick the dominant screen component:
  - if `abs(axis_dir_screen.x) >= abs(axis_dir_screen.y)` use X
  - else use Y
- then:
  - if X: $s_{px} = \Delta x \cdot \operatorname{sign}(\hat{a}_{screen}.x)$
  - if Y: $s_{px} = \Delta y \cdot \operatorname{sign}(\hat{a}_{screen}.y)$

Option B gives a very “CAD-like” feel: the drag is driven purely by one screen axis, but the sign still respects the gizmo’s on-screen orientation.

### Step 3: convert pixels to world units along the axis

We need a “world units per pixel” scale at the pivot depth.

Two common approaches:

#### 3A) Unproject-at-pivot-depth (hybrid, robust)

Even if we call this “screen-space mapping”, it’s fine to use *one* unprojection at drag start to get a stable scale.

Compute a world-space delta corresponding to a 1-pixel move along `axis_dir_screen` at the pivot depth:

- `w0 = unproject(p0_screen, depth = pivot_depth)`
- `w1 = unproject(p0_screen + axis_dir_screen * 1px, depth = pivot_depth)`
- `delta_world_1px = w1 - w0`
- `axis_world_per_px = dot(delta_world_1px, axis_world)`

Then:

- `delta_axis_world = axis_world * (s_px * axis_world_per_px)`

This makes translation feel consistent under perspective.

#### 3B) Analytic scale from FOV + depth (fast)

For perspective projection, approximate:

- $world\_per\_px\_y \approx \dfrac{2 \cdot z \cdot \tan(fovy/2)}{viewport\_height}$
- $world\_per\_px\_x \approx world\_per\_px\_y \cdot aspect$

Then convert $s_{px}$ into meters using either x/y scale depending on Option B, or a blend for Option A.

### Why this fixes sign weirdness

The sign comes from the *projected axis direction* (screen-space geometry), not from how a ray happens to intersect a captured plane.

As the camera orbits, `axis_dir_screen` rotates smoothly, so “drag along the axis” stays intuitive.

## Ring rotation: “screen-angle dial” mapping

We want: dragging around the pivot in screen space rotates around the ring axis in world space.

### Option A: signed screen-space angle around the projected pivot

On drag start:

- `pivot_screen = project(pivot_world)`
- `v0 = normalize(mouse_start - pivot_screen)`

On drag move:

- `v1 = normalize(mouse_current - pivot_screen)`
- compute signed 2D angle:

$$
\theta = \operatorname{atan2}(\text{cross2}(v0, v1), \text{dot}(v0, v1))
$$

where `cross2(a,b) = a.x*b.y - a.y*b.x`.

Then apply rotation around `axis_world`:

- `delta_rot = quat(axis_world, theta * sensitivity)`

#### Fixing clockwise/counter-clockwise consistency

2D signed angle is defined relative to screen coordinates. To keep “drag clockwise rotates clockwise” consistent regardless of whether the axis points toward or away from the camera, apply a sign correction based on axis direction in view space.

One simple rule:

- transform the axis into view space: `axis_view = (view_matrix * axis_world)`
- if `axis_view.z > 0` then negate `theta`

This compensates for the fact that rotating about an axis that points “toward the camera” reverses apparent orientation compared to one that points “away”.

(Exact sign depends on handedness and the engine’s view conventions; this is a place to add a small unit test / debug print and tune once.)

### Option B: keep the ring physically grounded (ray-plane) but stabilize it

If we decide pure screen-angle is too “detached”:

- intersect the pointer ray with the ring plane (plane normal = `axis_world`, point = `pivot_world`)
- compute `start_vec` and `current_vec` in that plane
- signed angle via $\theta = \operatorname{atan2}(\hat{n} \cdot (a \times b), a \cdot b)$

This is still ray-based, but it’s tied to the ring’s actual constraint plane (not the start-ray normal), so it tends to be much more consistent than the current “plane normal = start ray” mapping.

You can still use screen-space angle as a fallback when ray-plane becomes ill-conditioned.

## Plane translation handle: true screen-plane panning

For a “move in camera plane” handle (or for a general “screen space translate” tool), a straightforward mapping is:

- compute world-per-pixel scales at pivot depth (as above)
- `delta_world = camera_right_world * (dx * world_per_px_x) + camera_up_world * (dy * world_per_px_y)`

This is the most literal meaning of “screen space” for translation.

## Degeneracy + fallbacks

Any screen-space mapping will have degenerate camera angles:

- axis translation when the axis projects to ~0 length on screen (axis near camera forward)
- ring rotation when the pivot is behind the camera or the pivot projection is unstable

Suggested fallback policy:

- If the handle’s projected direction magnitude is below a threshold:
  - fall back to ring/axis mapping that uses a 3D constraint plane (axis plane or ring plane)
  - or fall back to “closest point between ray and axis line” for translation

A good UX heuristic is:
- prefer the simplest screen mapping when well-conditioned
- gracefully switch to a 3D mapping when the screen mapping is near-degenerate

## Where this mapping might live (architecture note)

Right now `GestureSystem` outputs `delta_world`.

If we want handle-specific screen mappings, it may be cleaner to have `GestureSystem` output *raw pointer deltas* alongside world info, e.g.:

- `DragMove { ..., delta_pixels: [f32; 2], ... }`

Then `GizmoSystem` (which knows which handle is grabbed) can choose:
- axis slider mapping
- ring dial mapping
- plane pan mapping

This avoids baking gizmo-UX policy into general gesture recognition.

## Suggested next experiments

1. Axis translation prototype:
   - implement Option B (choose X or Y) first; it’s very close to what you described
   - add a debug overlay printing `axis_dir_screen`, chosen component, and `s_px`

2. Ring rotation prototype:
   - implement Option A (screen-angle) and tune the `axis_view.z` sign correction
   - compare to Option B (ray-plane in ring plane) to see which feels better

3. Keep the existing plane-projected mode, but rename it:
   - it’s still useful (especially for “free drag in 3D”), it just shouldn’t be called “screen space”


