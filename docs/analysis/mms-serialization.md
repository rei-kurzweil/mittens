# MMS Serialization Audit

Inventory of `encode()`/`decode()` across all components — what they currently do, where they fall short, and what needs to change to make them MMS-native.

---

## The Core Problem

The `Component` trait defines:

```rust
fn encode(&self) -> HashMap<String, serde_json::Value>
fn decode(&mut self, data: &HashMap<String, serde_json::Value>) -> Result<(), String>
```

This is JSON, not MMS. The only consumer is `component_codec.rs`, which uses these to save/load scenes as `.json` files. MMS round-tripping is not wired up at all — the interface signature is wrong for it.

**What MMS encoding should look like:**

```
// Current (JSON in component_codec):
{ "intensity": 0.95, "radius_ndc": 0.06, "half_res": true }

// What MMS emission should produce:
Bloom {
    intensity(0.95)
    radius_ndc(0.06)
    half_res(true)
}
```

For MMS round-trip the signature needs to change (or a second trait method added):

```rust
fn encode_mms(&self) -> String          // emits MMS component body
fn decode_mms(&mut self, src: &str)     // parses MMS component body
```

The JSON codec path (`component_codec.rs`) can stay as-is for scene persistence, but the MMS path is entirely missing.

---

## Where encode/decode Is Used Today

Only `src/engine/ecs/component_codec.rs`:
- `ComponentCodec::encode_subtree` → calls `component.encode()` → writes JSON
- `ComponentCodec::decode_subtree` → calls `component.decode(&data)` → reads JSON
- Used for file-based scene save/load and `attach_clone` (clone-by-serialize)

The REPL path mentioned in CLAUDE.md is not wired up.

---

## Components With No encode/decode (Fall Through to Default Empty Impl)

These return an empty HashMap from `encode()` and are no-ops on `decode()`. For zero-state marker components this is fine. For anything with configuration it is silent data loss on clone/save.

| Component | File | Has State? | Problem |
|---|---|---|---|
| `OverlayComponent` | `overlay.rs` | no (marker) | OK |
| `TransformMapTranslationComponent` | `transform_pipeline_map.rs` | no (marker) | OK |
| `TransformMapRotationComponent` | `transform_pipeline_map.rs` | no (marker) | OK |
| `TransformMapScaleComponent` | `transform_pipeline_map.rs` | no (marker) | OK |

These four are fine as-is. Their MMS form is just `TransformMapTranslation {}` with no body.

---

## Components Missing From `component_codec.rs` Factory

`ComponentCodec::create_component` has a hard-coded match. These 27 components are missing — if a saved scene contains them, decode returns `Err("Unknown component type")`:

**UI / Layout**
- `style` — `StyleComponent` (has extensive flex/size/color state)
- `layout` — `LayoutComponent`
- `html_element` — `HtmlElementComponent`
- `stencil_clip` — `StencilClipComponent`
- `scrolling` — `ScrollingComponent` (recently added)
- `world_panel` — `WorldPanelComponent`
- `inspector_panel` — `InspectorPanelComponent`
- `selectable` — `SelectableComponent`

**Rendering**
- `blur_pass` — `BlurPassComponent`
- `transparent_cutout` — `TransparentCutoutComponent`
- `normal_visualisation` — `NormalVisualisationComponent`
- `texture_filtering` — `TextureFilteringComponent`
- `gltf` — `GLTFComponent`
- `mesh` — `MeshComponent`

**Audio**
- `audio_band_pass_filter`
- `audio_gain`
- `audio_high_pass_filter`
- `audio_limiter`
- `audio_low_pass_filter`
- `audio_mix`
- `audio_oscillator`
- `music_note`

**Avatar / IK**
- `avatar_body_yaw` — `AvatarBodyYawComponent`
- `avatar_control` — `AvatarControlComponent`
- `ik_chain` — `IKChainComponent`

**Transform Pipeline (extras)**
- `quat_extract_yaw`
- `quat_yaw_follow`
- `raycastable_shape`

---

## Data Fidelity Problems in Existing encode/decode

### `TransformComponent`

```rust
// encode stores the flat model matrix:
map.insert("model", json!(self.transform.model));

// decode restores only model:
self.transform.model = ...;
self.transform.matrix_world = self.transform.model;
// translation / rotation / scale fields are NOT restored
```

**Problem:** Stores the baked 4×4 model matrix. The individual `translation`, `rotation`, `scale` fields are not preserved — `TransformSystem` is expected to recompute from them later, but they start as zero/default after decode. In practice this breaks cloned transforms.

**MMS form needs:** `T.position(x, y, z).scale(sx, sy, sz).rotation(rx, ry, rz)` — three float-triple calls. The encode should capture TRS, not the baked matrix.

