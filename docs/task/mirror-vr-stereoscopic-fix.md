# Mirror VR stereoscopic fix

## Status

Draft task.

## Root cause

In `universe.rs:415-427`, the rendering order is:

1. `render_xr()` — renders XR eye scenes
2. `render_visual_world()` — renders window scene + **mirror offscreen targets**

Mirror capture targets (mono + stereo) are rendered **inside** `render_visual_world()` (vulkano_renderer.rs:3785). By the time XR eyes are rendered at step 1, the stereo mirror captures have not been produced yet. The late retarget step (`retarget_mirror_surface_textures_for_render_view` at vulkano_renderer.rs:2287) tries to swap the mirror surface texture to `stereo.{eye}` during XR rendering, but the runtime texture handle is either absent or a 1×1 black placeholder from `render_to_texture_system`.

Result: XR eyes render the mirror surface with a stale/missing texture instead of the correct per-eye reflection.

The problem only manifests when a window camera is active because `render_visual_world` (and therefore mirror capture rendering) is skipped entirely when there is no window camera (`universe.rs:422`).

## Sync and timing

Both paths use the **same graphics queue** and **block on GPU completion** via `fence.wait(None)` — no additional barriers needed.

**Desktop is already throttled to headset rate.** The current order (`render_xr` → `render_visual_world`) already has `wait_frame()` (openxr_system.rs:1542) blocking at the top of the XR path, which stalls the entire `render()` call until the headset is ready. Reordering doesn't change this — the desktop framerate is already at the mercy of the headset's vsync when XR is active.

**Correctness**: The mirror capture data (camera positions, plane equations) is computed during the ECS tick, which runs before `render()`. So whether mirror captures are rendered before or after `wait_frame()`, the source data is the same — the scene state doesn't change within `render()`.

## Fix plan

### 1. Extract mirror capture rendering from `render_visual_world`

Create a new public method `render_mirror_captures` on `VulkanoRenderer` that performs only the mirror offscreen rendering loop (currently lines 3785-3886 of vulkano_renderer.rs).

### 2. Remove mirror rendering from `render_visual_world`

Have `render_visual_world` call the shared mirror rendering helper, or skip the mirror block since it will have already been rendered.

### 3. Reorder rendering in `universe.rs`

Change `render()` to:

```rust
// 1. Render mirror offscreen captures first, so XR and window can consume them.
self.renderer.render_mirror_captures(&mut self.visuals)?;

// 2. Render XR eyes (mirror textures are now available).
self.systems.openxr.render_xr(&self.world, &mut self.visuals, &mut self.renderer);

// 3. Render window scene (mirror textures are now available).
if self.systems.camera.has_active_window_camera() {
    self.renderer.render_visual_world(&mut self.visuals)?;
}
```

This ensures that when `render_xr` runs and `retarget_mirror_surface_textures_for_render_view` looks up `stereo.0` / `stereo.1`, those textures have already been rendered and published.

### 4. Handle the no-window-camera case

`render_mirror_captures` should still run when there is no window camera but XR is active. The check at `universe.rs:422` currently gates the entire mirror rendering. The new ordering naturally fixes this — mirror captures are rendered unconditionally, and the window render is gated separately.

## Files to change

| File | Change |
|------|--------|
| `src/engine/graphics/vulkano_renderer.rs` | Extract mirror loop into `render_mirror_captures()`, remove from `render_visual_world` or make it call the helper |
| `src/engine/universe.rs` | Reorder: mirror captures → XR → window |

## Open questions

- Should `render_mirror_captures` be a no-op when there are no mirrors? (Probably yes, match current skip-at-`view_count==0` behavior.)  
- Does `render_visual_world` still need to call `apply_pending_runtime_texture_updates` before mirror rendering? If mirror is extracted, it needs its own call.
