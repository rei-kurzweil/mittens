# Render graph: post-processing

This document designs the opt-in post-processing system for cat-engine, covering bloom and depth-of-field (bokeh) as the first two effects. It also covers the emissive pipeline refactor needed to support them cleanly.

**Scope:**
- Emissive as a float intensity + dedicated pipeline
- BloomComponent and BokehComponent design
- RenderGraphComponent as the scene-facing opt-in marker
- Render graph changes (intermediate images, passes, ordering)
- XR considerations

**Not in scope:**
- SSAO, SSR, TAA, tone mapping (noted as future work)
- Full render-to-texture capture systems (mirrors, portals, monitors)

**Related:**
- `docs/spec/render-phases.md` — existing phase ordering
- `docs/spec/render-to-texture.md` — implemented runtime-texture publication bridge
- `docs/spec/renderer-stats-component.md` — renderer diagnostics
- `docs/spec/render-graph-pipeline.svg` — diagram: base pipeline
- `docs/spec/render-graph-pipeline-post-processing.svg` — diagram: post-processing render graph
- `src/engine/graphics/vulkano_renderer.rs` — current render loop
- `src/engine/graphics/vulkano_swapchain.rs` — swapchain/depth image setup
- `assets/shaders/toon-mesh.frag` — current emissive branch

---

## Bloom / overlay behavior

The current target semantics are:

- **Opaque** geometry occludes the emissive extraction pass.
- **Cutout** geometry occludes the emissive extraction pass.
- Regular transparent geometry is still a separate question and is not part of this change.
- **Overlay** should remain visible in the main color path and therefore be affected by bloom in
    the final composite.
- **Overlay should not contribute to the emissive-source extraction pass** for this change.

This gives the desired artistic result:

- bloom glow is clipped by solid scene geometry (`Opaque` / `Cutout`)
- overlay visuals still receive the final bloom treatment because they are present in main color
- overlay does not become a new bloom source just by existing in the overlay phase

An important consequence is that the depth used by emissive extraction must reflect the
scene-depth result from opaque/cutout rendering, not an overlay-only depth clear/rewrite.

---

## 1. Current pipeline state

Everything renders in a **single `begin_rendering()` → `end_rendering()` dynamic-rendering scope**:
- Color: MSAA optional (4x), resolves to sRGB swapchain image
- Depth: `D32_SFLOAT`, MSAA-matched, **store op = `DontCare`** (discarded after render)
- Emissive: per-instance `u32` flag on `VisualInstance`; in the fragment shader it just early-returns before lighting
- No intermediate images, no readback, no post-processing passes

The depth being discarded is the critical constraint for any depth-dependent post-processing (bloom occlusion, depth-of-field).

---

## 2. Emissive pipeline refactor

### 2.1 Why emissive needs its own pipeline

Currently the toon-mesh fragment shader has a single branch:
```glsl
if (mat.emissive != 0u || v_emissive != 0u) {
    f_color = vec4(base, base_rgba.a);
    return;
}
// ... rest of lighting (SSBO reads, quantization, etc.)
```

This causes **intra-warp divergence** whenever emissive and non-emissive surfaces land in the same draw batch: all 32–64 fragments in the warp must execute the lighting path even if only one fragment is non-emissive.

More importantly for bloom: the emissive prepass needs to draw *only emissive instances*. With a shared shader + flag, that requires scanning all instances and re-sorting. With a dedicated `MaterialHandle`, emissive objects already sort into their own draw batches — the emissive prepass is just "draw batches that use `EMISSIVE_TOON_MESH`."

### 2.2 New emissive material handles

Add two new `MaterialHandle` variants:
- `EMISSIVE_TOON_MESH` (unskinned emissive)
- `SKINNED_EMISSIVE_TOON_MESH` (skinned emissive, same vertex shader as `SKINNED_TOON_MESH`)

