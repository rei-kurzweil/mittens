# Make `RendererSettings.window_size` a Suggestion, Not a Constraint

## The Problem

`RendererSettingsComponent.window_size` is supposed to set the window size, but
currently it **forces** the window to that size on **every frame**.

The culprit is in `src/engine/windowing.rs:130-141`:

```rust
WindowEvent::RedrawRequested => {
    if let (Some(window), Some(universe)) = (&self.window, self.universe.as_ref()) {
        if let Some([width, height]) = universe.preferred_window_size() {
            let current = window.inner_size();
            if current.width != width || current.height != height {
                let _ = window.request_inner_size(winit::dpi::LogicalSize::new(
                    width as f64, height as f64,
                ));
            }
        }
    }
    // ...
}
```

This means: set `window_size(640, 480)` → try to resize the window → next
`RedrawRequested` snaps it right back to 640×480. The window is effectively
non-resizable any time a `RenderSettings.window_size` is present.

## Root Cause

The data flow conflates *initial suggestion* with *runtime constraint*:

```
RendererSettingsComponent.window_size
  → VisualWorld.preferred_window_size  (stored forever)
    → used at window creation (correct)
    → used every RedrawRequested frame (INCORRECT — this is the bug)
```

There's no mechanism to distinguish "this is what I'd like the window to be"
from "the window must always be this size."

## Design

### Core Idea

`window_size` is a **suggested initial size**, just like `with_inner_size()` in
winit. Once the window is created, the user can resize freely. The stored value
is consumed once at window creation, then becomes irrelevant.

No new API surface is needed. The change is subtractive.

### Changes Required

#### 1. Remove the `RedrawRequested` enforcement loop

Delete lines 131–141 of `windowing.rs` (the `universe.preferred_window_size()`
block). After this, `preferred_window_size()` is only called in `resumed()` for
the initial window size.

#### 2. Clear `preferred_window_size` after initial use (optional but clean)

Add a `take_preferred_window_size()` to `VisualWorld` / `Universe` that returns
`Option<[u32; 2]>` and sets the field to `None`. Call it from `resumed()` instead
of `preferred_window_size()`. This prevents stale state from lingering.

```rust
// VisualWorld
pub fn take_preferred_window_size(&mut self) -> Option<[u32; 2]> {
    self.preferred_window_size.take()
}
```

```rust
// Universe
pub fn take_preferred_window_size(&mut self) -> Option<[u32; 2]> {
    self.visuals.take_preferred_window_size()
}
```

In `resumed()`:
```rust
let preferred_window_size = self
    .universe
    .as_mut()
    .and_then(|universe| universe.take_preferred_window_size())
    .unwrap_or([1024, 768]);
```

#### 3. (Optional) Rename to signal intent

Rename `preferred_window_size` → `initial_window_size` across:
- `VisualWorld` field, getter, setter
- `Universe::preferred_window_size()` → `Universe::take_initial_window_size()`
- `RendererSettingsComponent::with_window_size()` docs

This is pure renaming — no behavior change — but makes the semantics
unambiguous for future readers.

### After the Fix

Data flow becomes linear, no loop:

```
RendererSettingsComponent.window_size
  → VisualWorld.set_preferred_window_size() / set_initial_window_size()
  → consumed once in resumed() for Window::with_inner_size()
  → field cleared to None
  → window is freely resizable via winit's resize handles
  → WindowEvent::Resized → universe.resize_renderer() → swapchain recreates
```

### What About People Who Want a Fixed-Size Window?

Not addressed here, but the escape hatch already exists:
`Window::with_resizable(false)` in `windowing.rs`. If someone genuinely wants
a non-resizable window, that's a separate feature (e.g., a
`RendererSettings.resizable(false)` flag that gates the
`.with_resizable()` call).

## Files Touched

| File | Change |
|------|--------|
| `src/engine/windowing.rs` | Remove `RedrawRequested` enforcement block; use `take_` in `resumed()` |
| `src/engine/universe.rs` | Add `take_preferred_window_size()`, optionally rename |
| `src/engine/graphics/visual_world.rs` | Add `take_preferred_window_size()`, optionally rename |
| `src/engine/ecs/component/renderer_settings.rs` | Optionally update doc comment |
