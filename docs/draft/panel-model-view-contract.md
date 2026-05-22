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
- row-local event handlers

Rust should own:

- building the item list
- deciding when to rerender
- exposing the editor command surface that MMS handlers can call through intents

## Clicking Items In MMS

Yes, the item click handlers can live in MMS.

That is the preferred experiment now.

The content factory should eventually be able to do something conceptually like:

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

That snippet is contract-level pseudo-MMS, not a requirement that the current runtime
already supports this exact syntax.

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

Recommended first contract:

- Rust passes item text together with `target_ref` into the factory
- `target_ref` is preferably a guid string in `@uuid:...` form
- MMS emits `EDITOR_SELECT(item.target_ref)`

## Row Identity

Even if MMS owns click handlers, rows should still get stable names derived from `item.key`.

Why keep this?

- debugging
- future testability
- fallback Rust-side binding if needed
- queryability from other MMS code

Example direction:

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

## Records And Structs Requirement

This panel work depends on a more general MMS interop feature:

- Rust needs to pass structured records into MMS functions

That feature should not be panel-specific. It should be specified as a general
records / structs / maps capability for MMS.