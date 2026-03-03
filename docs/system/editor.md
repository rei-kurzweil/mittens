# Editor system (`EditorSystem`)

`EditorSystem` is the selection/attachment router for editor gizmos. It listens for `SignalValue::DragStart` (mouse-down on a ray-hit renderable) and, when the clicked renderable is inside an editor subtree, it re-attaches the editor’s `TransformGizmoComponent` to the clicked target so subsequent gizmo drags manipulate that target.

The system is implemented entirely as an immediate-mode Rx handler installed in `SystemWorld::tick()`. It does not maintain a global “selection” component; instead it emits a topology action (`SignalValue::Attach`) and relies on `ActionSystem` to perform the reparent, emit `ParentChanged`, and trigger transform refresh/init as needed.

## Data flows

| Code path | Trigger (input) | Guards / early returns | Lookups / traversal | Emitted action(s) | Downstream effects |
|---|---|---|---|---|---|
| Select target (happy path) | `DragStart { renderable }` | Renderable must *not* be a gizmo handle (no `TransformGizmoComponent` ancestor). Must be under an `EditorComponent` ancestor. Must have a `TransformComponent` ancestor. Must be able to find a `TransformGizmoComponent` in the editor subtree. | `nearest_editor_ancestor(renderable)` (walk parents). `nearest_transform_ancestor(renderable)` (walk parents). `resolve_editor_transform_gizmo(editor_root)` (cached id, else DFS subtree search). | `Attach { parents: [target_transform], child: transform_gizmo }` (scope = `editor_root`) | `ActionSystem` calls `World::add_child(parent, child)` (detaches from old parent automatically), emits `ParentChanged`, queues topology transform refresh and audio graph dirty. `TransformGizmoSystem` observes `ParentChanged` and rebinds `TransformGizmoComponent.target_transform` to the new parent transform. |
| Click on gizmo handle | `DragStart { renderable }` | If any ancestor of `renderable` has `TransformGizmoComponent`, return immediately. | `has_transform_gizmo_ancestor(renderable)` (walk parents). | None | No selection change; gizmo continues to operate normally on drag. |
| Click outside editor subtree | `DragStart { renderable }` | If no `EditorComponent` is found while walking parents, return. | `nearest_editor_ancestor(renderable)` (walk parents). | None | No selection change; editor gizmo is not moved. |
| Click editor object without transform | `DragStart { renderable }` | If under an editor root but no `TransformComponent` exists in its ancestry, return. | `nearest_editor_ancestor(renderable)` then `nearest_transform_ancestor(renderable)` (walk parents). | None | No selection change; this object is effectively non-selectable for transform gizmo attachment. |
| Editor has no transform gizmo | `DragStart { renderable }` | If `resolve_editor_transform_gizmo(editor_root)` returns `None`, return. | Fast path uses `EditorComponent.transform_gizmo` cache (validated). Fallback searches entire editor subtree via DFS (`find_transform_gizmo_in_subtree`). | None | No selection change; editor root is present but does not currently own a `TransformGizmoComponent` instance. |

## Notes / constraints (current behavior)

- Selection is driven by `DragStart` (mouse press) rather than a distinct “click” (press+release) signal.
- The editor gizmo is identified by searching the editor subtree for a `TransformGizmoComponent`. `EditorComponent` caches the resolved gizmo id in `EditorComponent.transform_gizmo` for subsequent selections.
- Reparenting is performed via the signal graph (`Attach` → `ActionSystem`) rather than directly mutating topology inside `EditorSystem`.
- Only the transform gizmo is handled right now; additional gizmo types should follow the same pattern (editor handler routes selection → emits `Attach` for the relevant gizmo).
