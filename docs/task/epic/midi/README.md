# Epic: MIDI devices, events, and file authoring

Status: planning, 2026-10-07. Documentation only; no MIDI components, parser,
or dependencies have been added. The basic rhythm example and MMS gameplay
library are the immediate implementation priority.

## Goal and boundaries

Provide general MIDI support for Mittens: live `MidiInput` and `MidiOutput`
components, an observable `MidiEvent` payload, and reading Standard MIDI Files
from disk. A first consumer is a rhythm course partly authored in a DAW and
exported as `.mid`. Other consumers include controllers, instrument routing,
and beat-authored scene animation.

Keep device I/O, file parsing, and game interpretation separate. Loading a
file produces timed musical data; it does not open a device or play audio.
The rhythm library maps that data to its course actions and constructs the
course animation, optionally through intermediate records. Initially it imports
a hand-authored animation factory from `lib/example_course.mms`; MIDI can later
supply an equivalent factory or keyframes with the same game bindings.
MIDI events alone do not define body positions, target sizes,
scoring rules, or rendered music; those require an explicit game mapping and
a separate music asset or sound source.

## Repository findings

The current repository has `MusicNote`, `AudioClip`, beat clocks, animations,
and the event/intent signal machinery. The MIDI reference in `music_system.rs`
is a pitch-to-frequency calculation, not device or file support. There are no
`MidiInput`/`MidiOutput` components or MIDI parser dependencies in `Cargo.toml`.

MMS currently parses untyped function parameters. The requested typed callback
below is the intended API shape, dependent on the separate
[typed declarations/functions draft](../../../../crates/meow-meow-script/docs/draft/typed-declarations-and-functions.md).
General MIDI support should be able to expose the same payload to an untyped
`fn(event)` callback before annotations are implemented.

## A. Live MIDI input and output

Proposed usage, not implemented syntax:

```mms
on(some_midi_input, "MidiEvent", fn(event: MidiEvent) {
    // Inspect the message and route it to scene or musical behavior.
})
```

`MidiInput` owns a selected input connection. `MidiOutput` owns a selected
output connection. Settle constructors and output send methods in a focused
task; names are agreed here, exact signatures are still open. Output sends
should enter the existing intent/execution path with a declared immediate or
scheduled timing policy. MIDI input uses normal scoped event delivery.

The first device proof needs:

- Port enumeration and an explicit selector; distinguish an absent port from
  an ambiguous name. Determine which backend IDs remain useful across runs.
- Connection status, errors, device loss, reconnection policy, and cleanup
  when a component/session is removed. Missing hardware should leave a useful
  diagnostic rather than prevent unrelated scene work.
- A callback-to-engine queue. Device callbacks capture bytes/timestamps;
  script handlers run on the engine's callback path, not a device thread.
- Ordered note-on/note-off and controller delivery. Declare queue bounds and
  overflow diagnostics; silently coalescing note edges can leave notes stuck.
- An output teardown policy for outstanding notes, plus a loopback proof that
  sends messages through `MidiOutput` and observes them through `MidiInput`.

Define the `MidiEvent` payload contract before wiring MMS conversion. Candidate
fields are source identity, message kind, channel when applicable, decoded
message fields, raw bytes, original device timestamp, and engine receipt time.
Document timestamp units/origin and any conversion to the engine clock. Choose
and document channel numbering; preserve actual values without mixing wire
numbers and DAW display numbers. Note-on with zero velocity needs a documented
note-off interpretation while retaining the raw message.

Start with MIDI 1.0 channel messages, including notes and control changes.
Inventory pitch bend, program changes, pressure, SysEx, and system messages;
state what is supported, ignored, or reported before claiming general support.
MIDI Clock/start/stop synchronization is a later transport task. A MIDI event
timestamp is not automatically an audio sample time or a course beat epoch.
MIDI 2.0/UMP is a separate follow-up.

## B. Reading MIDI files from disk

Expose a general disk-loading API to MMS that yields inspected file metadata
and ordered timed events. Decide whether this is a resource/builtin or a file
component in its own task; `MidiInput` is the live-device component.

Begin with Standard MIDI File formats 0 and 1 with metrical timing. Accumulate
delta ticks separately for each track, then derive absolute quarter-note beats
using the file's ticks-per-quarter-note division. Preserve track identity/name,
channel, event order, tempo map, time signatures, and note data. Define stable
ordering for events at the same tick across tracks. Report malformed files and
explicitly unsupported timing/format variants. Format 2 and SMPTE/timecode
division can follow; do not treat them as one metrical course accidentally.

