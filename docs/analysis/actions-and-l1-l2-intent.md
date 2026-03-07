# Actions and L1/L2 intent execution

This doc explores how to restructure the current intent/action layer.

## Motivation

Right now the codebase has three overlapping concepts:

- **`ActionComponent`**: declarative “do this intent” stored on a component.
- **`RxIntentExecutor`** (currently delegating into `ecs/system/action_system.rs`): an intent *interpreter* that expands higher-level intents into lower-level intents.
- **`SystemWorld::execute_intent_signal`**: the low-level mutation executor for internal registration/update/remove intents.

The term “action system” is overloaded: it currently means “interpret many intents” rather than “handle ActionComponent lifecycle”.

## Proposed split (conceptual)

### 1) `ActionComponent` lifecycle (declarative trigger)

“Action” should mean: *how an intent is triggered declaratively by component registration*.

Desired behavior:

- ActionComponent is **registered** like any other component.
- On registration, it may **auto-fire** its stored intent.
- Auto-fire should be **suppressed** if the ActionComponent is attached under a `KeyFrameComponent` (so it can be driven by animation/timeline instead of firing on boot).

So the “action system” becomes narrowly about:

- `RegisterAction { component }`
- deciding whether to auto-fire

It should not be the general-purpose executor for all intents.

### 2) Intent execution: two executors, clearer responsibilities

Rename/reframe:

- **Intent executor** (today: `RxIntentExecutor`)
  - Executes *higher-level intents* that are composition-heavy.
  - Emits follow-up intents/events.
  - **Guideline**: if the implementation is only a few lines and not system-specific, keep it here.
  - If it grows beyond a few lines, this executor should still “own fulfilling the intent”, but should delegate to a system.

- **Mutation executor** (today: `SystemWorld::execute_intent_signal`)
  - Executes *low-level engine mutations*:
    - register/remove component types with systems
    - update system caches
    - apply direct state changes that must remain centralized

This aligns with the drain-point model: events → handlers (observers) → intents → executors.

### 3) L1 vs L2 intent modules (optional organization)

If the intent executor grows, we can organize it into:

- `intent_l1.rs`: “primitive” intent execution (small, direct, non-system-specific)
  - e.g. attach/detach/remove-child/remove-subtree, simple property sets

- `intent_l2.rs`: “composed” intent execution (expands into other intents)
  - e.g. `AttachClone` (encode/decode prefab then attach)
  - e.g. higher-level editor/gameplay commands that emit multiple mutations

This is a code-organization tactic, not a semantic promise.

## Immediate refactor plan

1) Move intent interpreter logic currently living in `ecs/system/action_system.rs` into `ecs/rx` (near `RxIntentExecutor`).
2) Move low-level mutation execution currently in `SystemWorld::execute_intent_signal` into `ecs/rx` as a `MutationExecutor`.
3) Reduce `ecs/system/action_system.rs` to only ActionComponent lifecycle:
   - `RegisterAction` intent
   - auto-fire rules (skip if under `KeyFrameComponent`)

## Notes / questions

- Should “user intents” and “internal mutation intents” be split into separate enums?
  - Today they’re mixed inside `IntentValue`.
  - Splitting would make executor boundaries explicit and reduce accidental misuse.

- Keyframe-triggering of ActionComponents:
  - The initial goal is **only** to suppress auto-fire under `KeyFrameComponent`.
  - Later we can add explicit keyframe playback logic to emit the stored intent.