Both use a new fragment shader with **no lighting code at all**:
```glsl
// emissive-toon-mesh.frag (conceptual)
layout(location = 0) out vec4 f_color;

void main() {
    vec4 base_rgba = texture(tex, v_uv) * v_color;
    float intensity = v_emissive_intensity;  // HDR float from instance buffer
    f_color = vec4(base_rgba.rgb * intensity, base_rgba.a);
}
```

~10 lines, no SSBO reads, no loop over lights.

The `toon-mesh.frag` shader loses the `v_emissive` branch entirely — objects either use the emissive pipeline or the toon pipeline, never both.

### 2.3 EmissiveComponent: bool → float intensity

Currently:
```rust
pub struct EmissiveComponent {
    pub enabled: bool,
}
```

Proposed:
```rust
pub struct EmissiveComponent {
    pub intensity: f32,  // 0.0 = disabled, 1.0 = normal, >1.0 = HDR overbright for bloom
}
```

- `EmissiveComponent::on()` returns `intensity: 1.0`
- Old `enabled: true` = `intensity: 1.0`, `enabled: false` = `intensity: 0.0` (easy migration)
- When a component has `intensity > 0.0`, `RenderableSystem` assigns it the `EMISSIVE_TOON_MESH` material handle
- `VisualInstance.emissive` changes from `u32` to `f32` (the intensity)
- `InstanceData.i_emissive` changes from `u32` to `f32` — zero cost in the GPU vertex attribute

This change is backwards-compatible in behavior: existing emissive objects continue to appear unlit at intensity 1.0.

### 2.4 Draw sorting consequence

Since `MaterialHandle` is part of draw batch sorting, emissive objects automatically batch separately from toon objects. The batching requires no changes.

---

## 3. Component design

### RenderGraphComponent

A global scene marker that activates the post-processing path. One per scene (not per camera). When absent, the renderer takes the current fast path unchanged — no intermediate images, no extra passes, no overhead.

```rust
pub struct RenderGraphComponent {
    pub enabled: bool,
}
```

Authoring shape: add anywhere in the scene (typically a sibling of the camera rig or a scene root child):
```
SceneRoot
  ├── CameraRig { ... }
    ├── RenderGraphComponent
    │     ├── EmissivePassComponent { ... }  // optional
    │     ├── BloomComponent { ... }
    │     └── BokehComponent { ... }         // optional
  └── ...scene...
```

When `RenderGraphComponent` is present, the renderer:
1. Allocates an intermediate color image (not the swapchain directly)
2. Activates the stored-depth path if any child component requires it
3. Runs the child-specific passes after geometry

### BloomComponent

```rust
pub struct BloomComponent {
    pub intensity: f32,       // Additive blend strength (default 1.0)
    pub radius_ndc: f32,      // Bloom spread in NDC units (default 0.05; see below)
    pub emissive_scale: f32,  // Scales emissive intensity in the bloom prepass (default 1.0)
    pub half_res: bool,       // Blur at half resolution for performance (default true)
    pub source: BloomSource,  // Which geometry feeds the bloom prepass (default Emissive)
}

pub enum BloomSource {
    Emissive,   // Only EMISSIVE_TOON_MESH batches (default, cheapest)
    // Future: All, LuminanceThreshold(f32)
}
```

**`radius_ndc` to pixels conversion.** The renderer converts `radius_ndc` to a pixel
half-width at render setup time using:

```rust
fn ndc_radius_to_pixels(radius_ndc: f32, viewport_width: u32) -> u32 {
    ((radius_ndc * viewport_width as f32) / 2.0).round().max(1.0) as u32
}
```

`radius_ndc = 0.2` on a 1920-wide viewport → `192 px` Gaussian half-width.
`radius_ndc = 0.05` → `48 px` (a reasonable default for subtle bloom).

This makes bloom spread resolution-independent — a scene authored at 1080p looks the same
at 4K. The half-res path (`half_res = true`) halves the effective pixel count before
blurring, then upsamples; `radius_ndc` stays unchanged.

**MMS authoring form:**

