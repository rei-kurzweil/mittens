# Draft: router system

Date: 2026-04-18

Status: draft only.

This note defines a more general runtime model for child topology routing:

- not only some special components can route children
- any component should be able to opt into routing external children into internal helper topology
- the engine should have a standard `RouterSystem`
- authored MMS should stay simple and declarative, while internal topology remains opaque

This is still spec mode.
No code changes are proposed here.

---

## 1. Motivation

The narrower trait-shaped framing solves the `Scrolling` problem, but it still frames child routing
as something that only certain component types do.

The broader direction is:

> any component should be able to accept authored/attached children and internally decide where
> those children belong.

That suggests we should stop thinking only in terms of:

- “special component implements a trait”

and instead think in terms of:

- “the runtime has a general child-routing layer”
- “components can opt into routing policy declaratively”

This moves the concept closer to:

- component-local topology mutation
- component-local slots/props/content routing
- eventually, templates / functional components / reusable UI components

---

## 2. Key intuition

From the outside, authors should be able to write something like:

```text
Scrolling {
    item
    item_2
    item_3
}
```

or:

```text
LayoutRoot {
    Router { target = "container" }
    T { name = "container" }
    authored_child_a
    authored_child_b
}
```

and the runtime should be free to interpret those children as semantic inputs rather than literal
final topology.

That means initialization may need to:

1. inspect direct authored children
2. create helper/runtime-owned topology
3. decide which children are external inputs vs helper nodes vs props/style nodes
4. reattach those external children into the correct internal target(s)

The important property is:

- external authoring stays simple
- internal subtree structure stays opaque

---

## 3. Relationship to templates / functional components

This idea is adjacent to templates or functional components, but it is probably a lower-level
primitive.

Templates / functional components usually imply things like:

- reusable authored macros or factories
- parameter passing
- subtree expansion
- maybe named slots

A router system does not require all of that.

A router system only needs to answer:

> when children are authored or attached under this component, where do they really belong inside
> the owned subtree?

So the likely layering is:

1. **router system** = low-level topology ownership primitive
2. **templates / functional components** = higher-level authoring abstraction that may use routing

This is a good sign.
It means the router system can be useful immediately without waiting for a whole function/template
language.

---

## 4. Core proposal

Introduce a `RouterSystem` plus a standard `RouterComponent`-style policy surface.

Conceptually:

- the runtime observes init-time and attach-time topology changes
- if a component is router-enabled, the router system asks how external children should be routed
- the system then maintains the owned helper topology accordingly

This is like a topology mutation pipeline that runs locally for a subtree owner.

---

## 5. Recommended mental model

### 5.1 Child routing is universal-capability, not niche behavior

Any component can become a topology owner.

That does **not** mean every component must actively rewrite children.
It means:

- all components are eligible to participate in the routing model
- most components will use the default no-op policy
- some components will declare an explicit routing policy

This is cleaner than treating routing as a one-off special power belonging only to components like
`Scrolling`.

### 5.2 Default behavior should be pass-through

The default router behavior should be:

```text
external children remain direct children
```

In other words, the default routing policy is no-op / passthrough.

That means we do **not** need every normal component to author a visible `Router.off()` unless we
want that for stylistic clarity.

Recommended baseline:

- universal routing capability exists conceptually
- explicit router policy enables non-trivial behavior
- absence of router policy means passthrough

### 5.3 `Router.off()` is a later extension, not v1

A disabled router surface could still be useful in cases where:

- a component type has an implicit default router policy
- a particular authored site wants to turn that off explicitly

But this should be optional, not required for every normal component.

Important clarification:

- this is **not** part of the recommended v1 model
- this belongs to a later phase where some component types may have implicit/internal routers
- one motivating example would be `Scrolling { Router.off() }`, where the authored `Router.off()`
    disables the component type's built-in/internal router behavior

So for v1:

- no implicit internal routers need to be overridable yet
- no `Router.off()` behavior is required yet
- explicit `Router { ... }` on a parent-owned subtree is enough

---

## 6. Terminology

Recommended terms for this draft:

- **RouterSystem** = the runtime system that maintains child-routing behavior
- **RouterComponent** = the authored/declarative policy component
- **topology owner** = the component whose subtree is being managed
- **external child** = a child authored/attached from outside the owner’s own helper logic
- **internal target** = a runtime- or authored-owned node that should receive routed children
- **passthrough routing** = leave external children as direct children

