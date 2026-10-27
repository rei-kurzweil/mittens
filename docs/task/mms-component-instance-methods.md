# MMS fields on individual component instances

Date: 2026-10-26
Status: direct live-component fields implemented; VR acceptance pending.

See [the component-field example](../../examples/proposals/component-instance-methods.mms).
The Sakura factory now returns its root component. Its root, body and lids have
an `on_hit` function field; the scene calls the hit primitive's field directly.
The separate method-syntax and scope-inheritance ideas remain future work.

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

- Fields belong to the live component identity within the owning MMS session, not its reference wrapper,
  display name, component type or parent. Query/event aliases see the same
  additions and changes; independent factory instances remain independent.
- Reading a missing script field returns null, permitting optional callbacks.
  Calling a missing/non-function field is an error. A component may also hold
  ordinary non-function fields; field assignment itself does not demand a
  callable value.
- Values retain ordinary MMS copy/identity and capture behavior. In particular,
  table identity and a function's captured state are retained across field reads.
- `can.on_hit(hit)` invokes the field's function synchronously and returns its
  result. Engine mutations inside it retain their existing host/intent timing.
- Assigning `can.on_hit` does not subscribe to an engine event. Call sites or
  explicit event adapters choose when to invoke it.
- Direct calls/read access are local to that component. No implicit lookup of
  parent fields, ancestor methods or unrelated objects is introduced.

The first slice rejects attempts to overwrite registered native methods
or properties, preserving existing RuntimeSpec validation and native dispatch.
Replacing an existing script field is ordinary reassignment. Native property
reflection/mutation and optional bracket access should be reviewed separately;
do not silently redesign the whole native component API to add script fields.

## Concrete can migration

The factory keeps `state` and its descendant handles private. It publishes the
existing `handle_hit(hit)` function, which recognizes its body/lids and plays feedback:

```mms
root.on_hit = handle_hit
root.hit_count = fn() { return state.hits }
return root
```

The caller receives a component directly, replacing the earlier
`{ root, state, handle_hit }` table.

For direct dispatch from raycast geometry, the same function is assigned to each
participating primitive:

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
inheritance. The scene's list is now only used to construct/attach the cans;
it is not searched to dispatch hits. The controlled car uses the same optional
target-field call after playing its impact effect.

## Root event handlers are another valid construction pattern

A factory can register `on(root, "RayIntersected", fn(event) { ... })` and
capture private state/descendants in that handler. The current RayCastSystem
emits at the hit renderable; RxWorld dispatches scoped handlers from that
component up its unique parent chain. Body/lid hits can already reach a can's
root handler without inherited script fields.

Raw RayIntersected includes hand-pointer hits too and does not carry the full
projectile/tool/user context assembled by the laser callback. A root-handler
design therefore needs weapon-ray recognition or a separately routed weapon-hit
event. Do not simply subscribe to every ray intersection and treat it as damage.
This is an alternative routing design; the current can migration uses explicit
function fields. Component fields, event bubbling and lexical scope inheritance
are three separate mechanisms.

## Future note: opt-in ancestor-scope inheritance

Explore a modifier, analogous in role to visibility modifiers, which explicitly
allows descendants to resolve fields/functions from an ancestor's script scope.
No modifier spelling is selected and no implementation is part of this slice.
Review which members are inherited, reads versus writes, child overrides,
propagation boundaries, multiple-parent ambiguity and cleanup before choosing
semantics. Do not change lexical closure capture or infer scope inheritance just
because an event bubbles to an ancestor.

Generic ancestor-owner discovery, a Damageable component and destruction remain
separate later decisions. None is needed to demonstrate component fields.

## Runtime/lifecycle behavior and follow-ups

- Keep script values and closures in the MMS runtime/session. Any host-side
  route carries opaque identity and lifecycle metadata, following the
  [host/runtime boundary](../meow_meow/spec/mittens-host-and-runtime-boundary.md).
  Avoid a second interpreter or conversion to engine JSON/property storage.
- The session checks field receiver ownership/liveness and prunes fields of
  removed components at subsequent evaluation/callback boundaries. Closing the
  session releases its field store. Reused component generations do not inherit
  old fields. A function already extracted into a variable remains an ordinary
  function value; it is not a new owner-bound method-wrapper type.
- Decide behavior for cloning and cross-session access. Copying closures that
  capture original descendants is not equivalent to rerunning the factory.
- Preserve animation visual/audio phase rules through function-valued fields;
  audio lookahead must not duplicate gameplay side effects.
- Define persistence/hot-reload reconstruction separately. Arbitrary MMS values
  and closures are not automatically serialized native component properties.
- Missing-field reads, function replacement, callback argument errors and
  native-name collisions need explicit tests. Future typed interfaces may add
  constraints, but are not a dependency for the initial dynamic field surface.

The implementation uses crate-owned session storage and a generic batched
ValidateComponents host request; values/functions are not sent to engine
property storage. StandardHost, EventStreamHost and MittensHost implement the
liveness request. Cross-session field sharing, automatic field copying during
native tree cloning, persistence and bracket access are not added.

Language tests cover values/closures, aliases, independent instances, replacement,
native collisions and lifecycle checks; scene tests exercise real hit feedback.
Finish with VR acceptance. Shorthand methods, inherited scope and alternative
root-event routing remain future slices.
