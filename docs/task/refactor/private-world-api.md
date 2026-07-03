# Private `World` API (refactor note)

Date: 2026-02-27

This doc captures a refactor direction: **prefer the `Universe` API for scene construction** and make direct access to `Universe.world` *exceptional*, not the default.

This is motivated by the fact that examples (and therefore users) routinely reach into `Universe.world` for operations that already have safer/higher-level `Universe` equivalents, most notably parenting and initialization.

## What we changed (examples)

- Replaced `universe.world.add_child(parent, child)` with `universe.attach(parent, child)`.
- Replaced `universe.world.init_component_tree(root, &mut universe.command_queue)` with `universe.add(root)`.

Why:

- `Universe::attach` emits `ParentChanged` and auto-initializes the attached subtree when the parent is already initialized.
- `Universe::add` is the supported entrypoint for making a subtree “live” (runs `Component::init` as needed).

These are the two operations that were most frequently done via `World` in examples.

## Problem statement

Today, `Universe` exposes:

- `pub world: ecs::World`
- `pub command_queue: ecs::CommandQueue`

That makes it very easy for user code to bypass invariants that `Universe` is trying to maintain:

- event emission (`ParentChanged`)
- initialization rules (what gets `init`’d and when)
- future cross-cutting hooks (e.g. editor bookkeeping, replication, undo/redo)

Even when user code is “correct”, it becomes coupled to internals (exact init order, signal push points, etc.).

## Goal

Make the **common path** (examples, typical app code, gameplay code) use `Universe` methods.

Allow “raw World access” only when truly needed (debug tooling, advanced systems, migration code), and make that need explicit at call sites.

## Universe already covers the common needs

For the operations most examples were using `World` for:

- Parenting: `Universe::attach(parent, child)`
- Subtree initialization: `Universe::add(root)`
- Structured deletion: `Universe::remove_child`, `Universe::remove_children`
- Prefab-style cloning: `Universe::attach_clone`
- Signals: `Universe::add_signal_handler`, `Universe::remove_signal_handler`

So the *best* fix is not “make `World::add_child` public in a different way” — it’s to route typical topology and lifecycle changes through `Universe`.

Progress note: we now also expose a small set of read-only query helpers directly on `Universe` (`parent_of`, `children_of`, `get_component_by_id_as`) so typical app/example code doesn’t need to reach into `world` for basic inspection.

## Proposed API exposure policy (no code changes yet)

### 1) Keep `world` visible for now (but discourage casual use)

We’re not committing to removing `world` as a public field yet.

Rationale:

- Engine-internal systems (inside `src/`) legitimately work directly with `World`.
- In the short term, keeping `world` public avoids churn while we finish adding the small, stable `Universe` wrapper surface that examples/app code should use.

A future change could still make raw access more explicit (feature gate / unsafe module), but it’s not required to get most of the value.

- `pub world: ecs::World` → `world: ecs::World` (private) or `pub(crate) world: ecs::World`.

### 2) Provide focused `Universe` wrappers for common usage

We already have `add` and `attach`, and we’ve added read-only query wrappers:

- `Universe::parent_of`
- `Universe::children_of`
- `Universe::get_component_by_id_as`

The next most common “World touch points” in user code are still:

- creating components (`register` vs `add_component`)
- inspection queries (`parent_of`, `children_of`, `get_component_by_id_as`)

We can keep construction available without “handing out the entire `World` API” by adding targeted methods (future):

- `Universe::spawn<C: Component>(c: C) -> ComponentId` (name TBD)
- `Universe::spawn_named(name: &str, c: C) -> ComponentId`
- `Universe::children_of(id) -> &[ComponentId]` (or iterator)
- `Universe::parent_of(id) -> Option<ComponentId>`
- `Universe::get<T: Component>(id) -> Option<&T>` and `get_mut` variants

These wrappers let us:

- keep signatures stable even if `World` storage changes
- insert future hooks (metrics, tracing, editor ownership tags)
- make “dangerous” operations obviously absent

### 3) Add an explicit escape hatch (exceptional access)

Some code will genuinely need raw ECS access. Make it explicit:

Option A — feature-gated:

- `#[cfg(feature = "unsafe-world-api")] pub fn world_mut(&mut self) -> &mut World`
- `#[cfg(feature = "unsafe-world-api")] pub fn world(&self) -> &World`

Option B — opt-in module:

- Put raw accessors under `engine::unsafe_api` so calling code must write e.g.
  `use cat_engine::engine::unsafe_api::UniverseWorldExt;`

Option C — closure-based access:

- `pub fn with_world_mut<R>(&mut self, f: impl FnOnce(&mut World, &mut CommandQueue) -> R) -> R`

Closure-based access has a nice property: you can log/trace all uses centrally, and you can later tighten invariants without changing every callsite.

### 4) Don’t try to prevent access where it’s structurally required

There are places where raw `World` access is part of the model:

- Signal handlers are currently `fn(&mut World, &mut CommandQueue, &Signal)`.

That’s fine for now. The goal is not “no one can ever touch `World`”, it’s:

- scene building, gameplay glue, and examples shouldn’t need it
- when you do use it, it should be deliberate and visible

## Migration strategy

1. Update examples to stop using `universe.world.add_child` and `init_component_tree` (done).
2. Audit docs and README snippets to prefer `Universe` calls.
3. Introduce the wrapper methods needed to replace remaining `world.register`/`add_component` usage cleanly.
4. Make `Universe.world` non-public.
5. Provide the explicit escape hatch (`unsafe-world-api` feature or `unsafe_api` module).

## Notes on `register` vs `add_component`

Examples currently use both:

- `world.add_component(...)`
- `world.register(...)`

Before introducing `Universe::spawn`, we should decide whether:

- they are actually distinct (e.g. GUID/name semantics), or
- one should become the canonical public constructor path.

A good end-state is:

- one “normal” constructor on `Universe`
- any specialized construction routes documented and placed behind explicit names/flags