This keeps the vocabulary shorter and cleaner than longer topology-indirection phrasing.

---

## 7. Proposed authored API direction

A first useful MMS shape could be:

```text
T {
    LayoutRoot {
        name = "app-ui"
        Router {
            target = "container"
        }
        T {
            name = "container"
        }
    }
}
```

The intended meaning is:

- `LayoutRoot` is the topology owner
- `Router` says: route external children to the internal target named `container`
- the child named `container` remains an authored/internal target node
- any other externally attached/authored children under `LayoutRoot` should be rehomed under
  `container`

This is a useful “components as props / content slot” primitive without needing a whole template
system yet.

### 7.1 Minimal `Router` config candidates

Possible v1 fields:

- `target = "container"`
- `ignore = ["internal_thing", "other_internal_thing"]`
- maybe later: `mode = passthrough | named_target | classify`
- maybe later: named rules or child-role rules

Good first-step goal:

```text
Router {
    target = "container"
    ignore = ["internal_thing", "other_internal_thing"]
}
```

That is already enough to prove the concept.

### 7.2 Meaning of `target` and `ignore`

Recommended v1 meaning:

- `target` = the internal node that should receive externally routed children
- `ignore` = internal children under the topology owner that should never be treated as external
  routed content

This gives the router policy two important powers:

1. choose where external content goes
2. explicitly protect known internal/helper children from being re-routed

For example:

```text
LayoutRoot {
    Router {
        target = "container"
        ignore = ["toolbar", "status"]
    }

    T { name = "toolbar" }
    T { name = "container" }
    T { name = "status" }
}
```

The intended behavior is:

- external children get routed to `container`
- `toolbar` stays where it is
- `status` stays where it is
- neither `toolbar` nor `status` is mistaken for routed content

### 7.3 Resolution syntax in MMS vs Rust

This should work by both selector-like name resolution and component reference.

For v1, the string forms in `target` and `ignore` should be interpreted conservatively:

- they are **name queries relative to the immediate subtree of the router's parent**
- they are not yet full general MMS queries
- they should resolve against nodes under the topology owner, not arbitrary global tree locations

So:

```text
Router {
    target = "container"
    ignore = ["toolbar", "status"]
}
```

should mean:

- find `container` under the immediate subtree of the router's parent
- find `toolbar` and `status` under that same local subtree

Later, we can expand this to full MMS-query resolution for deeper targets.

#### MMS

In MMS, the most natural surface is selector/reference-like authoring:

```text
Router {
    target = "container"
    ignore = ["internal_thing", "other_internal_thing"]
}
```

Later, when MMS supports stronger component references, this should also be valid conceptually:

- `target = some_component_ref`
- `ignore = [ref_a, ref_b]`

But the name-based form is the likely v1 authoring shape.

Future direction:

- support full MMS queries for deeper target resolution
- keep component-reference forms as the strongest/least ambiguous option when available

#### Rust

In Rust, this may want different construction methods rather than overloading a single field type.

For example:

```rust
RouterComponent::new()
    .with_target_name("container")
    .with_ignored_names(["toolbar", "status"])
```

or:

```rust
RouterComponent::new()
    .with_target_component(container_id)
    .with_ignored_components([toolbar_id, status_id])
```

That split is probably cleaner than trying to force Rust to mirror MMS literal syntax exactly.

The important semantic requirement is:

- MMS should be able to use readable authored references
- Rust should be able to use direct `ComponentId` references when already available

---

## 8. Router policy models

There are at least three plausible models.

### Model A — trait-only

- certain components implement a child-routing trait
- those component types always own routing behavior

Pros:

- simple for special-purpose components like `Scrolling`

Cons:

- not universal enough
- makes routing feel like niche special-case behavior
- awkward if arbitrary authored components should route children too

### Model B — explicit `RouterComponent`

- any component can become a topology owner by having a `RouterComponent` policy attached
- router behavior is driven by authored config and/or system defaults

Pros:

- very explicit in MMS
- general-purpose
- feels closer to author-controlled composition

Cons:

- slightly more verbose than implicit behavior

### Model C — universal implicit router with optional override/off

