# Texture Mipmaps & Filtering ＼(＾▽＾)／

Improve texture filtering quality by generating mipmaps on upload and fixing
the sampler configuration so text/UI is crisp up close and alias-free at
distance.

---

## Current State

`upload_texture_rgba8` creates a single-level image (`mip_levels: 1`).
`upload_texture_bc7` does the same.  No mip chain is ever generated.

Samplers (`vulkano_renderer.rs`):

| `TextureFiltering` | mag | min | mipmap mode | Effect without mipmaps |
|---|---|---|---|---|
| `Linear` (default) | Linear | Linear | Linear | Bilinear everywhere — up close blurry, far away aliases because only a 2×2 texel neighbourhood is sampled |
| `Nearest` | Nearest | Nearest | Nearest | Always pixelated |
| `NearestMagnification` | **Nearest** | Linear | **Nearest** | Crisp when magnified, bilinear when minified — but mipmap mode is wrong, and without mips, distant aliasing is identical to `Linear` |

The `mipmap_mode: Nearest` on `sampler_nearest_mag` is also a bug: it should
be `Linear` so transitions between mip levels are smooth.

---

## Goal

For text and UI rendered via `NearestMagnification`:

- **Up close** (texel : pixel < 1, magnification) → **nearest** — crisp, no blur.
- **Far away** (texel : pixel > 1, minification) → **trilinear** — the GPU picks
  the appropriate pre-averaged mip level, interpolates smoothly between levels.
  No shimmer, no grain.

For `Linear` textures (scene objects, GLTFs, etc.): trilinear filtering
automatically once mips exist — no sampler change needed, `simple_repeat_linear`
already uses `MipmapMode::Linear`.

---

## Changes Required ＼(＾▽＾)／

### 1 — `vulkano_texture_upload.rs` · generate mipmaps for RGBA8

**`upload_texture_rgba8`** needs to:

1. Compute `mip_levels = floor(log2(max(width, height))) + 1`.
2. Create the image with `mip_levels` and add `ImageUsage::TRANSFER_SRC` (required
   to blit from one level to the next).
3. Copy staging buffer into mip level 0 as before.
4. Issue a blit chain — for each level `i` from 1 to `mip_levels - 1`:
   a. Pipeline barrier: transition level `i-1` from
      `ImageLayout::TransferDstOptimal` → `TransferSrcOptimal`.
   b. `blit_image` from level `i-1` to level `i` with `Filter::Linear`.
   c. Level `i` stays in `TransferDstOptimal` until the blit writes it, then
      transition to `ShaderReadOnlyOptimal`.
5. After the loop, transition level 0 to `ShaderReadOnlyOptimal` as well.
6. Submit, wait for fence, return view.

```
ImageCreateInfo {
    mip_levels,
    usage: TRANSFER_SRC | TRANSFER_DST | SAMPLED,
    ...
}
```

> **Note on BC7**: BC7 (`BC7_UNORM_BLOCK` / `BC7_SRGB_BLOCK`) fully supports
> linear sampling — `VK_FORMAT_FEATURE_SAMPLED_IMAGE_FILTER_LINEAR_BIT` is
> guaranteed on any Vulkan implementation that supports BC compression.  What
> BC7 cannot do is act as a `BLIT_DST` for GPU-side mip generation, so the
> RGBA8 blit chain below doesn't apply.
>
> However, DDS files already contain a full pre-baked mip chain.  The current
> `decode_dds_bc7` discards all mips beyond level 0 (`// We only use the top mip
> for now.` — `texture_system.rs:363`).  The fix is to read all mip levels from
> the DDS data and upload each block slice to its corresponding image level.
> This is a separate but straightforward change to `decode_dds_bc7` and
> `upload_texture_bc7`.

### 2 — `vulkano_renderer.rs` · fix `sampler_nearest_mag`

Change `mipmap_mode` from `Nearest` to `Linear`:

```rust
let sampler_nearest_mag = Sampler::new(
    device.clone(),
    SamplerCreateInfo {
        mag_filter: Filter::Nearest,
        min_filter: Filter::Linear,
        mipmap_mode: SamplerMipmapMode::Linear,   // was: Nearest
        address_mode: [SamplerAddressMode::Repeat; 3],
        ..Default::default()
    },
)?;
```

No change needed to `sampler_linear` — `simple_repeat_linear()` already sets
`MipmapMode::Linear`; it will benefit automatically.

---

## Layout Transition Detail (ﾉ ᵒ ᵕ ᵒ)ﾉ

Vulkan requires explicit layout transitions between uses.  The blit chain:

```
[staging copy]
  level 0: UNDEFINED → TRANSFER_DST_OPTIMAL   (implicit via copy)

for i in 1..mip_levels:
  barrier: level i-1  TRANSFER_DST_OPTIMAL → TRANSFER_SRC_OPTIMAL
  blit:    level i-1 → level i  (writes i into TRANSFER_DST_OPTIMAL)

[after loop]
  barrier: all levels → SHADER_READ_ONLY_OPTIMAL
```

