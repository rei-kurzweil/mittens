# Multi-asset support and texture-paint workflow

Date: 2026-06-05

Status: planning only. Do not start implementation from this doc.

## Goal

Extend the editor asset workflow beyond MMS factory assets so the assets panel can browse and select these proposed asset types:

0. folders, if we decide they count as assets for browser purposes; they may need to for cases like skyboxes that are effectively six-image sets
1. MMS script assets: `.mms`
2. image/texture assets: `.dds`, `.png`
3. audio assets: `.wav`, `.opus`
4. model assets: `.glb`
5. shader assets: `.frag`, `.vert`

And change paint behavior so:

- selecting an MMS or GLB asset keeps the current "place an object into the world" model
- selecting a texture/image asset changes the paint panel into a texture-application workflow
- when paint is active and the selected asset is a texture, clicking a scene object applies that texture to the clicked target instead of spawning a new renderable subtree

This is primarily an editor/data-model task. The rendering and interaction paths already exist in partial form; they need to be generalized around asset kind.

## Current implementation facts

### Asset discovery is MMS-only today

`AssetSystem` in [src/engine/ecs/system/asset_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/asset_system.rs) currently:

- scans a single directory with `scan_assets_dir()`
- only loads files with `.mms` extension
- evaluates MMS modules and exposes exported functions as `AssetItem`s
- treats each asset item as a callable factory/prefab, not as a generic file asset

Current `AssetItem` shape is MMS-centric:

- `module_id`
- `export_name`
- `title`
- `description`
- `category`
- `param_names`

That shape cannot represent a raw folder entry, `.png`, `.dds`, `.wav`, `.opus`, `.glb`, `.frag`, or `.vert` asset without bolting on nullable fields.

### Paint behavior is hardwired to MMS placement

`EditorPaintSystem` in [src/engine/ecs/system/editor_paint_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system.rs) currently resolves paint activity to a `PaintAssetTemplate`:

- asset selection state stores only `selected_item: Option<String>`
- the selected string is matched back to a template by `title`
- `place_asset()` always calls `spawn_mms_module_component_uninitialized(...)`
- placement then wraps the spawned subtree in a transform/raycastable root and attaches it into the scene

So the paint system has no notion of "apply texture", "preview audio", or "spawn GLTF component".

### Selection identity is display-text based

`SelectionSystem` currently derives selected item identity from `#selection_item_label` text in
[src/engine/ecs/system/selection_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/selection_system.rs).

That is tolerable for the current MMS panel, but it is weak for mixed asset inventories because:

- labels are presentation, not identity
- duplicate file names across folders or kinds are plausible
- paint dispatch should not depend on UI text matching

This becomes a real issue once `cat-face-neutral.png` and `cat-face-neutral.dds` can both exist as assets.

### Asset preview is already split between "easy bounded thing" and "hard unknown thing"

The current asset preview path in `AssetSystem::build_asset_item_shell()` already reflects the same difficulty split you described:

- renderable geometry with measurable bounds is previewable now
- styled/UI content sometimes needs deferred layout remeasurement
- panel-like assets are temporarily skipped

For the new asset kinds:

- image/texture preview is the easy case because a 2D quad can be sized explicitly
- GLB preview is the hard case because its bounds are unknown until import and may be huge, tiny, offset, or absent
- audio preview is not really a visual-bounds problem at all and should likely use metadata/icon UI rather than a world-space content preview

## Proposed data model

### Add `AssetType`

Introduce an asset-kind enum at the asset-system layer:

```rust
enum AssetType {
    Folder,
    Mms,
    Texture,
    Audio,
    Glb,
    Shader,
}
```

This should be the decision point for:

- discovery
- preview strategy
- panel labeling
- paint behavior
- default spawn/application behavior

The important design rule is: asset type should be first-class and explicit, not inferred ad hoc from file names at every call site.

### Split generic asset identity from kind-specific payload

`AssetItem` should become a generic editor-facing record with stable identity plus kind-specific source info.

Sketch:

