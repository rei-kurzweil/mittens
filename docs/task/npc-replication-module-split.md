# NPC replication module split

Status: proposed 2026-11-27 before extending the oversized
`assets/components/network/npc_replication.mms` (603 lines).

## Why split

The module currently owns several distinct replication responsibilities:

- Reliable ordered discrete NPC actions and acknowledgements.
- Host authoritative pose, health, shield, and peer damage reporting.
- Remote puppet construction and pose application.
- Target selection and replay of NPC weapon effects.

Keep the public API stable while moving the action window and shot effect
helpers into cohesive companion modules. `shared_npc` should remain the
component-family facade unless later changes show another natural boundary.

## Constraints

- Preserve batched latest-value pose transport and the current bounded action
  window; do not add per-frame allocations or extra channels per NPC event.
- Keep pose sends at the existing 20 Hz while spawned and 1 Hz while idle.
- Preserve event ordering, acknowledgements, late-pose handling, and damage
  counters across packet loss.
- Avoid creating duplicate render resources or weapon effects on each update.

## Validation

- Keep the existing public `shared_npc` and `npc_target_switch` call shapes
  working during the split.
- Validate host/client spawn, remove, movement, shots, down/respawn, and damage
  reporting across packet loss and reconnects.
- Compare per-frame allocations, channel count, and pose update cadence before
  and after the split; ensure idle links remain suspended.
