# Draft: `Selection` component

## Status

Draft only.

This note describes the current `SelectionComponent` briefly, then focuses on
the signal API that should surround it.

It does not claim that all of the signals described here already exist in the
runtime.

---

## 1. Purpose

`Selection` is a reusable UI selection scope.

Its job is to:

- define a local selection boundary
- own the selected option state for that boundary
- support either single-select or multi-select behavior
- let other systems react to selection changes without reading component state
  opportunistically

Typical uses:

- asset browser item selection
- paint tool selection
- world panel row selection
- panel-shell selection in a larger editor layout

`Selection` works together with `Option`:

- `Selection` defines the scope
- `Option` defines a selectable unit inside that scope

The intended hit-resolution rule is:

`renderable -> nearest Option -> nearest enclosing Selection`

Nested `Selection` scopes are barriers. An outer scope must not steal a hit that
belongs to an inner scope.

---

## 2. Component state

Current runtime state is roughly:

```rust
pub struct SelectionComponent {
    pub mode: SelectionMode,              // Single | Multiple
    pub selected_index: Option<usize>,
    pub selected_item: Option<String>,
    pub selected_component: Option<ComponentId>,
    pub selected_entries: Vec<SelectionEntry>,
}
```

Mode:

- `Selection {}` means single-select
- `Selection("multiple")` means multi-select

Semantics:

- in single-select mode, there is at most one selected entry
- in multi-select mode, `selected_entries` is the real selection set
- in multi-select mode, `selected_component` is only the current primary entry,
  not the full set

This distinction matters because single-select and multi-select do not mean the
same thing operationally, even if they share one component type.

---

## 3. Current runtime behavior

Today, `Selection` is mostly driven by `Click`:

- a `Click` event lands on a renderable
- `SelectionSystem` resolves the nearest `Option`
- it finds the nearest enclosing `Selection`
- it mutates that `SelectionComponent`
- it updates selection presentation/highlight

Important limitation:

- `SelectionSystem` currently mutates component state directly
- it does not emit a dedicated `Selection`-scoped change event
- there is no built-in `IntentValue` for "set this selection to that option"

So the current API shape is effectively:

- input: `Click`
- output: mutated `SelectionComponent` state

That is enough for local behavior, but weak for orchestration. Systems that care
about selection changes should not have to poll `selected_component` or diff
`selected_entries` by hand.

---

## 4. Why `Selection` needs signals

Signals give `Selection` a proper boundary between:

- gesture interpretation
- selection state mutation
- downstream reaction

Without a dedicated signal contract, downstream systems end up coupled to:

- `Click` directly
- internal selection storage details
- ad hoc tree inspection and state diffing

That is the wrong layer.

The purpose of `Selection` signals is to let other systems subscribe to
selection semantics, not to pointer mechanics.

Examples:

- a paint system should react to "tool selection changed", not to raw `Click`
- an asset preview system should react to "asset selection changed", not inspect
  panel-local state after every tick
- a panel system should be able to set selection programmatically without
  fabricating a fake pointer click

---

## 5. Signal roles

`Selection` needs two signal roles:

1. input signals
2. output signals

### 5.1 Input signals

These tell the selection scope to change state.

They are not facts. They are requests.

Recommended shape:

```rust
IntentValue::SelectionSet {
    selection_root: ComponentId,
    entries: Vec<SelectionEntry>,
    primary: Option<ComponentId>,
}
```

Purpose:

- set selection programmatically
- drive selection from keyboard navigation, scripts, or system logic
- avoid synthesizing fake `Click` events just to reuse selection behavior

For multi-select ergonomics, a delta-style family may also be useful:

- `SelectionAdd`
- `SelectionRemove`
- `SelectionToggle`
- `SelectionClear`

But the minimum useful input API is one direct "set selection" intent.

### 5.2 Output signals

These tell the rest of the engine that selection state changed.

They are facts, not requests.

Recommended minimum shape:

```rust
EventSignal::SelectionChanged {
    selection_root: ComponentId,
    mode: SelectionMode,
    selected_entries: Vec<SelectionEntry>,
    selected_component: Option<ComponentId>,
}
```

Purpose:

- let other systems observe the semantic result of selection
- keep consumers decoupled from `Click`
- give one canonical event for both single-select and multi-select scopes

This event should be scoped to the `Selection` component that changed.

That means consumers can subscribe to a specific selection scope instead of
filtering a global stream.

---

## 6. Single-select vs multi-select signals

Single-select and multi-select should not require different event kinds.

They do require different semantics, but that difference can live in payload:

- `mode`
- `selected_entries`
- `selected_component`

Why one event is preferable:

- one subscription model for all selection scopes
- no branching between "single selection changed" and "multi selection changed"
- multi-select consumers still receive the complete set
- single-select consumers can just read `selected_component`

So the recommendation is:

- keep one canonical output event: `SelectionChanged`
- include enough payload to represent both modes cleanly

If a future system truly needs deltas rather than snapshots, add secondary
events later:

- `SelectionAdded`
- `SelectionRemoved`
- `SelectionCleared`

Those should be additive, not replacements for the canonical snapshot event.

---

## 7. Relationship to existing `SelectionChanged`

The runtime already has an `EventSignal::SelectionChanged`, but today it is used
by `EditorSystem` for editor scene selection.

That is conceptually close, but it is not yet a general `SelectionComponent`
contract.

The long-term direction should be:

- `SelectionChanged` means "a selection scope changed"
- editor scene selection can use that same semantic event if its selection model
  is expressed through a real selection scope
- UI selection and scene/editor selection should not diverge into unrelated
  signal vocabularies unless their semantics are truly different

---

## 8. Recommended runtime contract

For a `Selection` scope, the recommended contract is:

1. raw gesture systems emit `Click`
2. `SelectionSystem` interprets `Click` into option selection
3. `SelectionSystem` mutates `SelectionComponent`
4. `SelectionSystem` emits `SelectionChanged` on that selection scope
5. downstream systems observe `SelectionChanged`
6. non-pointer systems can change selection through `SelectionSet`

This keeps responsibilities clean:

- pointer systems own pointer facts
- selection owns selection semantics
- feature systems consume semantic selection events

---

## 9. Non-goals

- making `Selection` itself responsible for content creation
- encoding pointer gesture details into selection events
- requiring separate event kinds for single vs multi-select by default
- forcing consumers to inspect internal component fields when a semantic event
  should exist
