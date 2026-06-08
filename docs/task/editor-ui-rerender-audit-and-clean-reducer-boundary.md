# Editor UI Rerender Audit And Clean Reducer Boundary

## Status

Current investigation note based on the live inspector detail experiment.

No architectural refactor implemented yet.

## What currently works

The standalone world-space inspector detail experiment works better than the in-panel detail slot path.

Current observed behavior:

- a separate inspector panel spawned at `(0, 8, 0)` renders correctly
- its detail view renders without overlapping the sidebar
- the detail content appears stable when rendered as a complete independent panel

This is important because it changes the most likely diagnosis.

## Updated conclusion

The evidence now suggests the primary problem is not the core `layout_system` width math by itself.

The stronger suspicion is the current stopgap approach to:

- materializing MMS-authored trees
- attaching/re-attaching subtrees into already-rendered MMS panel shells
- mixing reducer transitions, model rebuilding, tree diffing, subtree replacement, and live ECS mutation in one adapter path

The specific clue is:

- when the detail content is rendered as a fresh separate panel, it behaves correctly
- when the detail content is injected into the existing inspector panel shell, the behavior is worse and some expected sidebar/selection structure can disappear or behave unexpectedly

That points at the composition/update model around live MMS-rendered subtrees, not just the raw inline/block layout logic.

## Why this matters

The current stopgap adapter is doing too many jobs at once:

- state synchronization
- event translation
- panel model building
- panel shell materialization
- subtree replacement
- post-render selection wiring
- partial rerender heuristics

That makes it hard to reason about:

- which reducer transition should rebuild which panel
- which interactions should only mutate local component state
- which UI updates should reuse existing rendered trees
- which UI updates should rebuild a whole panel shell or panel body

The current architecture makes "what caused this UI subtree to change?" harder to answer than it should be.

## Immediate design direction

Treat this as an editor UI architecture issue first, not a narrow detail-slot bug.

The direction should be:

1. define cleaner reducer-owned editor workspace state
2. define a clear model/view contract per panel
3. make rerender triggers explicit and coarse at first
4. then incrementally increase rerender granularity only where needed

## Recommended first simplification

For the editor UI, prefer full panel rerender on the important cross-panel state changes before trying to preserve fine-grained subtree updates.

In particular:

- a world-panel selection change should be allowed to rerender the active unpinned inspector panel completely
- the initial detail-view population can be part of that full inspector rerender
- do not try to keep the old shell alive while patching the sidebar and detail pane independently until the ownership model is cleaner

That is the simplest path to a trustworthy baseline.

## Interactions that should not trigger full rerender

Once the baseline is clean, some interactions should stay local and avoid panel rerender:

- inspector sidebar row highlight changes driven by `SelectionComponent`
- text entry in a field editor
- caret/focus movement inside text input
- hover-only visual state
- scroll offset changes

These are local view-state or component-state updates, not panel-model changes.

So the intended split is:

- cross-panel semantic selection change: rerender panel model/view
- panel-local selection/highlight/input state: do not rerender the whole panel

## Working hypothesis for a clean baseline

For the first clean editor UI pass:

- keep the inspector panel as one reducer-owned view model
- when world-panel selection changes and the active inspector panel is not pinned:
  - rebuild the inspector panel model
  - rerender the whole inspector panel view
- when the active panel is pinned:
  - spawn or retarget according to inspector workspace reducer rules
- when the user clicks inside the inspector sidebar:
  - update panel-local subtree selection state
  - avoid full rerender unless the logical inspected target actually changes

This matches the current intuition:

- detail + sidebar can rerender together initially
- later, rerender granularity can be narrowed if needed

## Audit we need to do

### 1. Identify every editor UI reducer state owner

We need an explicit map of which module owns:

- editor context selection
- world panel selection
- inspector workspace state
- panel-local sidebar selection
- asset panel state
- paint panel state
- focus state

Today that ownership is split across reducer state, ECS components, and stopgap adapter logic.

### 2. Separate reducer transitions from side effects

Reducer code should only compute next logical state.

Effects should be a separate phase that decides:

- rerender panel shell
- rerender panel body
- update selection component
- attach/remove a spawned subtree
- update text/status labels

Today those concerns are interleaved.

### 3. Document panel-level rerender contracts

For each editor panel, define:

- what state drives its full model
- what events force full rerender
- what events only mutate local component/UI state
- which subtree boundaries are stable shell boundaries

This should be written down before trying to optimize rerender granularity further.

### 4. Audit the stopgap MMS adapter responsibilities

We need a concrete breakdown of which parts of
[editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1)
should move into:

- reducer/state modules
- panel-specific systems
- shared editor workspace coordination
- MMS materialization helpers
- view diff/rerender code

The goal is to delete the stopgap adapter, not just keep shrinking it forever.

### 5. Decide the first "clean" editor UI target

The recommended first clean target is the inspector panel, because it currently demonstrates:

- cross-panel selection-driven updates
- pinned/unpinned workspace logic
- sidebar local state
- detail-view rendering
- the tension between full rerender and local updates

## Recommended architecture direction

Follow the direction already described in:

- [docs/draft/nested-reducers-for-panels.md](/home/rei/_/cat-engine/docs/draft/nested-reducers-for-panels.md:1)
- [docs/draft/panel-model-view-contract.md](/home/rei/_/cat-engine/docs/draft/panel-model-view-contract.md:1)
- [docs/draft/inspector-panel-multi-instance-and-v2.md](/home/rei/_/cat-engine/docs/draft/inspector-panel-multi-instance-and-v2.md:1)
- [docs/task/shared-editor-ui-routing-and-paint-state-manager.md](/home/rei/_/cat-engine/docs/task/shared-editor-ui-routing-and-paint-state-manager.md:1)

Concretely:

- use nested reducers to mirror editor workspace state ownership
- keep reducers pure
- route engine/UI events into panel-domain events
- make the view layer consume explicit panel models
- keep side effects outside reducers

## Incremental plan

### Phase 1: clarify and freeze current behavior

- document what currently rerenders the inspector panel
- document what should rerender it instead
- record which local interactions should remain non-rerendering

### Phase 2: make full inspector rerender the clean default

- on world-panel semantic selection change, rebuild the active unpinned inspector panel completely
- stop trying to patch sidebar and detail as separate ad hoc subtree operations for the baseline path
- keep pinned panel semantics in the reducer

### Phase 3: separate panel-local state updates

- keep row highlight / focused row / text editing / scrolling local
- only trigger rerender when reducer-owned model changes

### Phase 4: split stopgap adapter responsibilities

- move reducer logic out
- move panel materialization contracts out
- narrow the remaining system to event routing + effect application

### Phase 5: delete the stopgap adapter

- replace it with explicit panel systems and shared editor workspace coordination

## Practical next step

Before changing behavior further, write a concrete rerender audit for the current inspector flow:

- world-panel click
- editor context update
- inspector workspace reducer update
- model rebuild
- full panel spawn vs subtree rerender
- panel-local selection changes

That audit should identify which current updates are:

- semantic workspace changes
- panel-local state changes
- pure view-state changes

Once that map exists, the first clean inspector rerender implementation can be done with much less guesswork.