### `RenderableComponent`

```rust
map.insert("mesh",      json!(self.renderable.mesh.0));       // u32
map.insert("base_mesh", json!(self.renderable.base_mesh.0));  // u32
map.insert("material",  json!(self.renderable.material.0));   // u32
```

**Problem:** Stores raw runtime handle IDs. `CpuMeshHandle` integers are assigned at startup via `RenderAssets::register_mesh`. They are stable only for the built-in handles (CUBE=0, SPHERE=1, etc.) but fragile for dynamic meshes. There is no way to reconstruct a meaningful MMS expression (`R.cube()`, `R.sphere()`) from a u32.

**MMS form needs:** A `mesh_kind` string key (`"cube"`, `"sphere"`, `"plane"`, `"triangle"`, `"square"`, `"tetrahedron"`, `"circle2d"`) derived from `base_mesh`. Dynamic meshes (from GLTF) have no MMS representation yet.

### `StyleComponent`

```rust
// encode comment says explicitly:
// "Encode a representative subset for REPL/debug; full round-trip not required."
// Only captures: display, position, flex_grow, flex_shrink
```

**Problem:** Intentionally incomplete. The full `StyleComponent` has padding, margin, size dimensions (`SizeDimension`), flex direction/wrap/align, overflow, background color, border radius, gap, etc. None of that survives encode/decode.

### `TransformComponent` (TRS recovery)

Already noted above, but worth repeating: the mismatch between "store model matrix" and "MMS uses TRS calls" means a decoded transform will have wrong `translation`/`rotation`/`scale` even if `model` is correct.

### `AnimationComponent`

```rust
map.insert("state", json!(format!("{:?}", self.state)));  // e.g. "Looping"
```

Debug-format strings like `"Looping"` are not stable across refactors and not valid MMS syntax. MMS uses `A.looping {}` / `A.playing {}` / `A.paused {}`.

### `ControllerXRComponent`

```rust
map.insert("hand",  json!(format!("{:?}", self.hand)));   // "Left" / "Right"
map.insert("pose",  json!(format!("{:?}", self.pose)));   // "Aim" / "Grip"
```

These happen to match what `component_registry` expects in its `ControllerXR.new(enabled, hand, pose)` ctor — but only because the Debug strings are `"Left"`/`"Right"` and `"Aim"`/`"Grip"`. Fragile. Should use canonical enum string keys.

### `GestureCoordTypeComponent`

```rust
map.insert("coord_type", json!(format!("{:?}", self.coord_type)));
```

Same Debug-string fragility pattern.

---

## What a Proper MMS encode Would Look Like (Per Component)

The MMS AST is a `ComponentExpression`:

```
ComponentType[.ctor_method(args)] {
    .call(args)
    key = value
    "positional"
    ChildComponent { }
}
```

`encode_mms()` only needs to produce the body (calls + named assignments) — the type name and children are handled by the tree walker. Examples:

| Component | MMS body |
|---|---|
| `Transform` | `.position(x, y, z).scale(sx, sy, sz).rotation(rx, ry, rz)` |
| `Color` | (ctor arg) — type is `C.rgba(r, g, b, a)` so ctor carries state |
| `Bloom` | `intensity(0.95)\nradius_ndc(0.06)\nhalf_res(true)` |
| `Renderable` | type line carries ctor: `R.cube()` — body empty |
| `DirectionalLight` | `intensity(0.9)` |
| `Scrolling` | ctor args: `Scrolling.new(vh, ch)` — body empty |
| `Texture` | ctor: `Texture.uri("path/to/file")` — body empty |
| `Animation` | ctor: `A.looping {}` / `A.playing {}` |

Key design question: **ctor args vs body calls**. Many components encode their primary state as constructor args (`Color.rgba(...)`, `Scrolling.new(v, c)`). The MMS emitter needs to know whether to put state into the ctor signature or body calls. This is not currently representable via the HashMap interface.

---

## Summary: What's Missing

| Gap | Scope |
|---|---|
| Trait signature wrong for MMS | All components — `fn encode(&self) -> String` / `fn decode_mms(&mut self, src: &str)` |
| `component_codec.rs` factory missing 27 components | `component_codec.rs` only |
| Transform stores matrix not TRS | `TransformComponent` |
| Renderable stores runtime handle IDs | `RenderableComponent` |
| Style encode is explicitly partial | `StyleComponent` |
| Debug-format enum strings (fragile) | `AnimationComponent`, `ControllerXRComponent`, `GestureCoordTypeComponent`, others |
| No MMS round-trip path wired up anywhere | Architecture — no caller exists |

The JSON codec (`component_codec.rs`) is functional for the components it covers. The MMS path is a greenfield addition: new trait method(s), a tree-walking emitter, and per-component MMS body implementations.
