# Draft: externally opaque child topology indirection

Date: 2026-04-17

Status: draft only.

This note proposes a generalized topology feature for components that want to accept authored or
attached children as if they were “props”, while remaining free to place those children somewhere
else inside an internal runtime-owned subtree.

This is the generalized version of what surfaced while refactoring `Scrolling`:

- from the outside, authors should be able to write or attach children directly under a component
- inside the runtime, that component may want those children to live under an internal helper node
- the internal topology should be opaque from the outside
- the component should own the policy for where incoming external children belong

The motivating intuition is similar to React “children as props”, but the runtime model here is not
virtual DOM. It is component-graph topology ownership.

---

## 1. Problem statement

Today, some components want to expose a simple authored shape while secretly maintaining a more
structured internal subtree.

Examples:

- `Scrolling` wants to expose:

```text
Scrolling
└── content...
```

but internally wants something more like:

```text
Scrolling
└── __scroll_track
	└── content...
```

- future panel/layout/widget components may want:

```text
Widget
├── child_a
└── child_b
```

while internally maintaining:

```text
Widget
├── __background
├── __header_slot
└── __content_slot
	├── child_a
	└── child_b
```

- text-like or UI-like components may eventually want separate routing for:
  - content children
  - style children
  - helper/operator children

Right now, this kind of behavior is handled ad hoc:

- manual init-time topology rewriting
- bespoke system logic
- direct `set_parent` / `add_child` rewrites in system code
- no standard trait/interface for “this component owns how external children are routed”

That works locally, but it does not scale cleanly.

---

## 2. Core idea

Introduce a standard component capability for:

> intercepting externally attached children and deciding where they belong inside a component-owned
> subtree.

From the outside, the operation still looks like normal parent/child authoring:

```text
Parent
└── FancyComponent
	└── authored_child
```

But after the component’s topology policy runs, the runtime may stabilize the subtree as:

```text
Parent
└── FancyComponent
	├── __runtime_helper_a
	└── __runtime_content_slot
		└── authored_child
```

The author does not need to know or care about the internal helper shape.

That is what “externally opaque” means here:

- external code authors/attaches children in the obvious place
- the component may internally route those children elsewhere
- internal helper topology is an implementation detail

---

## 3. Design goals

The feature should make the following possible:

1. a component can declare that it owns child routing within its subtree
2. children attached from outside can be treated as semantic inputs, not necessarily as literal
   direct children that remain where they were attached
3. internal helper topology can remain opaque and runtime-owned
4. the same mechanism should work for:
   - authored MMS trees
   - hand-built runtime/ECS trees
   - dynamic attach/reparent operations after init
5. topology ownership should be explicit and inspectable, not hidden in random system code

The feature should avoid:

- requiring every component to reimplement attach interception manually
- making helper topology visible as required authoring ceremony
- breaking normal components that do not need topology indirection

---

## 4. Terminology candidates

The naming is not settled yet. These are the main candidates.

### 4.1 Recommended conceptual term: child topology routing

This document will use:

- **external child** = a child attached/authored under a component from outside that component’s own
  runtime-owned helper logic
- **internal helper topology** = runtime-owned nodes such as `__scroll_track`
- **child routing** = deciding where an external child should actually live within the owned subtree
- **topology owner** = the component that owns this routing policy

This wording is plain and matches the engine’s current emphasis on topology.

### 4.2 Trait naming candidates

Good candidates:

- `ChildTopologyRouter`
- `OwnedChildTopology`
- `ExternalChildRouter`
- `TopologyOwner`
- `ChildAttachmentRouter`

My current favorite is:

```rust
trait ChildTopologyRouter
```

because it says exactly what the feature does.

### 4.3 Slot terminology candidates

If components can route different kinds of children to different places, we may want a slot term:

- `content slot`
- `style slot`
- `header slot`
- `background slot`

This should not imply HTML/DOM semantics too strongly. It is really about internal topology targets.

---

## 5. Relationship to existing systems

This proposal sits near several existing ideas, but is not identical to any of them.

### 5.1 Not the same as signal pipelines

Signal pipelines rewrite routing of intents/events.

This proposal rewrites routing of **children in the component graph**.

Signal pipeline mental model:

- “when a signal flows, intercept and transform it”

This proposal:

- “when a child is attached or authored, intercept and route it into owned topology”

### 5.2 Related to topology splice helpers

