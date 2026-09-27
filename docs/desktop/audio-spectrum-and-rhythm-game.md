# Workbench: audio spectrum debugging and VR rhythm game prototype

Status: planning. This is a loose dashboard for two related epics, not their
implementation specification. The prototype may expose needs in Mittens while
the audio diagnostic work helps inspect microphone analysis independently.

| Workstream | Current scope | Next review point |
|---|---|---|
| [Audio spectrum debugging](../task/epic/audio-input-spectrum-debugging.md) | Optional pre/post-filter FFT taps for microphone RMS and a reusable spectrum view | Review the small analysis-plan boundary and bucket handoff before implementation |
| [VR rhythm game prototype](../task/epic/vr-rhythm-game-prototype.md) | Beat-authored targets, guides, coach, and scenery in a playable MMS example | Review the stage contract and first slices before building `minimal.mms` |

## How these relate

Both are useful development work in this repository, but neither is a
prerequisite for the other. The rhythm prototype co-develops Mittens through
concrete game needs. The spectrum view is a diagnostic tool for microphone
processing and may later help inspect audio used by a game; it does not define
rhythm-game timing or scoring.

The rhythm prototype stays public and deliberately generic through its proof
of concept. Expect at least one and likely two environments here. When the
prototype begins to resemble the game being built with the collaborator, plan
a private-repository move. Keep game-specific content and reusable engine
changes separable so that move can happen without copying Mittens internals.

## Review order

1. Review the rhythm [stage choreography contract](../task/rhythm-game-stage-choreography.md)
   and [course timing and judging](../task/rhythm-game-course-timing-and-judging.md).
2. Adjust the [delivery slices](../task/rhythm-game-prototype-slices.md) and
   choose the first environment and music asset.
3. Build the minimal example and let its needs drive small Mittens changes.

Keep arbitrary cross-epic notes here; keep detailed decisions and acceptance
criteria in the linked task documents.