```
RenderGraph {
    Bloom.radius(0.2).intensity(0.8) {
        quality = 0.5      // scales kernel sample count (0..1, default 1.0)
        EmissiveSource     // positional tag: bloom source = emissive geometry (default)
    }
    Bokeh { ... }
}
```

The chained `.radius(0.2).intensity(0.8)` desugars in the parser: `radius` becomes the
constructor call, `intensity` becomes a body `Call`. Both map to builder methods on
`BloomComponent`. `EmissiveSource` is a bare positional tag handled by `apply_positional`
for `BloomComponent` — it selects `BloomSource::Emissive` (the default, so it's mostly a
documentation hint today; future variants would be `AllSource`, `LuminanceSource(f32)`).

Requires: emissive objects using `EMISSIVE_TOON_MESH` material.
Does not require depth (occlusion of glow is an enhancement, see open questions below).

### BokehComponent

```rust
pub struct BokehComponent {
    pub focus_distance: f32,   // World-space focal plane (meters)
    pub aperture: f32,         // Controls CoC radius (larger = more blur)
    pub max_blur_radius: f32,  // Max circle-of-confusion radius in pixels (default 8.0)
}
```

Requires: stored depth buffer (triggers depth store + optional MSAA depth resolve).

---

## 4. Bloom pipeline

### 4.1 Emissive prepass

After the scene geometry needed for emissive occlusion is established, render emissive objects into
a dedicated HDR buffer:

- Format: `R16G16B16A16_SFLOAT` (HDR; allows values > 1.0)
- Size: full or half-resolution (`BloomComponent::half_res`)
- Clear: `(0, 0, 0, 0)`
- Depth test: ON if stored depth is available (reuse main depth attachment, no write); otherwise OFF
- Draw: only batches using `EMISSIVE_TOON_MESH` or `SKINNED_EMISSIVE_TOON_MESH`

For the intended behavior above, the reused depth should contain the occluding result of:

- opaque scene geometry
- cutout scene geometry

and should **not** be replaced by an overlay-only depth clear before emissive extraction.

**Depth occlusion for glow**: If the emissive prepass reuses the stored depth (read-only), bright objects behind walls won't bleed glow through them. If depth is not stored (e.g., no `BokehComponent`), glow bleeds through walls — acceptable for an initial implementation, can be addressed later by always storing depth when bloom is active.

**Performance note**: If there are no emissive instances in the scene this frame, skip the pass entirely.

### 4.2 Gaussian blur (two-pass separable)

Two full-screen passes on the emissive buffer:
1. Horizontal Gaussian blur → temp buffer
2. Vertical Gaussian blur → blurred emissive buffer

Kernel: separable Gaussian, width = `2 * blur_radius + 1`.

If `half_res = true`, both passes run at half resolution. The blur hides the resolution difference.

### 4.3 Bloom composite

Full-screen additive blend:
```
final_color = main_color + bloom_blurred * bloom_intensity
```

Because overlay remains part of `main_color` in the intended behavior, overlay visuals naturally
receive the bloom composite even when they do not contribute to the emissive extraction source.

If main color is sRGB (which it currently is), the additive blend needs to happen in linear space. This is an argument for keeping the intermediate image as a linear float format.

Output: main color buffer (updated in-place or written to a new buffer).

---

## 5. Depth-of-field (bokeh) pipeline

### 5.1 Depth requirements

Current depth: MSAA `D32_SFLOAT`, store op = `DontCare`. Changes required when `BokehComponent` is present:
- **Store op → `Store`** (retain depth after render)
- **If MSAA enabled**: allocate a single-sampled `R32_SFLOAT` image and resolve MSAA depth into it (Vulkan supports depth resolve via `VK_KHR_depth_stencil_resolve`)
- **If MSAA disabled**: use depth image directly as shader input

This depth image covers the full frame (all geometry phases). Since depth is cleared mid-frame for the overlay phase, the stored depth will be the opaque+transparent scene depth, not overlay.

### 5.2 CoC computation

