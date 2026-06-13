# Selection root target subtree and direct option payloads

Date: 2026-06-12

Status: planned task / follow-up to editor slot projection work

## Goal

Finish the selection model cleanup by making two changes together:

1. remove `Selection.payload_selector(...)`
2. let a `Selection` control node target an adjacent authored subtree instead of
   wrapping that subtree

The intended end state is:

- selection payloads come only from `Option -> Data`
- selection roots do not need to be layout wrappers
- authored panel shells can keep stable selection control nodes without
  collapsing layout or forcing payload queries

This task is the missing structural piece behind:

- [option-direct-data-payload-refactor.md](/home/rei/_/cat-engine/docs/task/option-direct-data-payload-refactor.md:1)
- [editor-slot-projection-and-mount-points.md](/home/rei/_/cat-engine/docs/task/editor-slot-projection-and-mount-points.md:1)

## Problem recap

The recent world-panel regression exposed a real structural issue.

We want selection roots to be stable authored control nodes owned by the shell.
But if the only way to scope a selection root is:

```text
Selection {
  content subtree
}
```

then `Selection` becomes a topology wrapper.

That is bad for layout-owned UI because:

- `Selection` is a control node, not a layout primitive
- wrapping the content mount under `Selection` changes the layout parent chain
- block sizing / scrolling / overflow now depend on an unstyled wrapper
- the content area can collapse even when the inner styled node is correct

That is exactly what happened with the world panel:

- `content_slot` used to be the styled block participant
- after moving it under `world_panel_selection`, the outer wrapper had no layout
  sizing contract
- status/content flow broke

So removing `payload_selector` alone is not enough.
We also need a way for `Selection` to point at a subtree it governs without
becoming that subtree's layout wrapper.

## Desired authored shape

We want this kind of structure to be valid:

```text
world_panel_content_area
  world_panel_selection   // control node
  content_slot            // layout/content mount
    item_0
      Option
        Data
    item_1
      Option
        Data
```

Instead of:

```text
world_panel_selection
  content_slot
    item_0
      Option
        Data
```

The same applies to inspector sidebar, asset lists, settings lists, and other
selection-driven UI surfaces.

## Proposed contract

## 1. Payload ownership

Keep the payload rule from the existing refactor note:

- the selected option owns its semantic payload
- that payload is a direct child `DataComponent`
- runtime resolves `selected_payload` from the selected option root only

Target row shape:

```mms
T {
    name = "item_0"

    Option {
        Data {
            row_kind = "WorldItem"
            label = "renderer_settings"
        }
    }

    // visual subtree
}
```

No `payload_selector`.
No descendant query.

## 2. Selection scope

A selection root should explicitly target a selection surface subtree.

Conceptually:

```mms
Selection.root("#rows_mount") {
    name = "world_panel_selection"
}

T {
    name = "rows_mount"
    ...
}
```

The exact syntax can change.
What matters is the contract:

- a selection root may govern an adjacent subtree
- that subtree is the scope for option discovery and selection highlighting
- the selection root itself does not need to be the parent of the selectable
  rows

## 3. Entry discovery

Within the targeted subtree:

- selection hit resolution still starts from clicked/rendered row topology
- runtime resolves the nearest `Option` root for the clicked entry
- that option root must be inside the targeted subtree
- `selected_payload` resolves from the selected option root's direct `Data`
  child

This keeps topology ownership explicit without reintroducing query strings.

## Why this is better

- selection no longer depends on selector-based payload lookup
- selection no longer forces layout wrappers around scroll/content boxes
- panel shells can author stable control nodes and stable mount points
- runtime can resolve and cache selection controls independently from content
  mounts
- the model scales to world panel, inspector sidebar, assets, settings, and
  future panel/list surfaces

## Runtime implications

## 1. `SelectionComponent` needs target-root semantics

Today it stores:

- `selected_component`
- `selected_payload`
- `selected_index`
- `payload_selector`

The target shape should be:

- keep selected state
- remove `payload_selector`
- add explicit target-root semantics for adjacent subtree scoping

Possible shapes:

- selector string first: `target_root_selector: Option<String>`
- resolved component ref later: `target_root: Option<ComponentRef>`
- or a split between authored selector and runtime-resolved cached id

The long-term preferred direction is resolved component identity, not freeform
selector text at runtime.

## 2. Selection system should stop mixing two queries

Today selection behavior is split across:

- hit/option resolution
- payload-selector descendant queries

The next version should instead use:

1. resolve selection scope root
2. ensure selected option belongs to that scope
3. resolve direct child `Data`

That is a much narrower contract.

## 3. Mounted-view runtime objects should cache selection controls

This follows the slot-projection task directly.

For example:

- world panel resolves:
  - `Selection` control node
  - `Content` mount
- inspector sidebar resolves:
  - `Selection` control node
  - `Sidebar` mount

The runtime should not keep re-finding those nodes after every rerender.

## Authoring implications

## 1. Panels should stop wrapping layout mounts in `Selection`

Use:

