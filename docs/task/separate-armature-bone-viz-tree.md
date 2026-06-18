# Separate Armature Bone Viz Tree for Runtime Toggle

Date: 2026-06-18

## Context

We need a runtime toggle for bone visualization on GLTF armatures from MMS:

```mms
let gltf = GLTF.new("assets/models/bisket.8.0.glb")
gltf.enable_bone_visualization(true)
# ... later ...
gltf.enable_bone_visualization(false)
```

Currently, bone viz markers are spawned as direct children of real armature bone
transforms during GLTF load (`gltf_system.rs:982-1027`). This makes runtime toggle
impossible without re-spawning the entire GLTF subtree (which would destroy AVC/IK
wiring) or leaving viz clutter in the armature tree.

The solution is simple: remove viz from GLTF load, add it on demand via a new
`BoneVisualizationSystem`, and remove it via the existing `RemoveSubtree` path.
The viz proxies remain children of the real bones (so transform inheritance and
gizmo routing via `SignalRouteUpwardComponent` work naturally).

## New intent

```rust
// src/engine/ecs/rx/signal.rs
IntentValue::ToggleBoneVisualization {
    component_id: ComponentId,   // the GLTFComponent
    enabled: bool,
}
```

## BoneVisualizationSystem

```rust
// src/engine/ecs/system/bone_viz_system.rs
pub struct BoneVisualizationSystem {
    /// gltf_cid → [viz_proxy_root_cids]
    active_viz: HashMap<ComponentId, Vec<ComponentId>>,
    /// gltf_cid → [bone_transform_cids]
    bone_cache: HashMap<ComponentId, Vec<ComponentId>>,
}
```

Does not tick — fully event-driven via `handle_toggle()`.

### handle_toggle

```
enabled && no active viz for gltf:
  1. discover bones: walk gltf subtree for Transforms with BoneRestPoseComponent children
  2. cache in bone_cache
  3. for each bone:
     spawn as child of bone:
       viz:{name} (TransformComponent, scale=0.03)
         → SignalRouteUpwardComponent("update_transform", "transform")
         → OverlayComponent
           → viz_box (RenderableComponent::cube())
             → RaycastableComponent + ColorComponent{rgba:[1,1,1,1]}
       add Serialize.off() to viz proxy
  4. init_component_tree on each viz root
  5. store in active_viz[gltf_cid]

!enabled && has active viz:
  1. for each root in active_viz[gltf_cid]: emit RemoveSubtree { component_ids: [root] }
  2. clear active_viz[gltf_cid]
```

## GLTFSystem changes

1. **Delete** lines 982-1027 from `spawn_node_recursive` (no viz at load)
2. **Remove** `with_visualized_transforms` param from `spawn_node_recursive`
3. **After spawn, if** `has_editor_ancestor`**: set flag + emit ToggleBoneVisualization

```rust
// In tick_with_queue, after spawned_components.insert(cid):
if has_editor_ancestor(world, cid) {
    if let Some(c) = world.get_component_by_id_as_mut::<GLTFComponent>(cid) {
        c.with_visualized_transforms = true;
    }
    emit.push_intent_now(cid, IntentValue::ToggleBoneVisualization {
        component_id: cid,
        enabled: true,
    });
}
```

## MMS binding

In `component_registry.rs:apply_call`:

```rust
"enable_bone_visualization" => {
    *gltf = gltf.clone().with_visualized_transforms(arg_bool(args, 0)?);
}
```

MMS handler also emits `ToggleBoneVisualization` intent. This requires the MMS
`apply_call` to have access to an `emit` handle. If `apply_call` doesn't currently
have one, the MMS handler sets the flag and `BoneVisualizationSystem` can also
poll newly-set flags on already-spawned GLTFs as a fallback path.

## Editor integration

### Checkmark icon (`assets/components/icons.mms`)

```mms
export fn checkmark_icon() {
    return T {
        name = "checkmark_icon"
        T.rotation(0.0, 0.0, -0.785) {
            T.scale(0.11, 1.5, 0.1) {
                R.cube() { C.rgba(0.25, 0.75, 0.25, 1.0) }
            }
        }
        T.position(0.25, 0.5, 0.0).rotation(0.0, 0.0, 0.45) {
            T.scale(0.11, 0.65, 0.1) {
                R.cube() { C.rgba(0.25, 0.75, 0.25, 1.0) }
            }
        }
    }
}
```

### Settings row (`panels.mms`)