The splice proposal in [docs/refactor/splice-component-into-topology.md](../refactor/splice-component-into-topology.md)
is about a convenient topology rewrite API.

This proposal is higher-level:

- splice helper = one explicit topology operation
- child topology routing = a reusable component capability that decides where externally attached
  children belong over time

### 5.3 Related to style inheritance / topology query helpers

The query helpers proposed in
[docs/refactor/topology-queries-and-style-inheritance.md](../refactor/topology-queries-and-style-inheritance.md)
would likely be useful here, but they do not solve this problem directly.

This problem is about **owned topology mutation policy**, not just traversal/query semantics.

---

## 6. Proposed behavior model

### 6.1 Baseline rule

When a child is attached under a component that implements the routing trait, the engine should:

1. recognize that the parent component owns child routing
2. ask it where externally attached children belong
3. allow it to move the child to the correct internal location
4. preserve the illusion that authoring/attachment happened “under that component” semantically

In other words, this becomes a component-local topology mutation pipeline.

### 6.2 What counts as an external child

This is critical.

An **external child** should mean:

- a child attached under the topology owner by authored MMS structure, clone attach, or runtime attach
- where that child was not inserted there by the topology owner itself as part of its own helper setup

We need to distinguish:

- user/content children
- runtime-owned helper children

Without that distinction, the router risks recursively re-routing its own helper nodes.

### 6.3 Runtime-owned helper nodes

The component should be able to create helper nodes such as:

- `__scroll_track`
- `__background`
- `__content_slot`
- `__header_slot`

Those helpers should be clearly marked as runtime-owned, either by:

- reserved labels/names
- dedicated marker components
- or an explicit ownership record in the router protocol

Label-only detection is probably sufficient for v1, but a real marker/ownership concept may be better long term.

---

## 7. Draft trait shape

The exact API is open, but a useful first shape would look something like this:

```rust
trait ChildTopologyRouter {
	/// Ensure any runtime-owned helper topology exists.
	fn ensure_child_topology(
		&mut self,
		world: &mut World,
		emit: &mut dyn SignalEmitter,
		owner: ComponentId,
	);

	/// Called when an external child is attached or discovered under `owner`.
	fn route_external_child(
		&mut self,
		world: &mut World,
		emit: &mut dyn SignalEmitter,
		owner: ComponentId,
		child: ComponentId,
	);

	/// Decide whether a child should be treated as runtime-owned/helper topology.
	fn is_runtime_owned_child(
		&self,
		world: &World,
		owner: ComponentId,
		child: ComponentId,
	) -> bool;
}
```

That is intentionally imperative and world-mutation oriented, because the engine is not trying to be a pure declarative tree reducer.

### 7.1 Why not only one method?

Splitting helper creation from child routing is useful because:

- helper topology may need to exist before routing can happen
- some components may need to rebuild or verify helper structure even with no new children
- it makes init-time sync and attach-time sync use the same interface

### 7.2 Alternate “plan object” shape

If we want a more declarative style, the trait could instead return a routing plan:

```rust
enum ChildRoutingDecision {
	KeepDirect,
	ReparentTo(ComponentId),
	Reject,
}
```

But that likely still needs an imperative helper-topology phase before the decision can be made.

So for v1, the imperative trait is probably simpler.

---

## 8. Lifecycle points where it should run

The feature needs to run in at least two situations.

### 8.1 Init-time sync

When a component subtree is first initialized, the router should be able to:

- create helper topology
- inspect current direct children
- move external children into the correct internal slot(s)

This is what enabled the first `Scrolling` ownership slice.

### 8.2 Attach-time / parent-change sync

When a child is attached later under the topology owner, the router should run again.

This is needed for:

- late-authored attachments
- editor insertions
- runtime clone/attach operations
- any future UI/widget systems that compose children after init

Without this, the feature is only an init-time convenience, not a true topology ownership model.

### 8.3 Maybe detach-time sync

Detach/removal may also require policy hooks if components want to maintain invariants like:

- “content slot should always exist even when empty”
- “helper topology should collapse when no content remains”

That may be phase two, not v1.

---

## 9. Suggested runtime integration points

There are several plausible places to hook this in.

### 9.1 In `SystemWorld` registration/init helpers

For init-time sync, `SystemWorld` is the most obvious place.

Pros:

- already coordinates component registration
- already has access to `World` and `SignalEmitter`
- good fit for initial helper-topology setup