- every component has routing capability by default
- router policy may be implicit, explicit, or disabled with `Router.off()`

Pros:

- conceptually powerful
- future-proof for templates/component composition

Cons:

- more abstract
- can be harder to reason about if too much is implicit

### Recommendation

For v1 spec and eventual implementation, prefer a hybrid:

- **conceptually universal capability**
- **practically surfaced through explicit `RouterComponent` policy**
- **default behavior = passthrough when no router policy exists**

That gives us the universality you want without forcing every ordinary component to carry explicit
router boilerplate.

`Router.off()` and implicit component-type-owned routers should be treated as a v2 extension to this
model, not a v1 requirement.

---

## 9. What `RouterSystem` should do

The router system should be responsible for:

1. finding topology owners with routing policy
2. ensuring required helper topology/targets exist if needed
3. detecting external children under those owners
4. deciding where those children should go
5. reparenting them into the target location
6. avoiding loops or self-routing of helper nodes

This is basically a topology-maintenance system.

### 9.0 Router discovery rule

For v1, router discovery should be deliberately simple:

- when a component initializes, check whether it has a direct/immediate `Router` child
- if it does, that component becomes a router-enabled topology owner
- if it has multiple direct `Router` children, behavior is otherwise undefined today, but v1 should
    use the **first direct `Router` child found**

So for sibling routers:

```text
Owner
├── Router { ... }
├── Router { ... }
└── content...
```

the v1 rule is:

- first router wins
- later sibling routers are ignored

This should be explicitly documented as a temporary/compatibility rule rather than ideal final semantics.

### 9.1 Init-time behavior

At init, the router system should be able to:

- inspect direct children under the topology owner
- first check whether one of those direct children is a `Router`
- distinguish router/helper nodes from external authored content
- route external children into the target slot(s)

This matches the shape that worked for `Scrolling`.

More concretely, init-time flow should look like:

1. component initializes
2. inspect its immediate children
3. if no immediate `Router` child exists, do nothing
4. if an immediate `Router` child exists, resolve its `target` / `ignore`
5. route the rest of the children according to that router policy

### 9.2 Attach-time behavior

When a child is attached later under the owner, the router system should run again.

This is required for:

- runtime attach/clone
- editor insertions
- loops / generated children that appear after initial setup
- future UI composition systems

Without this, the concept is only an init trick, not a real topology abstraction.

More specifically:

- when a component is attached to an **initialized parent**
- and that parent has a direct `Router` child
- the newly attached component should be delegated to `RouterSystem` and routed immediately

So attach-time behavior must mirror init-time behavior, not diverge from it.

### 9.3 ParentChanged as the likely trigger

A strong candidate architecture is:

- `IntentExecutor::Attach` performs the normal attach
- `ParentChanged` is emitted as usual
- `RouterSystem` observes `ParentChanged`
- if the new parent has a direct `Router` child, it rehomes the external child according to that policy

That keeps the topology mutation layer consistent with the existing signal-driven engine design.

---

## 10. Classifying children

The biggest design question is how router-enabled owners distinguish:

- external children that should be routed
- internal helper nodes owned by the router/component itself
- internal target nodes like `container`
- style/prop/operator nodes that should remain on the owner root

A v1 classification model could be:

1. router policy component itself stays on owner root
2. target node(s) named in router policy stay on owner root
3. nodes named in `ignore` stay on owner root
4. runtime helper nodes with reserved names stay on owner root
5. everything else attached directly under the owner counts as external routed content

This is intentionally simple and should work for the `Router { target = "container" }` case.

### 10.1 Input-router exemption rule

Router-owned topology also needs a clear interaction rule.

Recommended rule:

- the `Router` policy node itself is exempt from input routing
- the `target` node(s) are exempt from input routing
- all nodes listed in `ignore` are exempt from input routing

This should be treated as a router-system-owned exclusion set.

The reason is simple:

- these nodes are part of structural/internal routing policy
- they should not be mistaken for ordinary externally routed content by any future input-routing
    layer

In short:

> router policy, router targets, and router ignore-list nodes are structural and should be invisible
> to input routing.

That keeps topology routing and input routing from fighting over the same internal nodes.

---

## 11. Reference `router.mms` example

We should define a reference authored scene before implementation.