```rust
struct AssetItem {
    id: AssetId,
    asset_type: AssetType,
    title: String,
    path: PathBuf,
    source: AssetSource,
    description: Option<String>,
    category: Option<String>,
}

enum AssetSource {
    MmsExport {
        module_id: AssetModuleId,
        export_name: String,
        param_names: Vec<String>,
    },
    TextureFile,
    AudioFile,
    GlbFile,
    ShaderFile,
}
```

Notes:

- the stable key for paint and selection should be `AssetId` or `ComponentId`, not label text
- `path` belongs on all file-backed kinds, including MMS modules
- keeping `AssetSource::MmsExport` separate preserves current export-level browsing for MMS modules

## Discovery changes

### Expand scan coverage

`scan_assets_dir()` should enumerate at least:

- folders
- `.mms`
- `.dds`
- `.png`
- `.opus`
- `.wav`
- `.glb`
- `.frag`
- `.vert`

and classify them into `AssetType`.

### Keep MMS export semantics

MMS should remain export-based:

- one `.mms` file may yield multiple asset entries
- those entries still behave like factories/prefabs

Non-MMS files should be one asset item per file.

### Folder navigation in the assets panel

The assets panel should not behave like a flat dump of one directory's files. It needs explicit
folder items and navigation.

Required behavior:

- folders should appear as asset items in the panel
- the first item should always be `..`
- selecting `..` navigates to the parent of the current folder
- selecting a folder item navigates into that folder

Even at the asset-model layer, this is a useful reason to treat folder as a first-class
`AssetType` rather than a special-case UI row. We will likely want that same distinction later for
filtering, breadcrumbs, tree views, drag/drop targets, and other file-browser behavior.

Open detail:

- at the filesystem root we still want `..` shown as the first item for UI consistency, but it may
  no-op if there is no allowed parent within the editor's asset browsing scope

### Dependency/runtime caveat for Opus

The codebase already has `AudioClip.opus(...)` surface support in component serialization, but `Cargo.toml` currently enables Symphonia features:

- `wav`
- `pcm`
- `vorbis`
- `ogg`
- `flac`
- `mp3`

and does not currently enable an `opus` feature in the listed dependency stanza.

Before calling Opus "supported" in the editor, verify the decode pipeline end-to-end. This may require a dependency feature change, not just an asset-browser change.

## Preview strategy

The assets panel should stop pretending every asset preview is "spawn the authored thing into a tile".

Instead, preview should branch by `AssetType`.

### Texture preview

Recommended first pass:

- render a fixed-size quad in `#preview_slot`
- attach a `TextureComponent` pointing at the asset path
- use a known square mesh so sizing is deterministic
- preserve aspect ratio if dimensions are available cheaply; otherwise letterbox inside the tile

Why this is the easiest case:

- no unknown world scale
- no import-time scene graph
- no placement pose math
- the preview surface can be fully controlled by the panel

### GLB preview

Recommended first pass:

- treat this as "best effort", not parity with texture preview
- either:
  - show a lightweight placeholder/icon + filename, or
  - reuse the existing measured-subtree preview path when imported bounds are available

GLB preview should not block the broader asset-type refactor. Unknown model scale is a real complication, not a cosmetic detail.

### Audio preview

Recommended first pass:

- do not attempt waveform rendering initially
- render an icon/label/extension/duration placeholder if cheap metadata is available
- optionally add a play button later, but keep that out of the initial task unless the interaction contract is agreed

### MMS preview

Keep the existing MMS preview path, but make it one strategy under the generic asset-preview router rather than the default for all assets.

### Folder preview

Folders do not need a rendered world-space preview. Recommended first pass:

- show a folder icon or folder-styled placeholder tile
- show `..` using the same folder/navigation presentation, but visually distinct enough that it
  reads as "navigate up" rather than "ordinary directory asset"

### Shader preview

Shaders do not need a rendered world-space preview for the first pass. Recommended first pass:

- show a shader/code icon or placeholder tile
- show the extension prominently so `.frag` and `.vert` are easy to distinguish
- avoid implying they are directly paintable until shader-attachment behavior is specified