Cons:

- does not by itself solve later attach-time routing

### 9.2 In `IntentExecutor::Attach` / topology mutation path

For attach-time routing, the topology mutation layer is the most natural place.

Conceptually:

1. attach child under parent
2. emit `ParentChanged`
3. if parent implements child routing, immediately let it rehome the child

Or equivalently:

1. detect that target parent is a topology owner
2. ask it for the actual destination before finalizing the topology

The first form is probably easier to implement incrementally.

### 9.3 As a dedicated topology-routing pass

Another option is a small dedicated pass/system:

- observe `ParentChanged`
- if the new parent is a topology owner, call its routing logic

Pros:

- aligns with the signal-driven architecture
- keeps routing logic out of raw attach executor code

Cons:

- adds another level of indirection
- must avoid loops or repeated self-routing

This may actually be the cleanest long-term model if we want topology mutation to feel pipeline-like.

---

## 10. `Scrolling` as the first concrete use case

`Scrolling` is the proving ground.

Desired external authoring:

```text
TransformPipelineOutput
└── T
	└── Scrolling
		└── items...
```

Desired internal runtime shape:

```text
TransformPipelineOutput
└── T
	└── Scrolling
		└── __scroll_track
			└── items...
```

The routing policy is simple:

- ensure `__scroll_track` exists
- any external child under `Scrolling` that is not runtime-owned should be reparented under `__scroll_track`
- scrolling state moves `__scroll_track`, not an ancestor transform

This is exactly the kind of behavior that should not have to be bespoke forever.

---

## 11. Future use cases

Once generalized, this pattern could support:

### 11.1 Panel/widget content slots

```text
Panel
├── title
└── body
```

internally routed to:

```text
Panel
├── __chrome
└── __content_slot
	├── title
	└── body
```

### 11.2 Style-like or prop-like children

Some components may want to treat certain children as semantic inputs rather than content nodes.

Examples:

- a component-local style child
- a component-local policy/operator child
- an alternate content child routed to a named internal slot

This is the closest analog to “components as props” in the current ECS/tree model.

### 11.3 Layout-owned helper wrapping

Layout features that need helper topology could also become more component-local and less ad hoc if a standard child-routing capability exists.

---

## 12. Open design questions

### 12.1 Should routing be purely implicit?

Maybe for simple cases.

But for more complex components, we may eventually want explicit slot markers or child-role markers, e.g.:

- content children
- style children
- header children
- background children

V1 should probably stay implicit and topology-based.

### 12.2 How do we prevent routing loops?

The runtime must avoid cases where:

- a component routes its own helper child again
- `ParentChanged` triggers re-routing repeatedly
- attach-time and init-time routing fight each other

This strongly suggests the trait needs a standard `is_runtime_owned_child(...)` or equivalent helper ownership filter.

### 12.3 How visible should helper topology be to tooling?

Open question:

- should editor/inspector views show helper nodes like `__scroll_track` plainly?
- or should some helper topology be hidden/collapsed in tooling?

The runtime feature does not require hidden tooling, but this will matter for usability.

### 12.4 Should child routing be component-driven or system-driven?

Two broad models:

- component trait owns routing policy directly
- a system owns routing behavior for certain component types

Given the prompt, a trait-like capability is the more general and composable target.

---

## 13. Recommended first implementation path

If this draft turns into code, the smallest useful staged path is:

### Phase 1

- define the terminology and trait/capability shape
- use `Scrolling` as the reference component

### Phase 2

- add init-time routing support only
- keep the implementation narrow and internal
- prove the `Scrolling` use case cleanly

### Phase 3

- add attach-time routing for externally attached children
- likely via `ParentChanged` observation or topology mutation hooks

### Phase 4

- evaluate whether other components should adopt the same capability

This keeps the concept grounded in a real use case before generalizing too aggressively.

---

## 14. Tentative recommendation

Current recommendation:

- use **child topology routing** as the conceptual term
- prototype a trait or trait-like capability named `ChildTopologyRouter`
- treat `Scrolling` as the first concrete adopter
- keep runtime helper topology opaque from normal authoring
- support both init-time and later external attachment routing as part of the long-term design

Short version:

> Let components own how externally attached children are routed into internal helper topology,
> while preserving simple outside authoring.

That is the cleaner, generalized form of “components as props” for this engine’s component graph.