Tempo affects beat-to-seconds conversion; a time signature supplies musical
grouping. Neither should silently overwrite the scene's clock. For the first
rhythm import, use a fixed-BPM course and verify the file tempo against the
declared BPM, or require an explicit override. Reject unsupported tempo changes
with a useful diagnostic until shared audio/visual transport supports them.

Pair note-on/off where duration is needed, with a declared policy for overlapping
same-pitch notes, missing note-offs, and velocity-zero note-ons. Preserve unknown
or unmapped events for inspection or report them. A loader should not discard
data merely because the rhythm game does not use it.

## C. DAW export to a rhythm course

The proposed authoring flow is:

1. Author cue timing in DAW MIDI tracks aligned to the music's beat grid.
2. Export `.mid` and the corresponding music asset with a known common origin.
3. Load the MIDI file and apply an explicit game mapping.
4. Produce keyframes using the same game actions as `lib/example_course.mms`,
   directly or through intermediate course records.
5. Let `examples/rhythm_game/lib/rhythm_game.mms` assemble and bind the finite
   animation, own the session epoch and debug controls, and coordinate targets,
   coach poses, and winding/fading pictograms.

A candidate mapping uses named tracks for head/left/right roles and note
numbers for entries in an MMS pose/placement library. This is an option to
prove, not an encoding chosen by this document. Decide how chords create
simultaneous sets and what note duration/velocity mean. Do not silently equate
duration with travel time or velocity with world position. Retain unmapped-event
diagnostics and source track/tick identity for authoring feedback.

Imported note onset normally becomes a due beat. Travel lead, final one-beat
fill, feedback hold, and expiry are calculated by the game library. Imported
cue beats use the same course zero as hand-authored cues; countdown/pre-roll
must not shift the exported song's course data. Include explicit DAW export
instructions covering tempo, track naming, song origin, and pickup/lead-in.

## Delivery order and acceptance

1. [ ] Build the basic visual rhythm example and MMS gameplay library with an
   imported `example_course.mms` animation; prove overlapping cues, coach and
   pictogram poses, winding/fading previews, pause/resume, and Reset cleanup.
2. [ ] Settle the shared MIDI message/event vocabulary and disk-loader API.
3. [ ] Implement disk loading and inspection with small format 0/1 fixtures;
   verify beats, tempo metadata, chords, ordering, and unsupported-file errors.
4. [ ] Prove a small DAW export maps to the same game actions and visible
   schedule as the hand-authored animation through the same library bindings.
5. [ ] Implement `MidiInput`, scoped `MidiEvent` conversion, and diagnostics;
   verify ordering, timestamp semantics, disconnects, and component removal.
6. [ ] Implement `MidiOutput` and a loopback example; verify delivery and note
   cleanup, then specify scheduled output against the engine clock.
7. [ ] Add typed callback support when the MMS type-system work permits it.
8. [ ] Address variable tempo, external MIDI transport sync, virtual ports,
   additional messages/platforms, and recording/file writing as focused tasks.

File import and device I/O can be developed independently after their common
message vocabulary is agreed. Neither live hardware nor typed parameters are
prerequisites for the first file-authored course.

## Candidate dependencies and research

These are implementation candidates, not selected or installed dependencies:

- [midir](https://github.com/Boddlnagg/midir) provides cross-platform live port
  I/O. Review supported backends and virtual-port differences for Mittens targets.
  Its [input API](https://docs.rs/midir/latest/midir/struct.MidiInput.html)
  supplies bytes and a microsecond timestamp with a connection-local, unspecified
  origin; engine-clock alignment requires an explicit policy.
- [midly track events](https://docs.rs/midly/latest/midly/struct.TrackEvent.html)
  expose per-track delta ticks. Its
  [timing](https://docs.rs/midly/latest/midly/enum.Timing.html) and
  [format](https://docs.rs/midly/latest/midly/enum.Format.html) models distinguish
  metrical/timecode division and single/parallel/sequential tracks. Review
  ownership of parsed bytes before exposing loaded data across MMS sessions.

## Related plans

- [Rhythm prototype epic](../vr-rhythm-game-prototype.md)
- [Prototype slices and library boundary](../../rhythm-game-prototype-slices.md)
- [Stage choreography](../../rhythm-game-stage-choreography.md)
- [Course timing and judging](../../rhythm-game-course-timing-and-judging.md)
- [Signals v2](../signals_v2/README.md)
