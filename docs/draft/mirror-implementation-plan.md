# Mirror Implementation Plan

## Overview

Planar mirrors in `cat-engine` are implemented as ad hoc offscreen render passes. Each visible mirror derives a reflection camera from the active viewer, renders the scene into a transient texture, and then makes that texture available for sampling during the main scene pass.

## Staged MMS Implementation

### Stage 1: Basic Renderable Sampling (v1)
In the first version, we use the standard `Renderable` and `Texture.render_image()` bridge. This requires no changes to the layout or style systems.

```mms
T {
    // 1. Declare the mirror behavior
    Mirror { quality: 1024 }
    
    // 2. Sample the generated texture in a standard quad
    R.square() {
        Texture.render_image("capture.mirror.self")
    }
}
```

### Stage 2: Styled Background (v2)
As an ergonomic improvement, we can add `background_image` support to the `Style` component. This would allow mirrors to work seamlessly with the layout system's background quads.

```mms
T {
    Mirror { quality: 1024 }
    Style { 
        width(2.0)
        height(2.0)
        // Future convenience property in Style
        background_image(Texture.render_image("capture.mirror.self"))
    }
}
```

## Architectural Components

### 1. `MirrorComponent` (Rust)
A simple marker component to identify mirror surfaces.

```rust
pub struct MirrorComponent {
    pub quality: i32,
    pub enabled: bool,
    // System-managed:
    pub(crate) capture_id: Option<String>,
}
```

### 2. `MirrorSystem` (Rust)
- **View Derivation**: Every frame, for each enabled `MirrorComponent`, compute the reflected `view` and `proj` matrices based on the active camera.
- **Texture Registration**: Assign a unique `capture.mirror.<id>` key to each mirror. If the author uses `.self` in MMS, the system resolves this to the specific mirror's ID.
- **Pass Scheduling**: Inform the renderer about the required offscreen passes.

### 3. Renderer Updates (`vulkano_renderer.rs`)
- **`RenderView` Abstraction**: Move away from hardcoded `Window`/`Xr` eyes toward a list of generic `RenderView` objects.
- **Dependency Ordering**: Ensure mirror passes execute *before* the main pass.
- **Resource Management**: Reuse color/depth attachments across frames to avoid constant allocations.

## Key Constraints

1. **No Recursion**: In v1, mirrors do not render other mirrors. A mirror surface in a reflection will appear as a solid color or a fallback texture.
2. **Oblique Clipping**: (v2) Add an oblique near-plane to the reflected camera's projection matrix to prevent rendering geometry behind the mirror.
3. **Culling**: Only render mirrors that are visible in the main camera's frustum.
