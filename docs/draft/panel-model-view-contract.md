# Panel Model View Contract

## Scope

This is the working draft for the first `InspectorSystem` experiment.

It is intentionally local to editor panels. It does not try to define a general
model/view framework for the whole engine.

## Core Decision

Keep the panel model in `InspectorSystem` for now.

That system should own the view-model data it wants to project into MMS. The
model can change freely during experimentation without forcing a reusable API too early.

## Shell And Content Split

The world panel is split into:

- `world_panel(title, items)` in `assets/components/world_panel.mms`
- `world_panel_content(items)` in `assets/components/world_panel_content.mms`

Contract:

- `world_panel(...)` owns the stable shell
- `world_panel_content(...)` owns the rerenderable body
- the `items` model only drives `world_panel_content(...)`

## Model Shape

The next useful model is no longer `items: [String]`.

Important current limitation:

- MMS does not currently have an authored record / struct / dict literal surface
- MMS does not currently expose a first-class `ComponentRef` value kind in source
- so panel item metadata cannot yet be passed into a factory in the shape we actually want

Each world-panel item should know:

- `key`
- `label`
- `depth`
- `selected`
- `target_ref`

`target_ref` is the important addition.

It should identify the scene component the item corresponds to.

Preferred direction:

- add records / structs / maps as general MMS data
- represent `target_ref` inside those records
- do not add a special first-class MMS `ComponentRef` type as the first step

Recommended panel item shape once records exist:

- `key`
- `label`
- `depth`
- `selected`
- `target_ref`

Recommended first `target_ref` encoding:

- canonical string form

Examples:

- guid-backed: `"@uuid:8c4f3e72-..."`
- selector-backed fallback: `"#hero"`

Why this is the better first contract:

- records/maps are broadly useful beyond panels
- Rust needs a general way to pass structured data into MMS anyway
- canonical string refs avoid inventing a special new runtime value kind just for component refs
- the existing engine-side `ComponentRef` resolution model already distinguishes guid vs query forms

So for this panel path, the recommended contract is:

- Rust passes item records into the factory
- each item record includes `target_ref` as a canonical ref string
- MMS handlers emit editor intents using that `target_ref`

## View Responsibilities

MMS should own:

- row structure
- row styling
- row naming

Rust should own:

- building the item list
- deciding when to rerender
- attaching row signal handlers in v1
- exposing any later editor command surface when MMS-owned handlers become practical

## Clicking Items In V1

For v1, item click handlers should be attached from Rust after rendering.

Why:

- `items` is still just `[String]`
- MMS does not yet have the record/struct surface needed to pass stable item metadata cleanly
- Rust can query named rows from the rendered content root and bind handlers there

The useful v1 contract is therefore:

- `world_panel_content(...)` renders named rows under `rows_mount`
- Rust queries those rows from `world_panel_content_root`
- Rust attaches click/select handlers after the subtree is spawned

Because v1 items do not yet carry stable keys, row names are derived from render order:

- `item_0`
- `item_1`
- `item_2`

That is good enough for the first Rust-side binding pass.

## Clicking Items In MMS Later

Once MMS can receive structured item records, the content factory should eventually be able to do something conceptually like:

```mms
export fn world_panel_content(items) {
    T {
        name = "world_panel_content_root"

        for item in items {
            let row = world_panel_row(item.key, item.label, item.depth, item.selected)

            on(row, "Click", fn(event) {
                emit(EDITOR_SELECT(item.target_ref))
            })

            row
        }
    }
}
```

That snippet is later contract-level pseudo-MMS, not a requirement for v1.

## Editor API Surface

The editor should be exposed to MMS through intents, not through a large direct host API.

First candidate intent:

- `EDITOR_SELECT(target_ref)`

Possible later intents:

- `EDITOR_FOCUS(target_ref)`
- `EDITOR_SET_GIZMO_MODE(mode)`
- `EDITOR_SET_GIZMO_SPACE(space)`
- `EDITOR_OPEN_INSPECTOR(target_ref)`

## Why Intents Are The Right Boundary

This keeps the contract narrow.

- MMS expresses what editor action it wants
- Rust stays responsible for how that action is executed
- editor-side state changes remain routed through the existing signal / intent model

That is a better fit than exposing `select_editor_target(...)`-style host functions directly to MMS.

## Intent Payload Shape

For the first draft, `EDITOR_SELECT(...)` should accept the component identity the item represents.

Preferred contract:

- accept a canonical target-ref string and resolve it at execution time

Acceptable experimental contract:

- accept a live `ComponentId` when the panel model is generated entirely at runtime

If both are supported later, they should be treated as one semantic target concept rather than two unrelated APIs.

Recommended later contract:

- Rust passes item text together with `target_ref` into the factory
- `target_ref` is preferably a guid string in `@uuid:...` form
- MMS emits `EDITOR_SELECT(item.target_ref)`

## Row Identity

For v1, rows should get easy query names from the rendered content root.

Why keep this?

- debugging
- Rust-side post-render binding
- future testability
- queryability from other MMS code

V1 direction with string-only items:

- first row name: `item_0`
- second row name: `item_1`

Later direction with stable item keys:

- item key: `node_42`
- row name: `item_node_42`

## Rerender Rule

Only rerender `world_panel_content(...)` when the content model changes.

Do not rerender for:

- scrolling
- hover
- pointer motion

Do rerender for:

- topology changes that affect visible items
- selection changes that affect row flags

## Open Design Question

How should `items` reach MMS?

Options:

- structured values if MMS grows the right surface
- generated MMS source with one explicit row call per item
- parallel arrays and helper functions

The only hard requirement is that the model carries `target_ref` and that the content factory can author the click handler against it.

For v1, before structured values exist, the hard requirement is smaller:

- Rust must be able to query rendered rows reliably from `world_panel_content_root`

## Records And Structs Requirement

This panel work depends on a more general MMS interop feature:

- Rust needs to pass structured records into MMS functions

That feature should not be panel-specific. It should be specified as a general
records / structs / maps capability for MMS.