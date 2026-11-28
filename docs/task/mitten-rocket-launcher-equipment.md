# NPC Mitten rocket launcher equipment

Allow NPC Mitten to collect, aim, fire and reload the shared STA launcher.
The current NPC weapon slot and armory expect pistol-specific methods, while
the launcher has a different grip, ammunition, reload and network lifecycle.

- Define a weapon interface for availability, pickup, aim pose, firing,
  ammunition and release; retain the existing pistol behavior.
- Integrate launcher pickup and reload with shared ownership so a single
  host controls each NPC and peers see launches and impacts once.
- Give rocket use appropriate range and blast safety rules, including nearby
  players and the firing NPC.
- Validate pickup, firing, ammunition depletion, reload, removal and respawn
  in solo and multiplayer, including player grabs and reconnects.
