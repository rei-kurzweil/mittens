# Editor context issues

Date: 2026-06-05

Status: active investigation.

## Progress so far

The editor-context refactor is materially better than the previous state:

- the shared editor context now owns active editor, selected component, and focused panel
- paint no longer duplicates shared UI reduction per editor root
- world-panel editor roots are selectable rows
- ordinary scene selection no longer rebuilds the full world panel on every editor selection change

Observed improvement:

- the entire UI no longer locks up for minutes after the editor/panel interactions that were
  previously triggering worst-case stalls
- menu interaction and `SelectionComponent`-driven panel selection appear responsive

## Remaining problem

There is still a shorter lockup or stall after attempting to paint.

Current user-observed behavior:

- selecting assets from the Assets panel is responsive
- panel menus and panel selections do not appear to lock up
- after clicking to paint something, the app can still stall for a few seconds
- it is not yet clear whether this is:
  - paint-specific work
  - editor selection/input routing
  - a scene hit on the wrong transform/wrapper
  - some follow-on recomputation after the click/drag path

## Relevant trace shape

The recent trace suggests:

- Assets selection correctly focuses `assets_root`
- paint status correctly stays inactive when Paint is not focused
- scene `DragStart` / `DragMove` still reach the editor-facing paint path
- editor selection can still resolve to `editor_auto_raycastable` wrappers during scene interaction

That means the remaining issue may be one of:

1. input routing is still doing too much work on scene interaction even when paint is inactive
2. editor selection is landing on wrapper transforms instead of the intended authored target
3. a scene click/drag is triggering expensive downstream updates unrelated to panel selection

## Next diagnostic step

Before changing more editor routing, build a clearer scene-level repro inside `bisket-vr-demo`.

The goal is to make scene interaction visually obvious so we can tell whether the remaining stall
is tied to:

- the clicked object / selection target
- parent-vs-child transform routing
- paint placement attempt vs ordinary editor selection

## Planned `bisket-vr-demo` repro scene change

Add a small authored transform hierarchy with three cubes:

- three cube children arranged into a triangle using child transforms
- each cube rotates individually so their identities are easy to track visually
- the parent transform of those cube transforms rotates around the x-axis

Required structure:

```text
rotating_parent
  cube_a_transform
    cube_a
  cube_b_transform
    cube_b
  cube_c_transform
    cube_c
```

Required behavior:

- `cube_a_transform`, `cube_b_transform`, and `cube_c_transform` place the cubes into a triangle
- each cube has its own animation/rotation
- `rotating_parent` rotates around the x-axis

## Why this repro is useful

This should let us answer:

- when clicking near one cube, which transform actually becomes selected?
- does the stall happen only when paint is attempted on these animated objects?
- does the stall depend on hitting a moving child vs a rotating parent-owned surface?
- are we still accidentally selecting helper wrappers such as `editor_auto_raycastable`?

The cubes should be animated with `Animation` and `Keyframe` components specifically so the repro
can distinguish camera/input lockup from a full app/render stall:

- if the camera appears stuck but the cubes keep animating, the app/render loop is still running
  and the issue is more likely camera or input routing related
- if the cubes also stop animating, the stall is affecting the wider app/frame update path rather
  than only camera control

## Likely follow-up checks after the repro exists

- log the resolved editor selection target for each scene click/drag
- log whether the chosen target is an authored transform or an auto-generated wrapper
- early-out editor-paint stroke handling even sooner when Paint is not focused
- consider teaching `EditorSystem` to skip selecting `editor_auto_raycastable` wrappers and prefer
  the wrapped authored transform

## Immediate next action

Implement the `bisket-vr-demo` animated triangle-of-cubes repro before continuing deeper changes
to editor input routing or paint activation behavior.
