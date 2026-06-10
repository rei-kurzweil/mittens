# Task: Signal Observation Routing (ObserverRouter)

## Problem: Inefficient Global Subscriptions
Currently, editor panels like the Paint Tool subscribe to signals (Click, Drag, SelectionChanged) at the `editor_root` level. Because the `editor_root` is an ancestor of both the scene tree and the UI, these panels receive every interaction event regardless of focus.

- **Late Filtering**: Panels receive the event, log it, and then check `is_paint_active()` or `is_paint_panel_focused()`.
- **Overhead**: Every `DragMove` in the viewport triggers multiple ancestor-walk checks (`eligible_scene_hit`) even when the Paint panel is closed or unfocused.
- **Log Noise**: Console output is cluttered with inactive system traces.

## Proposed Solution: Local Scope Observation Filtering
Introduce a mechanism to filter signal handlers based on a dynamic data-driven policy attached to the same scope as the handler. This allows systems to stay subscribed to broad scopes (like `editor_root`) without processing events when they are "blacklisted" by a router on that specific scope.

### 1. `SignalObserverRouterComponent` (MMS: `ObserverRouter`)
This component acts as a filter for signal handlers attached to the **same node**.

- **Placement**: Attached to a node that has broad signal subscriptions (e.g., the `editor_root`).
- **Fields**:
    ```rust
    pub struct SignalObserverRouterComponent {
        /// Names of handlers that are forbidden from receiving signals bubbling through this node.
        pub blacklist: Vec<String>,
        
        /// (Optional) If true, only allow handlers present in this list. 
        /// If empty, whitelist is ignored.
        pub whitelist: Vec<String>,
    }
    ```
- **Local Responsibility**: It only filters handlers attached directly to its own node. It does **not** affect handlers on descendants (e.g., Gizmos) or ancestors.
- **Named Handlers**: Signal handlers registered via `RxWorld` must optionally support **names** (e.g., `"paint_system"`).
- **MMS Usage**:
    ```mms
    ObserverRouter { blacklist: ["paint_system"] }
    ```

### 2. High-Performance Execution & Safety
This model is highly efficient because it eliminates the need for a global "routing state" during signal bubbling.

- **Dispatch Logic**: When the `RxWorld` bubbling loop reaches a node, it checks for an `ObserverRouter`. If present, it skips any handlers on that node whose names appear in the router's blacklist.
- **Safety (Gizmos)**: Gizmo handlers are typically attached to the **gizmo handles themselves** or the **target object**, which are descendants of the `editor_root`. Because the `ObserverRouter` only affects its own node, a router on the `editor_root` will **never** block a Gizmo handler, even if they share a name.
- **Selectable.off() vs. ObserverRouter**: 
    - `Selectable.off()` is a semantic marker. Systems like the Editor or Paint Tool check it *after* receiving an event to decide if they should ignore it. 
    - `ObserverRouter` is a structural filter. It prevents the system from receiving the event entirely, saving the cost of the system's logic and checks.
    - These work together: `Selectable.off()` remains useful for permanent "non-pickable" UI, while `ObserverRouter` handles dynamic "unfocused" state.

### 3. Usage Pattern: Lateral Observation
1. **The Observer**: The Paint system registers a named handler (`"paint_system"`) on the `editor_root`.
2. **The Router**: An `ObserverRouter` is also attached to the `editor_root`.
3. **The Coordinator**: When the Paint panel loses focus, the Editor Coordinator adds `"paint_system"` to the `ObserverRouter`'s blacklist.
4. **The Result**: Signals (Click, Drag, etc.) bubble up from the scene to the `editor_root`. When they arrive, the `ObserverRouter` sees the `"paint_system"` handler is blacklisted and prevents it from running. Gizmo handlers on descendant nodes remain unaffected and fire normally. No `eligible_scene_hit` checks or log traces occur for Paint.

## Implementation Plan

### Phase 1: Research & Modeling
- [ ] Extend `RxWorld` and `Signal` handlers to support optional **names**.
- [ ] Define the `SignalObserverRouterComponent` (MMS: `ObserverRouter`) and `SignalObserverFilter` structures.
- [ ] Model how the signal bubbling loop can efficiently check for `ObserverRouter` components and apply blacklists.

### Phase 2: Refactor Paint System
- [ ] Remove `editor_root` scoped handlers from `editor_paint_system.rs`.
- [ ] Implement focus-aware routing for Paint events.
- [ ] Verify that `paint_debug` logs no longer appear when the Paint panel is unfocused.

### Phase 3: Generalize for Other Panels
- [ ] Apply the same pattern to the Inspector and World panels if applicable.
- [ ] Ensure that gizmo interactions (which should always be active for the selected object) are not accidentally blocked by panel focus routing.

## Constraints
- **No changes to `src/` yet**: This document serves as the plan for approval.
- **Maintain performance**: The routing check should be faster than the current multiple parent-walks.
- **Respect multi-editor workspaces**: The router should handle multiple scene trees targeting the same shared UI.