```mms
import { checkmark_icon } from "./icons.mms"

fn editor_settings_toggle_row(row_name, label) {
    return T {
        name = row_name
        Option {
            Data {
                name = "editor_settings_payload"
                row_kind = "ToggleShowBones"
                row_name = row_name
                label = label
                interactive = true
            }
        }
        Raycastable.click_only()
        Style { display("block") width(100%) margin_xy(0.25, 0.20)
                padding_xy(0.55, 0.45) color([0,0,0,1.0])
                background_color([0.92,0.97,0.92,1.0]) background_z(-0.01)
                text_align("left") vertical_align("middle") }
        T { Text { label } }
        T.position(9.0, 0.0, 0.0) { name = "checkmark_slot" }
    }
}
```

Added in `editor_settings_panel`:
```mms
editor_settings_toggle_row("editor_settings_show_bones", "Show Bones")
```

### Editor state and event (`editor/context.rs`)

```rust
pub struct EditorContextState {
    // ...existing fields...
    pub show_bones: bool,  // default false
}

pub enum EditorContextEvent {
    // ...existing variants...
    ShowBonesToggled { editor: Option<ComponentId> },
}
```

In `editor_context_event_from_shared_signal`, when `is_editor_settings_selection`:
check `row_kind` from payload. If `"ToggleShowBones"`, return
`ShowBonesToggled { editor }`.

In `reduce_editor_context_state`:
```rust
EditorContextEvent::ShowBonesToggled { .. } => {
    new.show_bones = !old.show_bones;
}
```

### Sync function

```rust
fn sync_bone_viz_for_editor(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    state: &Arc<Mutex<EditorContextState>>,
    workspace: &Arc<Mutex<EditorContextWorkspaceState>>,
) {
    let ctx = state.lock().expect("poisoned").clone();
    let ws = workspace.lock().expect("poisoned").clone();
    let Some(panel_root) = ws.panel_query_root else { return; };

    // Mount/clear checkmark icon in slot
    let slot = world.find_component(panel_root, "#checkmark_slot");
    if ctx.show_bones {
        if slot.is_some_and(|s| !has_checkmark_child(world, s)) {
            // Spawn checkmark_icon() as child of slot
        }
    } else {
        if let Some(slot) = slot {
            for child in world.children_of(slot).to_vec() {
                emit.push_intent_now(child,
                    IntentValue::RemoveSubtree { component_ids: vec![child] });
            }
        }
    }

    // Find GLTFs under each editor and emit toggle intents
    for &editor in &ws.registered_editors {
        let gltfs: Vec<ComponentId> = world.all_components()
            .filter(|&cid| world.parent_of(cid) == Some(editor)
                || is_descendant_of(world, cid, editor))
            .filter(|&cid| world.get_component_by_id_as::<GLTFComponent>(cid).is_some())
            .collect();
        for gltf_cid in gltfs {
            emit.push_intent_now(gltf_cid, IntentValue::ToggleBoneVisualization {
                component_id: gltf_cid,
                enabled: ctx.show_bones,
            });
        }
    }
}
```

Called from the `install_shared_panel_handlers` closure after
`apply_editor_context_event`.

## Files changed

| File | Change |
|------|--------|
| `src/engine/ecs/rx/signal.rs` | New `ToggleBoneVisualization` intent variant |
| `src/engine/ecs/rx/signal_pipeline_processor.rs` | Handle new intent's component_ids |
| `src/engine/ecs/rx/mutation_executor.rs` | Handle intent → call `BoneVisualizationSystem` |
| `src/engine/ecs/system/bone_viz_system.rs` | New system: `handle_toggle`, `active_viz` tracking |
| `src/engine/ecs/system/gltf_system.rs` | Remove viz from spawn, editor auto-enable emits intent |
| `src/engine/ecs/system/editor/context.rs` | `show_bones` state, `ShowBonesToggled` event, sync function |
| `src/meow_meow/component_registry.rs` | `enable_bone_visualization` MMS handler |
| `assets/components/icons.mms` | New `checkmark_icon()` |
| `assets/components/panels.mms` | New `editor_settings_toggle_row()`, import icon, add row |

## Acceptance criteria

- [ ] `gltf.enable_bone_visualization(true/false)` via MMS toggles viz
- [ ] Viz proxies are children of real bones, inherit transforms naturally
- [ ] Gizmo drag on viz cube routes to real bone via existing `SignalRouteUpwardComponent`
- [ ] Editor "Show Bones" checkbox toggles all GLTF viz under the editor
- [ ] Checkmark icon mounts/clears in the settings row slot
- [ ] Removing viz proxies cleans up routes, renderables, transforms via existing `RemoveSubtree` handlers
- [ ] `has_editor_ancestor` at spawn time auto-enables viz
- [ ] No perf impact when viz is disabled (no polling, no stale components)
