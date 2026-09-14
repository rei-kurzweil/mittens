# Desktop workbench

Date: 2026-09-02

This directory is the thinking space and goal dashboard for the next desktop,
editor, rendering, input, and avatar-animation work. It does not replace the
detailed task, spec, analysis, and bug documents elsewhere in `docs/`; each
workstream below links to those implementation records.

## Current priorities

The [interaction priority meta tracker](interaction-priorities.md) records the
latest ordering: investigate the still-failing empty-grid Free Draw startup
first, grid startup visibility second, then the grabbing/poses/release-zones epic. This overrides the relative
priority of those items in the broader workstream list below; it does not make
every tracked item a release blocker.

| # | Workstream | Status | Useful next move |
|---|---|---|---|
| 1 | [Laptop rendering server](rendering-server.md) | Hardware-blocked for full-power validation; protocol design is not blocked | Specify the UDP/MMS contract, recovery snapshot, and two-machine proof |
| 2 | [Editor, grids, rasterized placement, and paint](editor-grid-paint.md) | Ready; active 0.8 release gate | Run the desktop reproduction and reduce the open release-gate checklist |
| 3 | [Adaptive mirror detail](adaptive-mirrors.md) | Ready for measurement/design after baseline correctness | Measure projected mirror coverage and define stable resolution/crop bands |
| 4 | [Keyboard and regular gamepad events](keyboard-and-gamepad-input.md) | Keyboard state exists; authored handlers and non-XR gamepads need a tracker/implementation | Define one normalized event surface, then land keyboard handlers first |
| 5 | [Locomotion and armature animation](locomotion-and-armature-animation.md) | Planned; pose-layer work is the main prerequisite | Prove an interruptible whole-armature pose blend, then drive it from locomotion |
| 6 | [Non-tracked locomotion actions and vehicle behavior assets](../task/non-tracked-locomotion-actions-and-vehicle-behaviors.md) | Design proposal | Keep V1 keyboard/XR car assets separate, then converge them on a normalized `Vec2` action surface |

The numbering records the current list, not a strict execution order. In
particular, workstream 1 can advance through design and loopback tests while a
replacement laptop battery is unavailable, and workstreams 2–5 do not depend
on that battery.

## Shared decisions

- Keep device acquisition separate from semantic engine events. Keyboard,
  desktop gamepad, XR controllers, and remote UDP input may share normalized
  event/intent concepts without pretending to be the same device.
- Keep authored scene state independent from presentation. The rendering
  server should consume a versioned scene snapshot plus ordered changes; it
  should not become the authority for VR input or simulation.
- Optimize only after adding observability. Mirror adaptation and rendering
  offload both need counters/timestamps that show what work was actually
  avoided.
- Treat live tracked bones as explicit animation owners. Whole-armature blends
  need masks/priorities so locomotion never silently overwrites the head,
  eyes, or tracked hands.

## When priorities change

Update the table first, then update the status and first unchecked milestone in
the corresponding workstream page. Detailed implementation findings belong in
the linked canonical task or bug tracker rather than being copied here.
