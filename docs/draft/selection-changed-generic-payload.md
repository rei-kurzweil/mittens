# Draft: Generic `SelectionChanged` Payloads

## Status

Draft only.

This note proposes a generic way for `SelectionChanged` to carry useful
selection meaning without forcing downstream systems to rediscover that meaning
by walking arbitrary option subtrees.

It is intended as a follow-on to
[`docs/draft/selection-component.md`](./selection-component.md).

---

## 1. Problem

Today a `SelectionChanged` event tells consumers mostly:

- which `Selection` scope changed
- which option/component became selected
- the current selected entries
- an optional display string (`item`)

That is enough for generic selection state, but not enough for systems that
need the semantic object behind the selected option.

Example: asset painting.

- the selected UI component is the `asset_item` shell
- the paint system does not want the shell
- it wants the asset identity to instantiate, and sometimes the preview subtree
  or another nested component under that shell

Today, consumers recover that meaning by:

- matching on `selection_root`
- traversing inside the selected `Option`
- reading text labels
- inferring which nested node was "the real value"

That is fragile because it leaks option-internal topology across the
`Selection` boundary.

---

## 2. Design goal

`SelectionChanged` should stay generic, but it should be able to carry enough
information that downstream systems do not need panel-specific tree walks.

The event should answer two different questions cleanly:

- what UI option was selected?
- what semantic value does that option represent?

These are related, but they are not the same thing.

---

## 3. Non-goal: panel-specific event enums

The event should not become a panel-specific sum type like:

- `AssetSelectionChanged`
- `PaintToolSelectionChanged`
- `PanelSelectionChanged`

Those are useful reducer-internal interpretations, but they are the wrong
runtime abstraction.

The runtime selection event should stay generic.

---

## 4. Core idea

Keep the existing generic selection facts and add an optional generic payload
reference.

Conceptually:

```rust
EventSignal::SelectionChanged {
    selection_root: ComponentId,
    mode: SelectionMode,
    selected_entries: Vec<SelectionEntry>,
    selected_component: Option<ComponentId>,

    // optional generic semantic payload
    selected_title: Option<String>,
    selected_payload: Option<ComponentId>,
}
```

Meaning:

- `selected_component`: the selected option owner / UI shell
- `selected_title`: optional display-facing label
- `selected_payload`: optional component id representing the semantic value of
  that option

The event remains generic because it carries only ids and optional display
metadata, not domain-specific enum variants.

---

## 5. Why a payload component id is different from `selected_component`

The selected option component is often just a wrapper:

- asset row shell
- paint tool tile shell
- world panel row shell

The thing that actually matters may be nested beneath it:

- a preview root
- a payload marker node
- a referenced object
- a subtree chosen as "the semantic output" of that option

So `selected_component` and `selected_payload` should be allowed to differ.

If they happen to be the same for a simple option, that is fine.

---

## 6. Why title is useful but insufficient

An optional title/string is still useful:

- status text
- logs
- debugging
- simple menus where the label is the value

But it should not be the only payload channel.

Titles are presentation:

- they can change
- they may not be unique
- they may be generated
- they may not identify the actual object needed downstream

So the correct ordering is:

- stable component/id payload first
- optional display title second

---

## 7. Where should the payload come from?

The payload should be authored at the `Option` boundary, not rediscovered by
outside consumers.

This keeps nested topology opaque from outside the selection scope.

There are several possible ways to do that.

### Option A: `OptionValue` child component

Introduce an explicit component under an option that points at or wraps the
semantic value.

Example shape:

```text
Option(asset row)
├── preview shell
├── label
└── OptionValue
    └── payload root / referenced node / semantic subtree
```

Possible runtime meaning:

- nearest `OptionValue` under the selected option defines `selected_payload`
- its own component id may be the payload id
- or it may point to another child / target component

Pros:

- explicit and inspectable
- no panel-specific code in consumers
- topology intent is authored once, near the option
- works for simple and complex options

Cons:

- adds one more authored concept/component
- requires deciding whether payload means "this node" or "a child under this
  node"

### Option B: payload selector/path on `Selection` or `Option`

Allow the selection scope or each option to declare how payload should be
resolved.

Examples:

- `Selection.payload_selector("[name='option_value']")`
- `Option.payload_selector("[name='preview_root']")`

Pros:

- no extra wrapper node required
- flexible

Cons:

- leaks selector strings into runtime semantics
- payload resolution becomes another authored query problem
- easier to break during tree refactors

This is better than external consumers walking the tree, but worse than an
explicit payload node.

