# Task: Signal Observation Routing (ObserverRouter)

## Problem: Inefficient Global Subscriptions
Currently, editor panels like the Paint Tool subscribe to signals (Click, Drag, SelectionChanged) at the `editor_root` level. Because the `editor_root` is an ancestor of both the scene tree and the UI, these panels receive every interaction event regardless of focus.

- **Late Filtering**: Panels receive the event, log it, and then check `is_paint_active()` or `is_paint_panel_focused()`.
- **Overhead**: Every `DragMove` in the viewport triggers multiple ancestor-walk checks (`eligible_scene_hit`) even when the Paint panel is closed or unfocused.
- **Log Noise**: Console output is cluttered with inactive system traces.

## Proposed Solution: Lateral Observation Routing
Introduce a mechanism to filter signal observations based on a dynamic data-driven policy. This allows systems to stay subscribed to broad scopes without processing events when they are "blacklisted" by a local router.

This differs from `SignalRouteUpwardComponent` (which routes *intents* to ancestors) by filtering *observations* (signals already in the bubbling phase).

### 1. `SignalObserverRouterComponent` (MMS: `ObserverRouter`)
This component acts as a generic gateway for signals emitted within its subtree. It is entirely agnostic of higher-level concepts like "Editor" or "Workspace".

- **Placement**: Attached to a node whose signals should be filtered (e.g., the Active Editor Root).
- **Responsibility**: It holds a `SignalObserverFilter` that determines which handlers are allowed to receive events bubbling through this node.
- **Named Handlers**: To enable filtering, signal handlers registered via `RxWorld` (or MMS) must optionally support **names** (e.g., `"paint_system"`, `"gizmo_system"`).
- **SignalObserverFilter**: A data structure (likely a blacklist/whitelist of handler names) that the component uses to intercept and block signals.

### 2. Focus-Aware Filtering
Higher-level systems (like an Editor Coordinator) are responsible for updating the `SignalObserverFilter` on the router.

- **Dynamic Updates**: When the `focused_panel` changes, the coordinator updates the `SignalObserverRouterComponent`'s filter.
- **Blacklisting**: For example, if the Paint panel is not focused, the coordinator adds `"paint_system"` to the blacklist of the `ObserverRouter` on the active editor root.
- **Decoupling**: The `ObserverRouter` doesn't know *why* a handler is blacklisted; it just enforces the current data-driven policy.

### 3. Lateral Observation
This mechanism allows observers (topologically distant systems or panels) to subscribe to broad scopes (like `editor_root`) without incurring the cost of processing every event when inactive. 

1. **Broad Subscription**: A system registers a **named handler** on a high-level scope.
2. **Local Enforcement**: An `ObserverRouter` placed further down the tree (near the event source) blocks that named handler from receiving the signal if the current filter forbids it.

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