In Vulkano, `CopyBufferToImageInfo` handles the initial upload layout.
The blit uses `BlitImageInfo` with `ImageBlit` regions specifying source/dst
mip levels and the full extent for each (halving each dimension).

`ImageSubresourceRange` for the final barrier should cover
`base_mip_level: 0 .. mip_levels`.

---

## `TextureFiltering` Enum — No API Change Needed

The existing `NearestMagnification` variant already expresses the intent.
The fix is purely internal (sampler config + mip generation).

---

## 3 — `TextureFilteringComponent` · mipmap control ＼(＾▽＾)／

Mipmapping should be opt-out.  Some use-cases need it disabled:

- Textures at a fixed pixel size that will never be minified (rare, mostly
  debug).
- Any case where the mip-averaged colour is undesirable (e.g. a texture used
  as a data lookup rather than a visual surface).

### Component field

Add a `mipmaps: bool` field (default `true`):

```rust
pub struct TextureFilteringComponent {
    pub filtering: TextureFiltering,
    pub mipmaps: bool,        // default: true
}

impl TextureFilteringComponent {
    pub fn without_mipmaps(mut self) -> Self {
        self.mipmaps = false;
        self
    }
}
```

`encode`/`decode` gain a `"mipmaps"` boolean key (absent = true for
back-compat).

### Sampler selection

The renderer currently picks a sampler purely from `TextureFiltering`.  With
`mipmaps`, the effective sampler key becomes `(TextureFiltering, mipmaps)` — 6
combinations.

Rather than creating 6 separate `Arc<Sampler>` objects, the cleanest
implementation is **LOD range clamping**: when `mipmaps: false`, set the
sampler's `lod_clamp_range: 0.0..=0.0`.  This forces the hardware to always
sample from mip level 0, effectively disabling mipmapping without needing a
separate image upload or sampler matrix.

```rust
// conceptually — implemented inside get_or_create_material_set or a
// sampler_for(filtering, mipmaps) helper:
if !mipmaps {
    lod_clamp_range = 0.0..=0.0;   // pin to base level
}
```

The six logical combinations:

| `filtering` | `mipmaps` | mag | min | mip mode | lod clamp |
|---|---|---|---|---|---|
| `Linear` | true | Linear | Linear | Linear | 0..∞ |
| `Linear` | false | Linear | Linear | Linear | 0..=0 |
| `Nearest` | true | Nearest | Nearest | Nearest | 0..∞ |
| `Nearest` | false | Nearest | Nearest | Nearest | 0..=0 (no practical difference) |
| `NearestMagnification` | true | Nearest | Linear | Linear | 0..∞ |
| `NearestMagnification` | false | Nearest | Linear | Linear | 0..=0 |

Samplers are cheap to cache — six static instances in `VulkanoState` is fine,
no dynamic creation needed.

### Sampler cache key

`get_or_create_material_set` currently keys on
`(material, texture_handle, TextureFiltering, quant_bits)`.  Extend to include
`mipmaps: bool`, or pack it into an existing field (e.g. low bit of a combined
`u32`).

`VisualInstance` and `DrawBatch` gain the field.  `build_draw_batches_for_order`
splits batches on it (same as `texture_filtering`).

### Propagation

`TextureFilteringComponent` is already inherited once per `TextComponent` via
`register_text` → `inherited_filtering`.  No change needed to the propagation
path — the `mipmaps` field rides along with `filtering` in the same component.

### Default

`mipmaps: true` on every new `TextureFilteringComponent`.  Existing call-sites
that construct `TextureFilteringComponent::nearest_magnification()` etc. get
mipmapping automatically — no source changes required unless they want to opt
out.

---

## Scope / Non-Goals

- Anisotropic filtering: out of scope for this change, can be a follow-up.
- LOD bias: not needed.
- Per-texture mip count override: not needed — `mip_levels` is always the full
  chain; disabling is done at the sampler level via LOD clamping.

---

## Files Affected

- `src/engine/graphics/vulkano_texture_upload.rs` — mip generation in `upload_texture_rgba8`
- `src/engine/graphics/vulkano_renderer.rs` — `sampler_nearest_mag` mipmap mode fix; 6-sampler table; extend `get_or_create_material_set` cache key
- `src/engine/graphics/visual_world.rs` — add `mipmaps: bool` to `VisualInstance` and `DrawBatch`; split batches on it
- `src/engine/ecs/component/texture_filtering.rs` — add `mipmaps` field, builder, encode/decode
- `src/engine/ecs/system/texture_system.rs` — propagate `mipmaps` field alongside `filtering` in `inherited_filtering`
- `src/engine/ecs/system/renderable_system.rs` — pass `mipmaps` through `flush_pending` → `visuals.update_texture_filtering`