Full-screen pass or compute shader:
- Input: stored depth image
- Camera data: near/far planes, focal length, focus distance, aperture
- Output: `R16_SFLOAT` circle-of-confusion radius image (pixels)

Simplified CoC formula (pinhole camera model):
```
depth_linear = (2 * near) / (far + near - depth_ndc * (far - near))
coc = abs(depth_linear - focus_distance) / depth_linear * aperture
coc_pixels = coc * viewport_width / sensor_width
```

Clamp CoC to `[0, max_blur_radius]`.

### 5.3 Bokeh blur

Variable-radius blur using the CoC image:
- Input: main color image, CoC image
- Output: blurred color image

Options (in order of quality/cost):
1. **Separable Gaussian with per-pixel radius** — fast, cheap, slightly incorrect but fine for game use
2. **Hexagonal bokeh kernel** — more photorealistic shape, GPU-heavy
3. **Dual-Kawase blur** — fast approximation, fewer passes

For the initial implementation, separable Gaussian with per-pixel radius is recommended.

### 5.4 DoF composite

Blend sharp vs blurred by CoC:
```
weight = saturate(coc / max_blur_radius)
final_color = lerp(sharp_color, blurred_color, weight)
```

Center pixels (CoC ≈ 0) are fully sharp; peripheral pixels are fully blurred.

---

## 6. Intermediate image allocations

All allocations are conditional — if the triggering component isn't present, the memory isn't used.

| Image | Trigger | Format | Size | Notes |
|---|---|---|---|---|
| Main color intermediate | `RenderGraphComponent` | `R16G16B16A16_SFLOAT` | Full res | Geometry renders here instead of swapchain |
| HDR emissive buffer | `BloomComponent` | `R16G16B16A16_SFLOAT` | Full or half res | Emissive prepass target |
| Blurred emissive buffer | `BloomComponent` | Same as above | Same | Ping-pong for H+V blur |
| Stored depth (resolved) | `BloomComponent` or `BokehComponent` | `R32_SFLOAT` | Full res | MSAA depth resolve target |
| CoC image | `BokehComponent` | `R16_SFLOAT` | Full res | Per-pixel CoC radius |
| DoF blurred image | `BokehComponent` | Same as intermediate | Full res | Blurred scene color |

**Without `RenderGraphComponent`**: nothing is allocated, current code path runs unchanged.

---

## 7. Full render graph sequence

```
No RenderGraphComponent:
  [geometry phases] → swapchain

With RenderGraphComponent:

[1] Main geometry passes (begin_rendering → end_rendering)
      Color → main color intermediate (R16G16B16A16_SFLOAT)
      Depth → stored D32_SFLOAT (if BloomComponent or BokehComponent present)
      Depth MSAA resolve → R32_SFLOAT (if MSAA enabled + depth needed)

[2] Emissive prepass (skip if no BloomComponent or zero emissive instances)
      Input: stored depth (optional, read-only, for occlusion)
      Draw: EMISSIVE_TOON_MESH / SKINNED_EMISSIVE_TOON_MESH batches only
      Output: HDR emissive buffer

[3] Bloom blur H (skip if no BloomComponent)
      Input: HDR emissive buffer
      Output: temp blur buffer

[4] Bloom blur V (skip if no BloomComponent)
      Input: temp blur buffer
      Output: blurred emissive buffer

[5] Bloom composite (skip if no BloomComponent)
      Input: main color intermediate + blurred emissive buffer
      Output: main color intermediate (additive blend)

[6] CoC computation (skip if no BokehComponent)
      Input: stored depth
      Output: CoC image

[7] Bokeh blur (skip if no BokehComponent)
      Input: main color intermediate + CoC image
      Output: DoF blurred image

[8] DoF composite (skip if no BokehComponent)
      Input: main color intermediate (sharp) + DoF blurred image + CoC image
      Output: main color intermediate (overwritten)

[9] Final blit to swapchain
      Input: main color intermediate
      Output: swapchain image (tone-map / gamma correct here if needed)
```

