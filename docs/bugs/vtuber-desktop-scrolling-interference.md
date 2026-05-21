# Scrolling in vtuber-desktop is difficult due to raycast interference

## Status

Open bug / investigation.

## Symptom

In the `vtuber-desktop` example, scrolling the "World" panel is inconsistent and difficult. It appears that objects located behind the world panel (like the decorative cubes in the background) are being prioritized or "stolen" by the raycasting system during drag operations, preventing the scroll system from receiving the necessary `DragMove` events.

## Repro

- [examples/vtuber-desktop.mms](../../examples/vtuber-desktop.mms)
- [examples/vtuber-desktop.rs](../../examples/vtuber-desktop.rs)
- Uses [assets/components/world-panel.mms](../../assets/components/world-panel.mms)

Steps to reproduce:
1. Run the `vtuber-desktop` example.
2. Attempt to scroll the yellow content area of the "World" panel by clicking and dragging.
3. Observe that the scroll often fails to engage, especially when background cubes are visible behind the panel.

## Expected behavior

The World panel (being an `Overlay` and positioned in front of the background cubes) should capture and consume raycast hits and subsequent drag events. Background objects should not interfere with interactions on foreground UI panels.

## Actual behavior

Raycasting seems to "pierce" through the UI panel or prioritize background objects, causing `DragMove` events to either not be emitted for the panel or be intercepted by background entities.

## Likely investigation targets

- `src/engine/ecs/system/raycast_system.rs`: How are multiple hits handled and sorted?
- `src/engine/ecs/system/gesture_system.rs`: How does the `DragUpdatePolicy` and the initial hit detection logic handle layered objects?
- `src/engine/ecs/system/scroll_system.rs`: How is the `drag_scope` assigned and does it properly capture events when foregrounded?
- `assets/components/world-panel.mms`: Check the `PointerEvents` and `Raycastable` configuration for the panel and its content slot.

## Questions to answer

- Is the `RayCastSystem` returning hits in the correct front-to-back order?
- Does the `GestureSystem` correctly pick the *topmost* hit that captures drags?
- Is there a "z-fighting" or precision issue in local-to-world ray transformation for UI overlays?
- Does the `ScrollingSystem`'s `drag_scope` (often the `__bg` of a layout node) correctly cover the intended interaction area?
- Why do background cubes (standard `R.cube()` with `ED` scope) seem to take precedence over `Overlay` elements?
- Does the fact that the panel is inside an `Overlay` and a `LayoutRoot` affect its raycast priority or hit detection?
- Is the `PointerEvents` configuration on the background cubes (likely `All` by default) interfering with the UI's attempt to capture the drag?
