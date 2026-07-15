# Bisket Editor First-Click and Visualization Transform Regressions

## Status

Fix implemented with focused regression coverage and desktop Bisket runtime validation. OpenXR
runtime validation remains pending.

## Root Cause and Fix

The first bad input transition was panel hit ownership, not bounds construction.
Panel controls author `Raycastable` as an immediate sidecar of a transform/layout owner, while the
BVH lookup only recognized a raycastable directly below the final renderable or as a structural
ancestor. Style-generated panel geometry could therefore miss its row's raycast policy on the
first frame and allow scene geometry, including the XR marker, to win the gesture.

`BvhSystem::find_raycastable_for_renderable` now recognizes an immediate raycastable sidecar at
every owner in the renderable ancestry chain. The nearest owner still wins, preserving explicit
row overrides and disabled markers. Panel roots contribute an explicit higher interaction-layer
priority so their controls win over overlapping world geometry. Editor registration also activates
the default editor without implicitly making the editor root a semantic selection; selection begins
only after an explicit valid interaction.

One transform propagation defect explains delayed bounds updates:

- independently rooted `TransformParent` followers were invalidated only when their target's
  nearest transform changed directly, not when an ancestor changed; bounds could remain stale and
  then visibly catch up during a later visualization action;
Follower invalidation now includes changed ancestors. The Bisket VR demo does not instantiate its
available secondary-motion preset, and that preset contains hair and bust chains only, so secondary
motion cannot explain the observed tail jump.

Focused integration coverage verifies that:

- editor startup has an active editor but no selected component;
- runtime panel title geometry is rejected by world-scene hit resolution;
- attaching a gizmo does not move its target or an unrelated transform follower;
- a bounds follower updates when an ancestor of its target moves;
- enabling armature visualization does not alter a joint's world transform;
- GLTF bounds are hidden by default;
- procedural wireframe boxes retain unit-box extents and share cached geometry by thickness.

The existing bounds ownership changes remain intact: markers are separate non-selectable,
non-raycastable followers and only materialize after imported renderables resolve. Visible bounds
now use a solid-edge `Renderable.wireframe_box(thickness)` mesh instead of translucent cubes.

## Runtime Context

- Example/world: Bisket avatar scene (`bisket-vr-demo`)
- Armature visualization uses the previously working joint-child marker implementation.
- Bounds visualization is disabled by default and can be enabled from Editor Settings.
- Bounds markers are independently owned, non-selectable `TransformParentComponent` followers.
- Bounds markers use cached unit wireframe-box geometry with edge thickness set to `0.02`.
- Imported mesh bounds are not materialized until the renderable has resolved, avoiding the old
  flat placeholder boxes at the avatar's feet.

## What Currently Works

- On initial world load, Bisket's bounding boxes have the expected shape and placement.
- Armature visualization works after the editor/settings interaction path becomes active.
- The restored armature markers remain selectable and route gizmo edits upward to their joints.

## Reproduction Sequence

Start from a fresh launch and do not interact with the editor before each sequence.

### Exact mode-click sequence (2026-07-14 retest)

1. Fresh-launch `bisket-vr-demo`.
2. Click **Select + Cursor** without first selecting scene geometry.
3. Observe a gizmo/marker materialize at the avatar's feet.
4. Click **3D Cursor**.
5. Observe the tail and its bounds move upward near the settings title-bar height.
6. Click **Select**.
7. Observe the remaining avatar bounds move upward, except for the head bounds.

Changing interaction mode must not create a transform gizmo: the only production path that creates
the shared gizmo is `select_editor_target`, so the first observation proves that a scene-selection
path (or a stale queued scene selection) is still executing during the settings click. The head-only
exception also points away from bounds construction and toward inconsistent transform-stream basis
refresh across different avatar branches.

Raycast tracing identified the duplicate scene selection. A desktop click activated two pointers:

- the desktop-camera ray hit the settings control with interaction priority 100;
- the `CameraXR` child pointer also consumed the mouse button because pointer activation treated
  every non-controller pointer as desktop input. Its `ParentForward` ray started at
  `[0.0, 0.08, 0.12]` and hit `Body.001` at `t=0.11`.

In the desktop/no-HMD run, the latter origin is the authored `xr_camera_wrapper` offset near the
identity XR pose rather than the visible desktop camera. This requires separate validation of the
intended no-HMD CXR pose, but it must not participate in desktop clicking regardless. Desktop mouse
activation now excludes pointers beneath `CameraXR`/`InputXR`; controller-owned pointers continue
using their corresponding XR trigger.

### Visualization toggles

1. Click **Show armature** or **Show bounds** in Editor Settings.
2. Observe that the toggle does not take effect while the settings panel has not been focused.
3. Click the Editor Settings title bar to focus the panel.
4. Try the toggle again.

Observed: both visualization controls depend on first focusing the panel.

Expected: settings controls work immediately. Panel focus may change editor UI state, but must not
gate whether their click handlers or GLTF visibility updates run.

### First scene/UI click

1. Fresh-launch the world.
2. Click either the Editor Settings title bar or a terrain cube.

