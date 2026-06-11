# Task: Audit Gizmo Space, Parent Rotation, and Painted-Icon Drag Axes

Date: 2026-06-10

Status: terrain-mapping / investigation plan.

## Problem

Current gizmo behavior appears internally inconsistent:

- translation gizmo arms look visually correct
- but dragging a gizmo on certain painted icon instances moves the target along an unexpected,
  orthogonal, or horizontal axis
- translation should be world-space by default
- rotation rings should be local / relative by default
- translation-arm visuals should not inherit parent rotation in a way that changes the meaning of
  drag axes

The suspicion is that gizmo visuals and gizmo drag math are not using the same space assumptions,
especially for targets produced through paint placement or nested authored icon topology.

## Expected behavior

### Translation

- default translation mode: World
- dragging the Y arrow moves the target in world +Y
- parent rotation should not rotate the translation intent

### Rotation

- default rotation mode: Local
- dragging a rotation ring applies relative/local rotation around the target’s local axes

### Visuals

- translation-arm visuals should present the same axes the drag math uses
- if visuals are screen/world compensated, that compensation must not desynchronize drag behavior

## Symptom focus

The strongest repro currently mentioned is:

- paint an icon from `assets/components/icons.mms`
- place a gizmo on that painted icon instance
- drag the Y translation arm
- observed result: motion occurs along a different axis than the gizmo arm implies

That is a high-value repro because painted icons are nested, rotated, and likely include
wrapper transforms created by paint placement or asset instantiation.

## Relevant code

- gizmo drag/space logic
  [src/engine/ecs/system/gizmo_system.rs](../../src/engine/ecs/system/gizmo_system.rs)
- editor defaults for gizmo spaces
  [src/engine/ecs/component/editor.rs](../../src/engine/ecs/component/editor.rs)
- paint placement path
  [src/engine/ecs/system/paint_placement.rs](../../src/engine/ecs/system/paint_placement.rs)
- paint system placement integration
  [src/engine/ecs/system/editor_paint_system.rs](../../src/engine/ecs/system/editor_paint_system.rs)
- icon authoring
  [assets/components/icons.mms](../../assets/components/icons.mms)

## Questions to answer

1. What component does the gizmo actually attach to for painted icon instances?
2. Is that target under a rotated parent or helper wrapper?
3. Are gizmo translation visuals compensating for parent rotation while drag math still uses local
   axes from some ancestor?
4. Is drag math using target local axes, gizmo local axes, or world axes for translation?
5. Is paint placement producing transforms whose apparent “up” differs from world +Y?

## Hypotheses

### Hypothesis A: parent rotation leaks into translation drag basis

The translation gizmo arrows may render in a compensated orientation, while drag projection still
uses an inherited/local axis basis from a rotated parent.

### Hypothesis B: gizmo attaches to the wrong transform

Painted icons may be nested under wrapper transforms. If the gizmo attaches to a wrapper rather
than the intended authored transform, visual axes and movement semantics can diverge.

### Hypothesis C: translation-space default is correct in editor config but not applied uniformly

`EditorComponent` defaults to World translation / Local rotation, but one or more gizmo codepaths
may still consult local/inherited orientation when computing drag deltas.

### Hypothesis D: paint placement rotates authored content in a way gizmo logic does not expect

If painted content is oriented to a surface normal, a local “up” axis may no longer match world
up. Translation in world space should still ignore that, but local-space assumptions would break.

## Investigation phases

### Phase 1: inspect the selected target topology

For the painted-icon repro, log:

- selected component
- gizmo attached parent
- nearest authored transform
- parent chain transform rotations

The first goal is to know exactly which transform is being moved.

### Phase 2: log translation basis

At translation drag time, log:

- translation space mode
- axis requested by gizmo handle (`X`, `Y`, `Z`)
- world axis actually used for projection/movement
- parent/world rotation used in any basis conversion

This should immediately reveal whether Y drag is really using world Y.

### Phase 3: compare visuals vs math

For each translation handle:

- log the rendered axis direction in world space
- log the movement axis direction used by drag math

If they differ, the bug is not “user confusion”; it is a true space mismatch.

### Phase 4: compare plain transform vs painted icon

Run the same logs on:

1. ordinary authored cube
2. painted icon instance

If ordinary cubes behave correctly but painted icons do not, the likely issue is target topology
or paint-placement rotation, not generic gizmo math.

## Desired end-state

After the audit, the system should satisfy:

- translation handles use world-space basis by default
- rotation rings use local-space basis by default
- translation-handle visuals do not inherit parent rotation in a misleading way
- painted icons and ordinary authored objects obey the same drag semantics

## Potential fix directions

### If wrong target is attached

- select nearest authored transform rather than wrapper
- or teach paint placement to mark the intended editable root explicitly

### If translation math is local when it should be world

- harden translation path to always use world basis in World mode
- ignore inherited parent rotation for drag basis in that mode

### If visuals are the only wrong part

- compensate translation-arm visuals so they represent the same world axes as drag math

### If paint placement is the mismatch

- clarify whether painted instances should be editable at placement root or authored-content root
- normalize the editable transform topology

## Repro set

Minimum useful repro matrix:

1. plain cube, Y drag
2. plain cube under rotated parent, Y drag
3. painted icon instance, Y drag
4. painted icon instance under any additional editor wrapper, Y drag

## Related

- [docs/task/editor_selection_and_paint_perf.md](./editor_selection_and_paint_perf.md)
- [docs/task/multi-asset-types-and-texture-painting.md](./multi-asset-types-and-texture-painting.md)
- [docs/spec/editor+general-gizmos.md](../spec/editor+general-gizmos.md)
