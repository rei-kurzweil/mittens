# Shared editor panels

## Status

Draft design only. This is not implemented yet.

## Problem

We now support multiple `EditorComponent` subtrees, each with independent selection and gizmo state, but panel spawning is still modeled as a per-editor side effect:

- `EditorComponent::spawn_panels = true` causes `SystemWorld::register_editor(...)` to ask `InspectorSystem` to spawn a world panel and inspector panel for that editor root.
- With more than one `ED {}` root, this produces multiple panel pairs even when the user conceptually wants one shared editor UI.
- The current behavior is simple, but it does not scale well to scenes like `examples/vtuber-desktop.mms` where several editors may coexist.

## Goal

Keep editor state local to each `EditorComponent`, while allowing panels to be either:

- shared across multiple editors, or
- private to a single editor subtree.

## Proposed authored API

Add a serialized `shared_panels: bool` field to `EditorComponent`.

- Default: `true`
- MMS: `ED { shared_panels = false }` opts an editor out of panel sharing and requests its own panel pair.

This keeps the common case terse while allowing explicit local editors.

## Semantics

### `shared_panels = true`

The editor contributes to a workspace-level shared panel set.

- World panel: shows all participating editors.
- Inspector panel: follows the active editor/selection within that shared panel set.
- Selection, gizmos, and editor-local settings remain owned by each `EditorComponent`.
- Only the panel presentation layer is shared.

### `shared_panels = false`

The editor behaves like the current implementation.

- Spawn a dedicated world panel and dedicated inspector panel for this editor root.
- These panels reflect only this editor's hierarchy and selection.

## Shared world panel presentation

The first implementation should prefer a simple grouped view over clever UI.

### Option A: grouped sections

Render one shared world panel with repeated sections:

- editor label/header
- subtree rows for that editor

Pros:

- easy to implement using the existing row builder
- no new interaction model required
- multiple editors remain visible at once

Cons:

- tall panel when many editors are present

### Option B: tabs

Render one shared world panel with one active editor at a time.

Pros:

- compact
- closer to desktop editor mental model

Cons:

- requires active-tab state and tab UI controls
- hidden editors are less visible/discoverable

Recommendation: start with grouped sections. Tabs can be added later if panel height becomes a real problem.

## Shared inspector behavior

A shared inspector should reflect one active editor context at a time.

Suggested precedence:

1. the editor most recently interacted with in the shared world panel
2. otherwise the editor that most recently changed selection
3. otherwise the first shared editor in registration order

This avoids merging heterogeneous selections from different editors into one inspector.

## Runtime shape

Treat panels as a separate concept from editors.

- `EditorComponent` continues to own:
  - selection
  - gizmo binding
  - coordinate-space settings
- panel management owns:
  - discovering which editors participate in a shared set
  - creating/destroying shared panel roots
  - choosing which editor drives the shared inspector

A likely implementation path is:

1. keep `EditorComponent` state unchanged
2. add `shared_panels` to the component
3. introduce a small registry in `InspectorSystem` or a dedicated panel coordinator
4. materialize either private panels or one shared panel set per workspace

## Non-goals

- merging selections across editors
- sharing gizmo instances across editor roots
- changing editor-local picking or signal routing
- designing the final tab UX up front

## Open questions

- Should there be exactly one shared panel set per workspace, or one per camera/editor-space cluster?
- How should shared editors be labeled in the world panel when no explicit user-facing name exists?
- Should an editor be allowed to request `shared_panels = true` but suppress panel spawning entirely if no shared host exists?
- Do we eventually want `shared_panels` to become an enum (`shared`, `private`, `none`) instead of a boolean?
