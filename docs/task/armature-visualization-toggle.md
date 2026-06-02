# Armature Visualization Toggle

## Context
We need a way to toggle the visibility of bone markers/locators (armature visualization) in the editor without disrupting the engine's runtime state or losing performance. Currently, armatures are often baked into the glTF scene hierarchy, and bone markers might be children of the joints themselves.

## Goal
Design and implement a system to toggle bone marker visibility smoothly.

## Constraints & Considerations
- **Runtime Disruption:** Re-building every glTF tree in the world to apply a visibility setting might be too heavy and could reset component states (animations, IK targets, etc.).
- **Topology:** If bone markers are separate nodes from the joints, we can toggle their visibility/opacity or detach/reattach them.
- **Performance:** Iterating over every bone in a complex rig every frame is not ideal. Using a system-level flag or a specific component for "BoneViz" nodes might be better.
- **Selection:** Bone markers are usually raycastable for selection; toggling visibility should probably also toggle raycastability.

## Proposed Approaches
1. **Separate Viz Tree:** Maintain bone markers in a separate tree that follows the joints via transform routing. Toggling visibility is then a single node toggle.
2. **Tagging/Component Filter:** Tag bone marker nodes with a `BoneMarker` component. A system can then update their `Opacity` or `Hidden` status en masse.
3. **MMS Re-render:** If the editor state changes, re-invoke the factory that creates the armature visualization. We need to Ensure this doesn't kill the underlying joint components.

## Task Breakdown
- [ ] Research current bone marker spawning logic in `SkinnedMeshSystem` or `GltfSystem`.
- [ ] Investigate if `OpacityComponent` or a `Hidden` flag is sufficient for high-performance toggling.
- [ ] Decide on the hierarchy strategy (nested vs. split).
- [ ] Create `editor_settings_panel.mms` to house the toggle.
