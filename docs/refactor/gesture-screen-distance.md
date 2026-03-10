# Gesture: screen-distance driven rotation (1D slider)

## Goal

Make rotational transform gizmo interactions depend only on how far the pointer has moved in screen space since the initial click/press, rather than relying on world-space plane/ring intersections.

Concretely:

- Rotation handles (X/Y/Z rings) should use a **screen-space distance slider**.
- Translation handles can keep using the existing **world-plane projection** (stable drag plane captured at DragStart) because translation is inherently world-space.

This document describes the refactor that restores/standardizes the “1D slider” behavior for all rotational gizmo sub-components.

## Current state (before refactor)

Drag gestures come from the gesture pipeline as events:

- `DragStart` includes `screen_pos_px: Option<(f32,f32)>`
- `DragMove` includes `screen_pos_px` and `screen_delta_px`

`TransformGizmoSystem::on_drag_move` currently supports two rotation modes, selected by an ancestry marker component:

- `GestureCoordType::WorldPlane`: computes a signed angle from the change in hit-point around a pivot, projected onto the plane orthogonal to the rotation axis.
- `GestureCoordType::ScreenSpace1DSlider`: maps `screen_delta_px` to radians.

However, the rotation rings were being spawned with `GestureCoordType::WorldPlane`, so the slider path was effectively not used for the standard X/Y/Z rotational handles.

## Problem with world-plane/ring intersection rotation

The plane/ring intersection approach ties rotation to:

- a stable drag plane / ray-plane intersection,
- accurate hit-point updates,
- camera configuration and handle geometry.

It also interacts poorly with “continue dragging even when the cursor leaves the handle” because the world-plane math is sensitive to the chosen projection.

By contrast, for rotation we often want an editor-like interaction:

- click a ring,
- drag the mouse anywhere,
- the rotation continues as long as the mouse keeps moving away from (or back toward) the starting point.

## Proposed interaction model

### Core mapping

For rotational gizmo handles in slider mode, we integrate *per-move* screen deltas.

On each `DragMove`, take the screen-space delta in pixels:

$$
\Delta p = (dx, dy)
$$

Then compute an incremental angle:

$$
	heta_{delta} = (dx + dy) \cdot k
$$

where $k$ is a sensitivity constant (`radians_per_px`).

We also keep an accumulated angle (mainly for bookkeeping/debug):

$$
	heta_{acc} \leftarrow \theta_{acc} + \theta_{delta}
$$

This avoids “flip” behavior that can happen when the mapping depends on an origin/reference vector (e.g. when gesturing across the origin of a displacement vector).

### Fallback behavior

If `screen_pos_px` isn’t available (e.g. non-screen pointers like XR), the slider path does nothing. In that scenario, rotation can continue using `WorldPlane` if desired.

## Implementation notes

### Data that must be stored per drag

The gizmo component needs a small amount of runtime-only state:

- `active_drag_slider_last_angle: f32` (treated as an accumulated slider angle)

This state is reset on `DragStart` and cleared on `DragEnd`.

### Applying to all rotational sub-components

To ensure every rotational gizmo handle uses the same interaction style:

- Spawn X/Y/Z rotation rings under a `GestureCoordType::ScreenSpace1DSlider` marker.
- Keep translation handles using the world-plane / ray-plane projection flow.

### Tunables

- `radians_per_px` controls sensitivity.

## Follow-ups

Potential improvements (optional):

- Make `radians_per_px` a configurable field (editor setting or component field).
- Make sign selection camera-aware (e.g. consistent clockwise behavior on screen).
- Support touch/trackpad scaling (DPI/viewport normalization).
