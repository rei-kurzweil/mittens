# MMS fields on individual component instances

Date: 2026-10-26
Status: agreed direction for the next slice; documentation/example only.
Direct component field assignment is not implemented yet.

See [the proposed example](../../examples/proposals/component-instance-methods.mms).
The working Sakura scene still returns a table from each can factory and loops
over those tables to route hits. This document describes the next change, not
behavior already available in the interpreter.

## Small first slice: fields holding any MMS value

Allow adding a named field directly to a live component object:

```mms
can.label = "Lime can"
can.settings = { damage_scale = 1.0 }
can.on_hit = handle_hit

let label = can.label
can.settings.damage_scale = 0.5
can.on_hit(hit)
```

A field may hold any MMS value, including a function, number, boolean, string,
list, table, component reference, type value or null. The value stays an MMS
value; it is not forced through JSON or narrowed to an engine property type.
Function-valued fields use normal call syntax and explicit arguments, with the
same lexical captures as ordinary functions. No implicit `this`/`self` is added.

The example uses existing `fn` declarations/function expressions followed by
plain member assignment. There is no new method-declaration grammar in this
slice, and no `methods({...})` registration step or special `.methods`/`.data`
namespace. A function-valued field is sufficient to provide a method-like API.

## What is superseded or deferred

Earlier drafts explored a separate method registry and then `.data` as a live
view of a retained component-block scope. Those were tentative designs and are
superseded for the first slice by direct fields.

The shorthand below is **deferred**, including typed parameter/return syntax
and the proposed `void` spelling:

```mms
T {
    do_thing(and_how: f32): void { /* ... */ }
}
```

This slice does not expose component-block locals automatically. Existing local
scope and closure-capture rules stay in place. A factory can retain private
state and descendants, publish selected values/functions on its root, and
return that component as its public interface. Retaining every lexical binding
or adding shared scalar binding cells is not a prerequisite for this design.

Earlier [component reflection plans](mms-component-reflection-and-table-dot-access.md)
also discuss native authored-property inspection. That is separate from adding
new script fields: assigning a script value is not implicitly an engine setter.

## Identity, reads and calls

- Fields belong to the live component identity, not its reference wrapper,
  display name, component type or parent. Query/event aliases see the same
  additions and changes; independent factory instances remain independent.
- Reading a missing script field returns null, permitting optional callbacks.
  Calling a missing/non-function field is an error. A component may also hold
  ordinary non-function fields; field assignment itself does not demand a
  callable value.
- Table/list/function values retain ordinary MMS identity and capture behavior.
  Returning or reading a field must not deep-copy its contents or its closure.
- `can.on_hit(hit)` invokes the field's function synchronously and returns its
  result. Engine mutations inside it retain their existing host/intent timing.
- Assigning `can.on_hit` does not subscribe to an engine event. Call sites or
  explicit event adapters choose when to invoke it.
- Direct calls/read access are local to that component. No implicit lookup of
  parent fields, ancestor methods or unrelated objects is introduced.

Native members need a collision rule before implementation. The recommended
first-slice policy is to reject attempts to overwrite registered native methods
or properties, preserving existing RuntimeSpec validation and native dispatch.
Replacing an existing script field is ordinary reassignment. Native property
reflection/mutation and optional bracket access should be reviewed separately;
do not silently redesign the whole native component API to add script fields.

## Concrete can migration

The current factory already has `root`, `state` and a `handle_hit(hit)` closure
that recognizes its body/lids and plays feedback. Its first migration can be:

```mms
root.on_hit = handle_hit
root.hit_count = fn() { return state.hits }
return root
```

The caller would receive a component directly, rather than a separate
`{ root, state, handle_hit }` table. For the smallest behavioral change, keep
the existing six-can loop but invoke `can.on_hit(hit)` on those components.

Then, if we want direct dispatch from raycast geometry without inventing an
ancestor API, publish the same handler explicitly on each participating primitive:

```mms
body.on_hit = handle_hit
top.on_hit = handle_hit
bottom.on_hit = handle_hit
```

The weapon/scene callback becomes:

```mms
let on_hit = hit.renderable.on_hit
if on_hit != null { on_hit(hit) }
```

These references all invoke the same captured target state. The callback still
receives the exact hit primitive, point and source tool/user. The closure is not
rebound to whichever primitive happens to carry it. This supports an opaque root
API and explicit forwarding on private hit surfaces without automatic scope
inheritance. Decide whether to do this forwarding in the first can migration or
retain the list temporarily; neither is implemented by this document.

Generic ancestor-owner discovery, a Damageable component and destruction remain
separate later decisions. None is needed to demonstrate component fields.

## Runtime/lifecycle decisions before implementation

- Keep script values and closures in the MMS runtime/session. Any host-side
  route carries opaque identity and lifecycle metadata, following the
  [host/runtime boundary](../meow_meow/spec/mittens-host-and-runtime-boundary.md).
  Avoid a second interpreter or conversion to engine JSON/property storage.
- Define removal/session cleanup, foreign/stale component errors and extracted
  callback behavior. Reusing an engine component slot must not inherit its old
  fields. Consider aliases and captures when releasing the per-instance store.
- Decide behavior for cloning and cross-session access. Copying closures that
  capture original descendants is not equivalent to rerunning the factory.
- Preserve animation visual/audio phase rules through function-valued fields;
  audio lookahead must not duplicate gameplay side effects.
- Define persistence/hot-reload reconstruction separately. Arbitrary MMS values
  and closures are not automatically serialized native component properties.
- Missing-field reads, function replacement, callback argument errors and
  native-name collisions need explicit tests. Future typed interfaces may add
  constraints, but are not a dependency for the initial dynamic field surface.

Suggested sequence: implement/read/write/call per-instance fields; verify query
and event aliases, different instances and native collisions; migrate can roots;
optionally publish primitive forwarding fields; validate the existing hit
feedback in VR. Add shorthand method declarations only in a future slice.