Observed: the first click selects the blue XR camera / XR viewport-rig marker and moves it down to
the avatar's feet.

Expected: a panel title-bar click only focuses the panel. A terrain click selects or places the
cursor on the terrain according to the active interaction mode. Neither click should select or
move the XR rig marker.

### Second click

1. Continue from the first-click sequence.
2. Click any panel or scene target again.

Observed:

- the tail moves upward by approximately the avatar's height;
- the tail's bounds marker follows it;
- the remaining avatar bounds move even farther upward.

Expected: unrelated model joints and bounds retain their world transforms. Bounds may follow a
target that genuinely moves, but must never initiate or amplify target movement.

## Important Diagnostic Clues

- The bounds are correct before input, so GLTF local AABB extraction and initial resolved-mesh
  marker construction are likely sound.
- The tail and its bounds move together, which suggests the follower is reflecting a corrupted
  target transform rather than directly moving the tail.
- Other bounds moving farther than the tail suggests a world basis may be applied twice after the
  first selection/focus mutation.
- A panel click and a terrain click can trigger the same first-click failure. The common path is
  more likely editor activation/selection/cursor initialization than a settings-row handler alone.
- The blue XR rig being selected first suggests stale or incorrect initial editor selection,
  gizmo target, cursor host, or active-editor state.
- Bounds and armature visibility requiring panel focus suggests handler installation or scoped
  signal delivery is occurring too late, or depends incorrectly on `focused_panel`.

## Areas to Audit

### Editor startup state

- `EditorContextState` initialization and `ensure_default_active_editor`
- initial selection and shared gizmo target materialization
- shared 3D cursor host initialization
- whether the XR camera/viewport marker is accidentally retained as an initial semantic selection

Relevant code:

- `src/engine/ecs/system/editor/context.rs`
- `src/engine/ecs/system/editor_system.rs`
- `src/engine/ecs/system/cursor_3d.rs`
- `src/engine/ecs/system/gizmo_system.rs`
- `src/engine/ecs/system/editor_scene_hit.rs`

### Panel signal installation and focus

- when shared panel click handlers are installed relative to the first frame/input drain
- whether Editor Settings clicks are scoped to an inactive root until title-bar focus
- whether a panel click also leaks through to global scene selection/cursor handlers

Relevant code:

- `src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs`
- `src/engine/ecs/system/editor/settings_panel.rs`
- `src/engine/ecs/system/editor/inspector_panel.rs`
- `src/engine/ecs/system/gesture_system.rs`

### Transform mutation and propagation

- the exact first and second `UpdateTransform` recipients
- any selection/gizmo initialization that writes an XR rig or joint transform
- transform-parent basis replacement versus structural ancestor multiplication
- duplicate transform refreshes following selection, cursor placement, or gizmo attachment
- whether topology refreshes caused by armature marker attachment recompute a joint from the wrong
  basis

Relevant code:

- `src/engine/ecs/system/transform_system.rs`
- `src/engine/ecs/component/transform_parent.rs`
- `src/engine/ecs/rx/intent_executor.rs`
- `src/engine/ecs/system/armature_visualization_system.rs`
- `src/engine/ecs/system/gltf_bounds_visualization_system.rs`

## Investigation Plan

1. Record component IDs, labels, local transforms, and cached world matrices for:
   - XR rig marker and its transform ancestors;
   - Bisket root;
   - tail joint;
   - one non-tail mesh transform;
   - their bounds follower roots/local transforms.
2. Capture the same snapshot at four points:
   - after startup settles;
   - immediately before the first click;
   - after the first command flush;
   - after the second command flush.
3. Log every `UpdateTransform` and `UpdateTransformWorld` affecting those IDs, including the
   originating event/intent and final routed recipient.
4. Enable and extend `CAT_DEBUG_EDITOR_SCENE_HIT` plus cursor/editor selection diagnostics to
   record which renderable and transform each click resolves to.
5. Determine whether the title-bar event is captured or also reaches global scene handlers.
6. Fix the earliest incorrect state transition rather than compensating in bounds visualization.
7. Add focused regression tests for the identified transition before removing temporary tracing.

## Required Regression Coverage

- Editor Settings visibility toggles work before any title-bar or panel focus click.
- Clicking the settings title bar does not produce a world-scene selection or cursor placement.
- The first terrain click resolves to terrain, never the XR rig marker.
- First and second clicks do not mutate unrelated XR, avatar root, or joint transforms.
- Enabling armature visualization does not change any joint world transform.
- Bounds markers cannot be selected or raycast.
- Bounds followers update when targets move but never emit mutations targeting the GLTF hierarchy.
- Bisket bounds are correct on their first visible frame and remain aligned after animation and
  secondary motion.
- Armature markers remain selectable and pose their target joints through gizmos.

## Acceptance Criteria

- No editor action requires a preliminary panel-focus click.
- Panel interactions do not leak into scene selection/cursor placement.
- The XR rig is only selected or moved through an explicit valid interaction.
- Repeated clicks leave the avatar and all bounds stable unless the clicked tool intentionally
  edits them.
- Bounds remain independently owned and non-selectable.
- Armature visualization retains its working selectable/gizmo-routing behavior.
