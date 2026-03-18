# FileTreePanel and panel prefabs

This document proposes `FileTreePanel`, an editor panel that displays a navigable file tree rooted at a configured path (default: `assets/` relative to the cat-engine working directory). It also establishes a general vocabulary for **panel prefabs** — the pattern that `FileTreePanel`, the inspector panel, and future panels all share.

---

## Panel prefabs: the general pattern

cat-engine has no "entity" concept and no prefab-cloning facility. Panels are **procedurally-built component subtrees**, constructed by a builder function and managed by a dedicated system. The pattern mirrors what `spawn_controller_cube` does in `examples/vr-input.rs` but at a higher level of abstraction.

Every panel type follows the same structure:

### 1. A marker component

A lightweight marker component placed at the root of the panel subtree:

- Stores panel-local state (scroll offset, selection, expanded rows, etc.)
- Identifies the panel type to the managing system
- Is the handle the system stores when it needs to update or tear down the panel

Examples today: `ComponentTreePanelComponent`, `ComponentInspectorPanelComponent` (from inspector-panel spec). `FileTreePanelComponent` is proposed here.

### 2. A builder function

A free function (not a method on any system) that constructs the full initial component subtree and returns the panel root `ComponentId`:

```rust
fn spawn_file_tree_panel(
    universe: &mut Universe,
    parent: ComponentId,
    config: FileTreePanelConfig,
) -> ComponentId
```

The builder function is **dumb** — it creates the topology and initial visuals but knows nothing about current data. The owning system fills in content.

### 3. An owning system

A system (or subsystem within `EditorSystem`) that:

1. **Spawns** the panel subtree when needed (if the panel root id is `None`).
2. **Updates** the panel's visual rows each tick (or on data change events).
3. **Handles** interactions: clicks, scrolls, hover state.
4. **Destroys** the panel via `RemoveSubtree` intent when it should no longer exist.

The system stores the panel root `ComponentId` in its own state. Importantly, the panel marker component does not reach back into the system — data flows one way: system → panel topology.

### 4. Primitives used

All panels are built from the same low-level component vocabulary:

| Purpose | Component |
|---|---|
| Overlay render pass | `OverlayComponent` (ancestor) |
| Positioning and sizing | `TransformComponent` |
| Background quad | `RenderableComponent` + `ColorComponent` |
| Row icon | `RenderableComponent` (cube or small quad mesh) |
| Row label | `TextComponent` |
| Clickable row | `RaycastableComponent` |
| Scroll clip region | (not yet defined; open question) |

### MMS representation (future)

Once MMS component expressions are evaluable, a panel builder call is a good candidate for a named constructor:

```txt
FileTreePanel.new("/assets") {
    // rows managed by FileTreeSystem, not declared here
}
```

The body here would be empty because the system generates and updates the rows dynamically. The `.new("/assets")` pre-body call sets the root path at construction time.

---

## FileTreePanel

### What it shows

A scrollable, hierarchical list of files and directories rooted at a configured path.

- **Directory rows**: expandable/collapsible; show a folder icon + directory name.
- **File rows**: leaf nodes; show a file-type icon + file name.
- **Nesting**: visual indentation at depth `d` by `d * indent_units` in local space (same model as the component tree panel).

Default root path: `assets/` relative to the cat-engine working directory.

### `FileTreePanelConfig`

Construction-time settings passed to the builder function:

```rust
struct FileTreePanelConfig {
    root_path: PathBuf,       // default: "assets/"
    width: f32,               // panel width in world units
    row_height: f32,          // row height in world units
    max_visible_rows: usize,  // before scrolling
}
```

### `FileTreePanelComponent`

Runtime state stored on the panel marker component:

```rust
struct FileTreePanelComponent {
    // config
    root_path: PathBuf,

    // view state
    scroll_offset_rows: i32,
    expanded_dirs: HashSet<PathBuf>,
    selected_path: Option<PathBuf>,

    // layout cache (rebuilt by system on data change)
    visible_rows: Vec<FileTreeRow>,

    // internal ids (children owned by this panel)
    row_nodes: Vec<ComponentId>,  // existing row subtree roots, reused/recycled
}
```

### Row data model

Each visible row in the flat display list:

```rust
struct FileTreeRow {
    path: PathBuf,
    kind: FileTreeRowKind,
    depth: usize,
    is_expanded: bool,   // only meaningful for Dir
    is_selected: bool,
}

enum FileTreeRowKind {
    Dir,
    File { extension: Option<String> },
}
```

### Interaction model

Clicks on a row (`RaycastableComponent` → `DragStart` or click signal) are handled by the owning system:

- **Click a file row**: emit `FileSelected { path }` event. Other systems (e.g. an asset importer, a texture previewer) can listen.
- **Click a directory row**: toggle `expanded_dirs` for that path, then request a panel rebuild.
- **Scroll**: increment/decrement `scroll_offset_rows`, then request a panel rebuild.

Clicks inside the panel subtree must not feed into the editor's scene selection. Use a `NonInspectableComponent` marker (or equivalent opt-out tag, as discussed in inspector-panel.md) on the panel root.

### Builder function sketch

```rust
fn spawn_file_tree_panel(
    universe: &mut Universe,
    parent: ComponentId,
    config: FileTreePanelConfig,
) -> ComponentId {
    // 1. Create panel marker
    let panel = universe.world.add_component(
        FileTreePanelComponent::new(config.root_path.clone())
    );
    universe.attach(parent, panel);

    // 2. Background quad at panel root
    let bg_t = universe.world.add_component(
        TransformComponent::new()
            .with_scale(config.width, config.row_height * config.max_visible_rows as f32, 0.01)
    );
    universe.attach(panel, bg_t);
    let bg = universe.world.add_component(RenderableComponent::quad());
    universe.attach(bg_t, bg);
    let bg_color = universe.world.add_component(ColorComponent::rgba(0.1, 0.1, 0.1, 0.85));
    universe.attach(bg, bg_color);

    // 3. Row slots are created lazily or pre-allocated by FileTreeSystem on first tick.

    panel
}
```