The important rule from your workflow is:

- the UI topology should be authored in `router.mms`
- bootstrap Rust can provide missing pointer/gesture wiring for now
- the reference DSL scene should drive the runtime design backward

### 11.1 Proposed authored shape

A good first reference scene is:

```text
T {
    LayoutRoot {
        name = "app-ui"

        Router {
            target = "container"
            ignore = ["toolbar", "status"]
        }

        T {
            name = "toolbar"
            // toolbar visuals / labels
        }

        T {
            name = "container"
            // content region visuals
        }

        T {
            name = "status"
            // status text region
        }
    }
}
```

Then content added “to the app-ui root” should be routed into `container`.

At the same time:

- `toolbar` should remain exempt from routing
- `status` should remain exempt from routing
- `container` itself should be treated as the routing target, not as routed content
- the router node, target node, and ignore-list nodes should all be exempt from any future input-router layer

### 11.2 Suggested interaction demo

The demo should include cube-like buttons that add content into the routed container:

- add `Text`
- add `Renderable cube`
- maybe add nested `T { Text { ... } }`

The visual goal is to prove:

- author attaches/targets content at the owner
- runtime routes it into the designated content region
- outside authoring does not need to know the final internal topology

### 11.3 Bootstrap split

Until MMS can author the needed pointer/gesture interactions comfortably, the example can be split as:

- `router.mms` = explicit authored UI topology and router policy
- `router.rs` = Rust bootstrapper that wires clicks/actions and programmatically adds content

That keeps the declarative topology as the source of truth while acknowledging current authoring limits.

---

## 12. Relation to `Scrolling`

`Scrolling` can be viewed as the first specialized router-owned topology pattern:

```text
Scrolling
└── __scroll_track
    └── content...
```

Under this broader model, `Scrolling` is no longer a weird one-off.
It becomes one example of a topology owner whose router policy says:

- ensure `__scroll_track`
- route external children there
- move the track during scrolling

That is a strong sign that a general router system is the right abstraction.

---

## 13. Open questions

### 13.1 Should every component literally have a router by default?

Conceptually: yes, in the sense that every component can participate in the child-routing model.

Runtime/API-wise: probably no, if that means materializing a real `RouterComponent` on every node.

Better interpretation:

- every component can be treated as router-capable
- default routing is passthrough
- non-trivial routing is opt-in via explicit router policy

Possible v2 extension:

- certain component types gain implicit/internal routers
- `Router.off()` can explicitly disable that internal router at an authored site

### 13.2 How far can this go toward templates?

Potentially quite far.

If we later add:

- named child roles
- multiple targets/slots
- clone/spawn semantics
- parameterized authored subtrees

then this could become a foundation for templates or function-like components.

But we do not need to solve that now.

### 13.3 Do we need named slots immediately?

Probably not for v1.

A single target field like:

```text
Router { target = "container" }
```

is enough to prove the core routing behavior.

### 13.4 Should router policy live on the owner or as a child?

For MMS readability, a child policy component is appealing:

```text
LayoutRoot {
    Router { target = "container" }
}
```

That mirrors how many other local policies are authored in this engine.

It also keeps the owner component type generic.

---

## 14. Recommended staged spec path

### Phase 1

- define this router-system draft as the primary spec for generalized child routing
- keep the v1 goal narrow: one target, simple ignore list, passthrough by default

### Phase 2

- define a minimal `RouterComponent` v1 shape
- target one internal named node
- route all external direct children to that node

### Phase 3

- define the reference `router.mms` + `router.rs` example split
- use that example to pin runtime semantics before code work

### Phase 4

- decide whether the eventual runtime surface should be:
  - explicit `RouterComponent` only
  - trait/capability only
  - or the hybrid model recommended above

---

## 15. Tentative recommendation

Short version:

- yes, this should grow into a generalized `RouterSystem`
- yes, it should feel like any component can route children
- no, we probably should not require a literal router instance on every ordinary component
- the clean first spec is: universal capability, explicit router policy, passthrough by default
- `router.mms` should be the reference authored shape, with `router.rs` providing temporary bootstrap interaction code if needed

If this works, it becomes a clean foundation for:

- `Scrolling`
- UI content regions
- panels/widgets
- eventually templates / function-like component composition
