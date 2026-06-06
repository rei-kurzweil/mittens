# GrabHandle Component

## Overview

The `GrabHandle` component allows any `Transform` (represented in MMS as `T {}`) to act as a drag handle for a target transform. It is designed to work seamlessly with the layout system, where styling is applied via a `Style` component child (`T { Style { ... } }`).

In a layout context, the `LayoutSystem` generates a `__bg` child transform with a quad renderable to represent the box's background. The `GrabHandleSystem` ensures these generated surfaces (or any other renderables under the `T`) become valid drag targets without requiring the user to manually configure raycasting for every handle.

## Component Definition

```rust
pub struct GrabHandleComponent {
    /// The transform that will be moved when this handle is dragged.
    /// If None, defaults to the immediate parent of the entity holding this component.
    pub target_transform: Option<ComponentId>,

    /// Optional coordinate space for the drag. 
    /// Defaults to WorldPlane.
    pub coord_type: GestureCoordType,
    
    /// Whether the handle is currently active.
    pub enabled: bool,
    
    component_id: Option<ComponentId>,
}

impl Component for GrabHandleComponent {
    fn name(&self) -> &'static str { "grab_handle" }
    fn set_id(&mut self, id: ComponentId) { self.component_id = Some(id); }
    // ... as_any etc
}
```

## Interaction Model & Layout Integration

1. **Automatic Raycasting**: If a `T` has both `GrabHandle` and `Style`, the `GrabHandleSystem` should ensure a `RaycastableComponent` exists on the `T` (if not already provided by the author). 
2. **Grafting**: The `LayoutSystem` automatically finds this `RaycastableComponent` and grafts it onto the generated `__bg` renderable.
3. **Pointer Events**: By default, `GrabHandle` should use `PointerEvents::DragOnly`. This allows the background of a panel to be draggable while still allowing clicks to pass through to nested buttons or links.
4. **Signal Bubbling**: Since the `__bg` quad is a descendant of the `T` holding the `GrabHandle`, `DragStart/Move/End` signals emitted by the `__bg` quad bubble up and are caught by handlers installed on the `T` node.

## GrabHandleSystem Responsibilities

### 1. Initialization & Configuration
- Monitor entities with `GrabHandleComponent`.
- Install scoped handlers on the `T` node for `DragStart`, `DragMove`, and `DragEnd`.
- If the node has a `StyleComponent` but no `RaycastableComponent`, the system should add `RaycastableComponent { pointer_events: DragOnly }` to ensure the layout-generated `__bg` quad becomes interactive.

### 2. Target Resolution
When a drag occurs, the system resolves the `target_transform`:
- If `GrabHandle.target_transform` is `Some(id)`, use it.
- If it is `None`, use the parent of the `GrabHandle`'s owner (the `T`).

### 3. Drag Handling
- On `DragMove`: 
    - Resolve the `target_transform`.
    - Apply `delta_world` to the `target_transform`.
    - Emit `IntentValue::UpdateTransform`.

## MMS Usage Examples

### Simple Draggable Panel
The entire panel background becomes a handle.

```mms
T {
    name = "my_panel"
    Style { 
        width(400)
        height(300)
        background_color(#222)
    }
    // Clicking anywhere on the #222 background drags the panel
    GrabHandle { target_transform: self }
    
    T {
        Style { padding(20) }
        Text { "Drag my background to move me!" }
    }
}
```

### Dedicated Title Bar
Only a specific top area moves the panel.

```mms
T {
    name = "window"
    Style { ... }

    // Title Bar
    T {
        Style { 
            height(30)
            background_color(#444)
        }
        // Drags the 'window' transform
        GrabHandle { target_transform: parent }
        Text { "Title Bar" }
    }
    
    // Content area (not draggable)
    T { ... }
}
```

## Design Considerations

- **Coordinate Spaces**: For 2D-in-3D UI, `WorldPlane` projection is generally preferred. The system should ensure the drag plane normal is consistent with the panel's orientation.
- **Nesting**: Scoped handlers correctly handle nested `GrabHandle`s (e.g., a small "reorder" handle inside a larger draggable card).
- **Redundancy**: If the user provides their own `RaycastableComponent`, the `GrabHandleSystem` should respect it but maybe warn if `pointer_events` is set to `All` (which might block clicks to children).