Rows are **not** created in the builder — they are created and recycled by `FileTreeSystem` on each rebuild. This keeps the builder fast and allows the system to reuse existing row nodes rather than destroying and recreating them.

---

## AssetSystem

`AssetSystem` is the data-supply layer for file and asset management. It is **not** responsible for rendering; it provides structured filesystem data that `FileTreeSystem` (or other systems) can query and display.

### Responsibilities

1. **Directory watching**: maintain a current snapshot of the filesystem rooted at one or more configured paths.
2. **File tree queries**: answer "what are the children of path P?" efficiently from the snapshot.
3. **Asset identification**: optionally tag known asset types (`.gltf`, `.dds`, `.mms`, etc.) for display and loading hints.
4. **Read/write**: provide `load_file(path)` and `write_file(path, bytes)` for other systems that need raw access (scene loader, texture importer, `.mms` evaluator).
5. **Change events**: emit `FileTreeChanged { root }` when the snapshot is updated so panels know to refresh.

### File tree data model

```rust
enum FileNode {
    Dir  { path: PathBuf, children: Vec<FileNode> },
    File { path: PathBuf, extension: Option<String>, size_bytes: u64 },
}
```

`AssetSystem` holds a `root_snapshot: Option<FileNode>` that is refreshed either:

- **On tick** (polled, e.g. every N seconds), or
- **Via OS file-watcher events** (preferred for responsiveness; out of scope for v1).

### Signals

New event to emit when the snapshot changes:

```rust
EventSignal::FileTreeChanged { root_path: PathBuf }
```

New event for file selection from the panel:

```rust
EventSignal::AssetSelected { path: PathBuf }
```

`AssetSystem` itself does not read selection events — it only supplies data. Selection events are produced by `FileTreeSystem` and consumed by whichever system acts on asset selection (e.g. a future asset importer or texture preview system).

### v1 scope

For v1, `AssetSystem` can be minimal:

- Read the directory tree once at startup (or on demand).
- No OS file-watching.
- `load_file(path) -> Result<Vec<u8>>` backed directly by `std::fs`.
- No write support initially.

---

## FileTreeSystem

`FileTreeSystem` is the managing system for `FileTreePanelComponent` instances. It sits between `AssetSystem` (data) and the panel component subtree (visuals).

### Responsibilities

1. **Panel existence**: ensure a `FileTreePanel` subtree exists wherever one is needed (e.g. inside the editor UI root). Spawn via `spawn_file_tree_panel` if not present.
2. **Data binding**: on each tick (or on `FileTreeChanged`), query `AssetSystem` for the current tree, flatten into `visible_rows` accounting for `scroll_offset_rows` and `expanded_dirs`, and update the panel's row component subtrees.
3. **Row recycling**: maintain a pool of row subtrees (icon + text per row) — update text content and icon color rather than destroying and recreating. This is the same pattern `InspectorSystem` would use for component tree rows.
4. **Interaction dispatch**: handle `DragStart` / click signals on row renderables, update panel state, emit `AssetSelected`.

### Tick order

`FileTreeSystem` ticks after `AssetSystem` (which may have updated its snapshot) and after raycasting + gesture (which supply click signals). Within the broader tick order in `SystemWorld::tick()`, this places it in step 10 alongside editor, text, and other UI systems.

---

## How panels fit together in the editor

The full editor UI is expected to be a collection of panel subtrees all owned by their respective systems, parented under a shared overlay root:

```
OverlayComponent
  TransformComponent (editor UI anchor)
    EditorComponent
      ComponentTreePanel     ← owned by InspectorSystem
      ComponentInspectorPanel ← owned by InspectorSystem
      FileTreePanel           ← owned by FileTreeSystem
```

Each panel is positioned by its parent `TransformComponent`'s TRS. No panel knows about the others. Layout (where panels are positioned relative to each other) is either hardcoded in the editor root builder or driven by a future layout system.

`EditorSystem` remains the selection router for scene objects. It does not manage panel subtrees directly — it delegates to `InspectorSystem` and `FileTreeSystem` for panel lifecycle.

---

## Open questions

- **Row recycling vs. full rebuild**: the text-system generates glyph subtrees that are expensive to destroy and recreate. Should we diff the visible row list and only update changed rows? Or is full teardown+rebuild acceptable for short panel lists?
- **Scroll clipping**: there is no clip-region primitive yet. Options: hide out-of-range rows by moving them off-screen (opacity=0 or T out of view), or implement a clip mask in the render pipeline.
- **File watching**: v1 polls on demand; a future OS-watcher integration (via `notify` crate or similar) would improve responsiveness. `AssetSystem` should be structured so the snapshot-update path is isolated and the signal emission works either way.
- **Write support**: needed for saving `.mms` scenes, custom component serialization, etc. `AssetSystem::write_file` is a natural home but needs careful threading (file I/O off main thread, result back via command queue).
- **Asset type registry**: who decides `.gltf` → "3D model" → which icon/color? `AssetSystem` could hold a registry, or that can be left to `FileTreeSystem` as a display concern.
- **`NonInspectableComponent` naming**: inspector-panel.md leaves this open. FileTreePanel also needs to opt out of scene picking; resolving the name is worth doing before both panels are implemented.