## Paint behavior by asset type

### Texture assets: apply to clicked target

When paint is active and the selected asset is `AssetType::Texture`:

- clicking a valid scene target should apply a texture reference to that target
- no new object should be spawned into the world

Recommended contract:

- the hit primitive is still a renderable hit from the existing click/raycast path
- the system resolves the target renderable from that hit
- the system ensures the target renderable has exactly one active file-backed `TextureComponent` attachment for this authored texture slot, replacing or updating the existing one

Open implementation detail:

- if multiple texture components are attached today, the engine needs a rule for which one is "the editable paint slot"
- first pass should likely target the nearest directly attached `TextureComponent` under the clicked renderable, or add one if absent

### MMS assets: keep placement behavior

No semantic change:

- selected MMS asset
- paint active
- click scene object
- spawn authored subtree
- place using current surface/grid placement rules

### GLB assets: likely object placement

Recommended first pass:

- selecting a `.glb` asset should behave like object placement, not texture application
- paint action should spawn a simple transform root with a `GLTFComponent::new(path)`

This keeps GLB aligned with "place content into the world", which matches user expectation more closely than trying to treat GLB like a texture or sound.

### Audio assets: out of scope for paint

Audio assets should be selectable and previewable in the assets panel, but they should not imply a paint behavior unless we explicitly define one.

Recommended first pass:

- selecting audio keeps the status text honest: asset selected, but paint action unsupported
- clicking scene objects while audio is selected should no-op with a clear status message

Later, if desired, audio could mean "attach AudioClip to clicked object", but that is a separate UX decision and should not be smuggled into this task.

### Folder assets: navigation, not paint

Folders are selectable in the asset browser for navigation purposes, but they should not activate a
scene paint operation.

Recommended first pass:

- selecting a folder or `..` changes the assets-panel directory view
- folder items do not become the active paint asset
- paint status should remain unchanged or clearly report that navigation items are not paintable

### Shader assets: browse/select-only for now

Shader assets should be discoverable and selectable in the assets panel, but they should not imply a paint behavior until we define how a shader gets attached to a renderable/material.

Recommended first pass:

- selecting a `.frag` or `.vert` asset keeps the status text honest: asset selected, but paint action unsupported
- clicking scene objects while a shader asset is selected should no-op with a clear status message

## Paint-system refactor needed

The current paint state/template model should be generalized from "selected MMS template" to "selected asset descriptor".

Likely changes:

1. Replace `PaintAssetTemplate` as the only shared asset payload with a generic paint asset record.
2. Store stable asset identity in paint state instead of only `selected_item: Option<String>`.
3. Resolve paint action from `AssetType`.
4. Split `place_asset()` into type-specific operations, for example:
   - `place_mms_asset(...)`
   - `place_glb_asset(...)`
   - `apply_texture_asset(...)`
   - `unsupported_paint_asset(...)`

This should keep the existing activation logic:

- panel focus still matters
- selected paint tool still matters
- scene-hit filtering still matters

Only the effect of the click should branch on asset kind.

## UI changes likely needed

### Assets panel item metadata

Each tile should expose enough metadata for debugging and future filtering:

- asset type badge or extension
- title
- maybe source path or module/export subtitle

For folders:

- folder items should be visually distinct from file assets
- `..` should always occupy the first slot regardless of sort order for the rest of the entries

This matters more once the panel contains mixed content.

### Selection should not rely on label text

Because selection currently derives `selected_item` from `#selection_item_label`, this task should either:

- enrich selection entries with a non-visual asset id binding, or
- have the asset panel maintain a direct map from selected `ComponentId` to `AssetItem`

The second option is a smaller refactor if we want to avoid changing the generic selection system immediately.

### Paint status text should reflect asset type

Examples:

- `paint active | texture mode`
- `paint active | object placement`
- `paint inactive: shader assets are not paintable`
- `paint inactive: selected asset cannot be painted`

The current status text assumes every valid asset is placeable.

## Files/systems likely touched

