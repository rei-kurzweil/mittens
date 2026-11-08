# Meow Meow Script documentation

User-facing MMS documentation lives with the crate.

## Guide

- [MMS language guide](guide/language.md)

## How to

- [Configure a script host](how_to/configuring_a_script_host.md)
- [Use the standard host](how_to/use_the_standard_host.md)

## Drafts

- [Implicit render graphs and post-processing composition](draft/implicit-render-graph-and-post-processing.md)
- [RuntimeSpec host binding model](draft/runtime-spec-host-binding-model.md)
- [MMS type-system epic](draft/type-system-epic.md)
- [Record type expressions and record values](draft/record-types-and-values.md)

## Working specifications

- [Host-filtered bespoke MMS event handlers](spec/host_filtered_bespoke_mms_event_handlers.mms) — typed event selectors and native filtering; proposal only.

Most design analysis, active implementation tasks, and Mittens-specific
runtime-boundary documentation remain in the workspace documentation while
the crate ownership cutover is active. Their migration is tracked in
[Move Meow Meow documentation into its crate](../../../docs/task/move-meow-meow-documentation-into-crate.md).