- styled/scrolled content area as the layout participant
- `Selection` as a sibling control node inside that area
- mount/content subtree as a sibling target

Do not use:

- `Selection { content_slot ... }`

for layout-owned panel content.

## 2. Row/item factories should standardize on `Option -> Data`

World rows, inspector rows, asset rows, paint rows, settings rows should follow
one rule:

- `Option`
- direct child `Data`
- then visual subtree

That gives one uniform semantic payload model across the editor UI.

## Implementation checklist

### Phase 1. Write down the selection-target contract

- [ ] Decide the authored API shape for adjacent subtree targeting
- [ ] Decide whether the first implementation uses selector strings or resolved
      component refs internally
- [ ] Document exact scope semantics:
  - [ ] what counts as “inside the targeted subtree”
  - [ ] how nearest `Option` resolution interacts with the target root
  - [ ] what happens if no target root is resolved

### Phase 2. Update `SelectionComponent`

- [ ] Remove `payload_selector` from
      [`src/engine/ecs/component/selection.rs`](/home/rei/_/cat-engine/src/engine/ecs/component/selection.rs:1)
- [ ] Add target-root configuration fields for adjacent subtree scoping
- [ ] Update CE serialization/deserialization for the new selection-root API

### Phase 3. Update selection runtime semantics

- [ ] Replace payload-selector descendant lookup in
      [`src/engine/ecs/system/selection_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/selection_system.rs:1)
      with:
  - [ ] target-root resolution
  - [ ] option-in-scope validation
  - [ ] direct child `Data` payload resolution
- [ ] Remove runtime support for query-driven payload selection
- [ ] Add debug warnings for:
  - [ ] invalid target-root configuration
  - [ ] selected option outside targeted subtree
  - [ ] zero or multiple direct child `Data` payloads

### Phase 4. Update MMS authoring/runtime registry

- [ ] Remove `Selection.payload_selector(...)`
- [ ] Add the new target-root authoring API
- [ ] Remove any registry/parser support for `payload_selector = ...` on
      `Selection`

### Phase 5. Convert editor panel shells

- [ ] World panel:
  - [ ] keep styled content area as layout node
  - [ ] keep `world_panel_selection` as a sibling control node
  - [ ] target the content subtree explicitly
- [ ] Inspector sidebar:
  - [ ] same sibling-control-node pattern
  - [ ] no layout wrapper `Selection`
- [ ] Settings / paint / assets:
  - [ ] convert authored selection roots away from wrapper-only assumptions

### Phase 6. Convert row/item payload topology

- [ ] World panel rows use direct `Option -> Data`
- [ ] Inspector rows use direct `Option -> Data`
- [ ] Asset rows use direct `Option -> Data`
- [ ] Paint tool rows use direct `Option -> Data`
- [ ] Settings options use direct `Option -> Data`
- [ ] Remove helper code that still queries named payload descendants

### Phase 7. Rewire mounted runtime instances

- [ ] Make mounted panel/view instances cache:
  - [ ] content mounts
  - [ ] selection control nodes
  - [ ] any needed target-root control ids
- [ ] Stop re-finding selection roots on every refresh path

### Phase 8. Remove temporary stopgap logic

- [ ] Delete panel-side code that assigns `selection_component.payload_selector`
- [ ] Delete helper functions that resolve payloads by descendant name query
- [ ] Delete compatibility code added only for wrapper-based selection topology

### Phase 9. Lock behavior with tests

- [ ] Selection system tests:
  - [ ] selected payload resolves from direct child `Data`
  - [ ] nested `Data` does not count
  - [ ] multiple direct child `Data` resolves to `None`
  - [ ] option outside target subtree does not count
  - [ ] sibling-target-root selection works without wrapper parenting
- [ ] Editor panel tests:
  - [ ] world panel selection still syncs editor context
  - [ ] inspector sidebar selection still syncs inspected row
  - [ ] layout does not collapse when selection root is sibling to content mount

## Recommended implementation order

1. Finalize the adjacent-target selection API.
2. Change `SelectionComponent` and `selection_system`.
3. Update MMS registry support.
4. Convert world panel first as the proof case.
5. Convert inspector/sidebar next.
6. Convert remaining editor lists.
7. Remove compatibility/query code.

## Acceptance criteria

- [ ] `Selection.payload_selector(...)` no longer exists
- [ ] selection payloads resolve only from direct child `Option -> Data`
- [ ] selection roots can govern adjacent authored subtrees without wrapping
      them
- [ ] world panel and inspector sidebar keep stable authored selection control
      nodes without breaking layout
- [ ] runtime no longer depends on descendant payload queries by selector string

## Related

- [option-direct-data-payload-refactor.md](/home/rei/_/cat-engine/docs/task/option-direct-data-payload-refactor.md:1)
- [editor-slot-projection-and-mount-points.md](/home/rei/_/cat-engine/docs/task/editor-slot-projection-and-mount-points.md:1)
- [selection-entry-payload-refactor.md](/home/rei/_/cat-engine/docs/task/selection-entry-payload-refactor.md:1)