- [src/engine/ecs/system/asset_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/asset_system.rs)
  Discovery, `AssetItem`, preview routing, non-MMS asset entries.
- [src/engine/ecs/system/editor_paint_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system.rs)
  Replace MMS-only paint template flow with type-based paint actions.
- [src/engine/ecs/system/editor_paint_system_state_manager.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system_state_manager.rs)
  Paint state identity should stop being title-only.
- [src/engine/ecs/system/selection_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/selection_system.rs)
  May need a stronger identity path than display text.
- [assets/components/asset_item.mms](/home/rei/_/cat-engine/assets/components/asset_item.mms)
  Tile layout may need a type badge / subtitle / differentiated preview treatment.
- [assets/components/assets_content.mms](/home/rei/_/cat-engine/assets/components/assets_content.mms)
  Selection scope likely stays the same, but mixed asset content may need filtering/search hooks later.
- [src/engine/ecs/component/texture.rs](/home/rei/_/cat-engine/src/engine/ecs/component/texture.rs)
  Texture application path should use existing file-backed `TextureComponent` conventions.
- [src/engine/ecs/component/gltf.rs](/home/rei/_/cat-engine/src/engine/ecs/component/gltf.rs)
  GLB placement should route through `GLTFComponent`.
- [src/engine/ecs/component/audio_clip.rs](/home/rei/_/cat-engine/src/engine/ecs/component/audio_clip.rs)
  Audio asset browsing can reuse existing clip URI conventions even if paint is unsupported.
- shader/material systems
  Shader asset browsing should stop short of attachment/application behavior in the first pass unless a concrete shader-binding path already exists.

## Suggested implementation order

1. Add `AssetType` and generalize `AssetItem`.
2. Add folder entries and directory navigation semantics, including persistent `..`.
3. Expand asset scanning to mixed file types while preserving MMS export scanning.
4. Refactor asset selection to stable asset identity instead of title matching.
5. Split preview generation by asset type.
6. Split paint action by asset type.
7. Implement texture-application paint behavior.
8. Add GLB placement behavior.
9. Leave audio and shader assets as browse/select-only unless a concrete attach/apply UX is specified.

## Verification checklist

- assets panel lists folders, `.mms`, `.dds`, `.png`, `.wav`, `.opus`, `.glb`, `.frag`, and `.vert` entries from the configured asset tree
- assets panel lists folders as first-class items and always shows `..` in the first slot
- clicking a folder item navigates into it
- clicking `..` navigates upward when possible
- selecting a texture asset and using Free Draw on a scene object applies/replaces texture on the clicked target instead of spawning a new subtree
- selecting an MMS asset still places the authored subtree exactly as before
- selecting a GLB asset places a GLTF-backed object rather than failing through the MMS path
- selecting an audio asset does not crash and reports unsupported paint behavior clearly
- selecting a shader asset does not crash and reports unsupported paint behavior clearly
- selecting folder navigation items does not corrupt paint selection state
- mixed assets with similar labels do not alias each other through title-based selection
- texture preview tiles are consistently readable regardless of source image dimensions

## Non-goals for the first pass

- waveform rendering
- audio playback UI in the assets panel
- shader authoring or shader/material attachment UI
- robust automatic GLB thumbnail framing
- material-slot editing beyond "apply one texture to the clicked renderable"
- generalized material authoring UI
- recursive asset-tree browser redesign

## Related

- [docs/task/assets-slection-and-paint-panels.md](/home/rei/_/cat-engine/docs/task/assets-slection-and-paint-panels.md)
- [docs/task/asset-preview-layout-resolution.md](/home/rei/_/cat-engine/docs/task/asset-preview-layout-resolution.md)
- [docs/task/paint-panel-selection-and-panel-focus.md](/home/rei/_/cat-engine/docs/task/paint-panel-selection-and-panel-focus.md)
- [docs/spec/texture.md](/home/rei/_/cat-engine/docs/spec/texture.md)
- [docs/spec/audio-sources.md](/home/rei/_/cat-engine/docs/spec/audio-sources.md)