### Option C: callback / extractor hook

Allow a `SelectionComponent` to have a custom payload extractor callback.

Pros:

- maximally flexible
- can express arbitrarily complex logic

Cons:

- opaque and harder to reason about
- moves selection meaning into code rather than topology
- poor fit for generic tooling, serialization, and inspection
- makes one selection scope behave very differently from another without that
  difference being visible in the tree

This should be a last resort, not the default model.

### Option D: no extra payload channel, only selected option id

Treat the selected option shell as the only selected object and require
consumers to inspect it.

Pros:

- simplest runtime shape

Cons:

- recreates the current problem
- spreads option-internal topology knowledge across consumers
- brittle under refactors

This is the status quo and should not be the long-term direction.

---

## 8. Recommended direction

Prefer an explicit authored payload node over selectors or callbacks.

Recommendation:

1. Keep `SelectionChanged` generic.
2. Add optional `selected_payload: Option<ComponentId>`.
3. Keep optional `selected_title` for display/debug value.
4. Let the selected `Option` define its semantic payload explicitly.

The best first concrete shape is likely one of:

- `OptionValue` child component under the selected option
- or a small extension to `OptionComponent` that stores a payload reference

Between those two, `OptionValue` is more explicit in topology and easier to
inspect visually.

---

## 9. What should `OptionValue` mean?

If `OptionValue` is introduced, it should mean:

- "this is the semantic value anchor for this option"

That anchor can be used in two ways:

### 9.1 Payload is the `OptionValue` node itself

`selected_payload` points to the `OptionValue` component id.

Then downstream systems can inspect children/attachments under it.

Good when:

- the payload is a small subtree
- the payload anchor wants metadata/components attached to it

### 9.2 Payload is the primary child/target of `OptionValue`

`selected_payload` points to the meaningful nested node under `OptionValue`.

Good when:

- the wrapper is just a marker
- consumers want the actual root directly

Either can work, but the rule must be explicit and stable. The simpler rule is:

- `selected_payload` is the `OptionValue` node
- consumers may optionally resolve beneath it in a local, well-defined way

That keeps the event contract consistent.

---

## 10. Asset-panel example

Current conceptual shape:

```text
Option(asset_item)
├── preview_slot
│   └── preview subtree
└── label
```

Proposed shape:

```text
Option(asset_item)
├── preview_slot
│   └── preview subtree
├── label
└── OptionValue
    ├── asset identity marker / ref
    └── optional payload subtree root
```

Then `SelectionChanged` can carry:

- `selected_component = asset_item shell`
- `selected_title = "paint_asset: cube_stamp"`
- `selected_payload = option value anchor`

Paint can then read stable asset identity from the payload anchor rather than
using label text as the primary key.

---

## 11. Paint-tool example

Paint tools are simpler.

For many tool tiles:

- `selected_component` may already be sufficient
- `selected_title` may be sufficient for display

But allowing `selected_payload` still helps because tool meaning becomes
authored and explicit rather than inferred from label text.

That means simple options can use:

- `selected_component == selected_payload`

while richer options can use a separate payload node.

---

## 12. Why this keeps the system generic

This model avoids panel-specific enums because the runtime event only knows:

- selection scope id
- selected option id
- optional title
- optional payload id

It does not know whether the payload means:

- asset identity
- preview root
- tool type
- panel root
- file row
- scene object

That interpretation remains the responsibility of downstream systems, but the
selection scope provides a stable semantic handoff point.

This is generic without being blind.

---

## 13. Open questions

1. Should `selected_title` live in `SelectionEntry`, on the event, or both?

`SelectionEntry.item` already exists. It may be enough to continue using that
field rather than introducing a separate `selected_title`.

2. Should payload resolution be authored on `Option` or `Selection`?

Default answer:

- payload belongs on `Option`, because different options in one selection scope
  may carry different payloads

3. Does `OptionValue` store data directly or only anchor a subtree/reference?

For the current engine shape, anchoring a subtree/reference is the more natural
first step than inventing a new typed value container.

4. Do we ever need a callback?

Probably yes eventually, but only for exceptional cases. It should not be the
main contract.

---

## 14. Recommended next step

Implement the minimum generic extension:

1. add optional `selected_payload: Option<ComponentId>` to `SelectionChanged`
2. teach `SelectionSystem` to resolve payload from the selected option
3. author a payload anchor for asset items
4. update `EditorPaintSystem` to use payload identity rather than label text as
   the primary brush key

This would fix the current paint-selection fragility without committing the
runtime to panel-specific event variants.