Passes [2]–[8] each run as a separate `begin_rendering()` / `end_rendering()` scope (or as compute dispatches). They are not part of the main geometry scope.

---

## 8. XR considerations

In XR mode, the engine renders each eye into a separate `XrOffscreenTarget` (color + depth). Post-processing must run per-eye.

Each eye needs its own:
- Main color intermediate
- HDR emissive buffer (if bloom)
- Blurred emissive buffer (if bloom)
- Stored depth (if bloom w/ occlusion or bokeh)
- CoC image (if bokeh)
- DoF blurred image (if bokeh)

The post-processing passes run twice — once for left eye, once for right — after the geometry for each eye is recorded.

**Alternative (future)**: Use `VK_KHR_multiview` for single-pass stereo post-processing. Not worth the complexity until per-eye approach is proven.

---

## 9. sRGB vs linear color space

Currently the swapchain is sRGB (`R8G8B8A8_SRGB`). The proposed main color intermediate uses `R16G16B16A16_SFLOAT` (linear).

This means:
- Geometry output is in linear space (no implicit sRGB conversion mid-frame)
- Bloom blur happens in linear space — correct behavior (blending in sRGB space darkens colors)
- The final swapchain blit applies gamma correction (sRGB encode)

If the intermediate is kept as sRGB, the bloom additive blend produces slightly wrong colors (darker than physically correct). Linear intermediate is the right call.

---

## 10. Performance and overhead summary

| Scenario | Overhead |
|---|---|
| No `RenderGraphComponent` | Zero — current fast path unchanged |
| `RenderGraphComponent` only | One extra blit to swapchain (minor) |
| + `BloomComponent` (half-res) | Emissive prepass + 2 half-res blurs + composite |
| + `BloomComponent` (full-res) | Emissive prepass + 2 full-res blurs + composite |
| + `BokehComponent` | Depth store + CoC pass + blur + composite |
| Both bloom + bokeh | All of the above; depth stored once for both |

**Emissive prepass cost**: nearly free if there are few emissive instances. The prepass only draws `EMISSIVE_TOON_MESH` batches.

**Blur passes**: the most expensive part. Two full-screen passes per blur axis. Half-res bloom is strongly recommended by default.

**Depth store on desktop**: changing `DontCare` → `Store` is effectively free (memory write already happened).

---

## 11. Future effects (same infrastructure)

These effects could be layered in later under the same `RenderGraphComponent` tree:

- **SSAO** (screen-space ambient occlusion): needs stored depth and normals; similar to bokeh CoC pass
- **SSR** (screen-space reflections): needs stored color + depth
- **Tone mapping / color grading**: natural fit for the final swapchain blit pass (step [9])
- **Chromatic aberration**: trivial full-screen pass; no extra buffers
- **TAA** (temporal anti-aliasing): replaces MSAA; requires per-instance velocity/motion vectors (new vertex output)
- **Vignette / lens distortion**: trivial full-screen pass; no extra buffers

---

## 12. Open questions

1. **Emissive prepass depth for bloom occlusion**: should we always store depth when `BloomComponent` is present, to avoid glow bleeding through walls? Or leave it as an opt-in (only if `BokehComponent` also present)?
   - **Proposed**: always store when `BloomComponent` is present. The depth store cost on desktop is negligible.

2. **Emissive prepass placement**: before vs after main geometry passes?
   - **Proposed**: after. The stored depth from the main passes is then available for read-only depth test in the emissive prepass.

3. **BloomComponent scope**: one per scene vs one per camera?
    - **Proposed**: one per scene (child of `RenderGraphComponent`). XR handles per-eye duplication internally without needing per-camera bloom configuration.

4. **Material handle for emissive objects not using `GLTFComponent`**: user-authored objects need to opt into `EMISSIVE_TOON_MESH` explicitly, or the engine assigns it automatically when `EmissiveComponent` is present.
   - **Proposed**: `RenderableSystem` assigns the emissive material handle automatically when it sees `EmissiveComponent` with `intensity > 0.0` in the same subtree. No user plumbing required.
