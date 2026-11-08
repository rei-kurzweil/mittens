/*
# Host-filtered bespoke MMS event handlers

Status: working specification / design investigation; NOT implemented.
Created: 2026-11-07. This is the primary document for revising this proposal.
This .mms file contains a commented specification, not an executable example.

## Confirmed direction and independent work

Typed table/record VALUES with meaningful retained type identity are a required
language direction, not merely an optional spelling experiment. A host must be
able to distinguish certain value kinds by their declared type, not only by
inspecting field names and values. The exact representation and construction
rules below remain open; this decision does not mean they are implemented.

Do not make the current vehicle/settings CPU investigation wait for that type
work. Profile and optimize callback execution with existing scripts first; any
unnecessary per-invocation work affects both the vehicle and the settings panels.
Keep native event matching/selector storage separable from source construction,
so its Rust-side tests and implementation can proceed independently as well.

The typed second-argument selector remains the intended authoring contract. Do
not add a temporary third options argument, infer typed identity from an ad-hoc
kind/__type field, or silently change broad DataEvent subscriptions into narrow
ones to obtain a performance result. Existing broad listeners remain functional
until migrated explicitly. Runtime optimization and event-selection correctness
are separate acceptance checks.

## Purpose

Describe a handler's event selection when registering its binding. The native
host must reject unrelated events before constructing a scripting payload or
queueing an MMS callback. Do not enter MMS just to discover that an event does
not match an `if` at the start of its handler.

The immediate case is the ship's graphics/XR settings panels. They listen to
every global DataEvent, then compare its name inside MMS. Driving publishes
VelocityChanged events which wake both panels despite not changing settings.
A diagnostic bypass of those listeners saved about 3 ms of CPU update/frame
in the pre-optimization runtime. Sharing immutable function bodies subsequently
reduced their cost independently; the CPU report retains both measurements.
The contract still needs specific event selection. Do not count the historical
3 ms saving again as the current expected gain from filtering.
The vehicle's own frame callback has a separate cost; this proposal does not
claim to eliminate it or explain the observed growth in callback time.

## Preferred authoring form

    let binding = on(world, DataEvent {
        name = "GraphicsSettingsChanged"
    }, fn(event) {
        refresh_graphics_controls()
    })

The public shape is on(scope, selector, handler). The selector occupies the
SECOND parameter; do not insert an options parameter between it and the
handler. DataEvent should be a typed table/record/descriptor value, not a
string naming an event kind and not a scene component.

DataEvent's resolved type identity selects the host event kind. The `name`
field is a constraint on the emitted DataEvent.name. It is NOT the old optional
handler name, a debug label, an owner name, or the observed component's name.
"GraphicsSettingsChanged" remains an ordinary string constraint; avoiding a
string for the event KIND does not require eliminating string-valued event data.

The same selector must work with a component/subtree scope:

    let binding = on(panel_root, DataEvent {
        name = "AccordionRestoreRequested"
    }, fn(event) {
        restore_panel_contents()
    })

For the legacy on_global form, if retained during migration, its selector is
the first parameter because world is implicit:

    on_global(DataEvent {
        name = "GraphicsSettingsChanged"
    }, fn(event) {
        refresh_graphics_controls()
    })

The on(world, ...) form is preferred. This proposal does not reintroduce a
separate global subscription model.

## Compatibility and catch-all observation

Current string selectors may remain compatibility shorthand:

    on(world, "FrameTick", fn(event) { advance(event.dt_sec) })
    on(world, "DataEvent", fn(event) { inspect_data_event(event) })

The second example deliberately receives ALL DataEvents. Catch-all observation
must remain possible for diagnostics and other intentional broad observers.

Open decision: should the typed catch-all be DataEvent {}, or require an
explicit wildcard form? Settle this before implementing construction/default
rules. In particular, omission, null, an empty string and a mistyped field must
not accidentally become interchangeable ways to request every event.

The earlier untyped table idea, { kind = "DataEvent", name = "..." }, is no
longer the preferred surface. Whether to accept it as a transitional form is
open; it must not become a second source of event-kind identity. Likewise, the
earlier extra options argument is superseded, not an additional overload to add.

## Investigation: what exists today

1. Primitive type identities are first-class values. TypeTag currently includes
   f32, vec2, vec3, vec4 and str. Import aliases preserve those identities.

2. Type-only records, such as { name: str }, evaluate to immutable RecordType
   values. They describe field types and are structurally identified; they are
   distinct from ordinary initialized value tables such as { name = "hello" }.

3. Runtime Value and host TransportValue preserve TypeTag and RecordType.
   Ordinary tables cross the boundary as value snapshots. A table snapshot
   currently carries no nominal DataEvent instance identity.

4. MaterialDefinition.inputs consumes a RecordType and validates its fields;
   material GPU reflection determines uniform offsets. Material.inputs consumes
   initialized values separately. This is a useful type-aware host-boundary
   precedent, NOT an existing implementation of tagged record instances.

5. Named structs and typed record VALUES retaining declaration identity remain
   proposed in the type-system drafts. RecordType({ name: str }) alone cannot
   distinguish a DataEvent descriptor from another same-shaped nominal type.

6. In the current parser, a recognized component name followed by a brace body
   follows component-expression parsing. Registering DataEvent as a fake ECS
   component to obtain this syntax would create the wrong semantics. The
   preferred form needs data-construction resolution separate from component
   construction; an unregistered name may currently fail parsing/resolution.

7. on/on_global currently require a string event kind. Their optional extra
   string is a handler label. RegisterSignalHandler and its by-name variant
   have scope/name/callback information but no native DataEvent-name predicate.

Implementation references (relative to this file):

- ../../src/types.rs                         TypeTag, structural RecordType
- ../../src/object.rs                        Value, heap object representation
- ../../src/parser.rs                        identifier/brace disambiguation
- ../../src/evaluator.rs                     type evaluation and value transport
- ../../src/evaluator_signals.rs             on registration and binding methods
- ../../src/host.rs                          registration requests/transport values
- ../../src/runtime.rs                       RuntimeSpec signal/catalog identities
- ../../../../src/scripting/configured_registry.rs  material schema consumption
- ../../../../src/scripting/host_signals.rs   native script callback registration
- ../../../../src/engine/ecs/signals/rx_world.rs     dispatch and bypass gates

## Proposed typed-value / host contract

The configured host declares the selector type and its accepted fields. MMS
resolves DataEvent to that declaration, validates the construction, and retains
the declaration identity with the value until host registration resolves it.
An import alias must retain the same identity. An unrelated same-spelled or
same-shaped type must not silently acquire DataEvent semantics.

Do not add each host's event types as language-owned primitive TypeTag variants.
The primitive enum is not a registry of Mittens-specific event types. Reuse the
direction of the named-type/catalog designs for host-provided data declarations.
Do not represent trusted type identity as a mutable __type or kind string field.

The initial slice may be a host-declared descriptor value with immutable type
identity and validated fields; it need not implement every planned struct,
annotation, inheritance, union, generic or memory-layout feature. Any narrow
slice must remain consistent with the broader nominal-type design.

The construction and registration must NOT spawn/register a scene component,
allocate a ComponentId, or install a per-event MMS predicate callback. There is
no GPU descriptor set, uniform buffer or reflected GPU layout involved here.
Reuse type identity, schema validation and boundary ownership principles rather
than the material renderer's descriptor implementation.

Two representation choices still need evaluation:

A. Preserve a generic tagged record through Value/TransportValue, letting the
   host consume an owned type identity plus field values.

B. Resolve the typed selector against the configured catalog during on(), then
   send a normalized selector identity and field constraints in the registration
   request. A general-purpose typed-record transport may not be necessary for
   this first slice, provided nominal identity is not replaced by guessed text.

The language should not hard-code Mittens' GraphicsSettingsChanged or XR names.
The host owns their meaning and validates supported event constraints. Whichever
representation is chosen, string compatibility selectors and typed selectors
must converge on the SAME native registration contract and binding storage.

Conceptually that contract contains:

- observed scope: world or a live component/subtree handle;
- configured selector/event-kind identity;
- validated constraints: initially exact DataEvent.name matching;
- callback identity and session/lifetime ownership;
- the resulting binding identity and its active/bypassed/removed state.

Normalize or copy selector values once at registration. Mutating a source table
afterward must not silently alter an active binding. Live filter mutation, if
ever wanted, needs a separate explicit contract. Do not hold a borrowed MMS heap
table across deferred native dispatch.

## Native matching and callback delivery

For each event, delivery requires ALL of:

    binding is live and not bypassed
    event kind matches
    world/component scope matches
    declared field constraints match

For the preferred example, name matching is exact and case-sensitive. An event
named VelocityChanged must never reach that MMS handler. GraphicsSettingsChanged
reaches it only when its normal scope/lifetime conditions also match.

Match before payload-to-script conversion, callback argument allocation and
callback queue insertion. A later script guard, or filtering only when servicing
an already-created MMS invocation, does not meet the performance objective.

Native storage should support narrow event-name buckets alongside an intentional
catch-all bucket. The first slice need not define arbitrary field predicates,
topic hierarchies, regexes, wildcard patterns or script-executed filters. Rust
handlers should be able to register the same selector without going through MMS.

Preserve existing scope propagation and deterministic handler ordering. Specify
how named and catch-all buckets interleave before changing storage. Do not
deliver one binding twice because several native indexing paths find it.

The selector's type is not automatically the type of the callback argument.
Current Mittens DataEvent callbacks receive the event-name string. Preserve that
payload shape during this change unless a separate payload contract is adopted.
The typed value above describes interest; it is not automatically an event emit.

## Registration errors

Fail at registration for an unknown/unavailable selector type, unrecognized
field, wrong field value type, unsupported constraint, invalid scope, or foreign
callback/type identity. Do not silently register a broader listener.

Examples that must not become catch-all subscriptions:

    DataEvent { nmae = "GraphicsSettingsChanged" }  // typo
    DataEvent { name = 123 }                       // wrong type
    DataEvent { name = null }                      // no implicit wildcard

Empty-string rules and typed catch-all omission rules are still open. Preserve
source spans so construction/registration errors identify the offending field.
If callback allocation precedes a host registration error, release that callback
and leave no partially active native binding.

## Binding handles and observer-router migration

    binding.bypass(true)
    binding.bypass(false)
    binding.remove()

Selector matching and binding control are separate responsibilities. The
selector determines WHICH events qualify. Bypass determines WHETHER a live
binding receives them; removal disconnects it permanently.

Rust and MMS must control one authoritative bypass state. The current MMS-side
cached-state shortcut can skip a resume after Rust changes the native flag;
resolve that consistency problem before claiming mixed-language control works.
Define cancellation for already-queued deferred invocations too. Resume should
not replay events that occurred while bypassed or resurrect a removed binding.

A BindingId identifies the registration; a ComponentId may identify its observed
scope or lifetime owner. They are different identities. No additional ECS
component is required for each binding. Old/stale handles must not control a
later binding which happens to reuse storage.

All Rust registration paths should eventually return typed binding handles,
including native function pointers and closures, not just script callbacks.
Preserve idempotent installation where systems currently install handlers each
frame. Keep pause/removal checks cheap on the native dispatch path.

Handler labels may remain for diagnostics during migration, but must never be
interpreted as the DataEvent.name filter. Replacing observer-router string
blacklists/whitelists with retained binding control is a separate migration:
preserve per-editor/per-component scope and cleanup, and retire the old mechanism
only after its actual users have moved. Do not disable all editor instances by
replacing a scoped rule with a single global bypass bit.

## Decisions to settle before implementation

- Is DataEvent resolved as a host-registered nominal record type, a narrower
  descriptor type, or a later general struct declaration?
- How are selector-type bindings exposed/imported, and how does construction
  distinguish them from components, namespaces and user-defined same-name types?
- Does generic host transport gain tagged record values now, or does on()
  normalize them through catalog metadata into a registration-specific value?
- What is the explicit typed catch-all spelling and its missing-field behavior?
- Are untyped selector tables supported at all during transition?
- Does the selector use the eventual event payload's nominal type identity or a
  distinct selector role associated with that event kind? Do not conflate the
  present descriptor example with a commitment to typed callback payloads.
- What ordering is guaranteed between catch-all and exact-name handlers?
- How do bypass/removal and deferred cancellation share authority across Rust
  and MMS, including changes made during an in-progress dispatch?
- What compatibility/deprecation path applies to event-kind strings, optional
  handler labels, on_global, and observer routers? Avoid bundling their removal
  into the first settings-event performance fix.

## Suggested implementation slices / acceptance checks

1. Freeze parse/resolve/value examples and expected errors for the preferred
   typed selector, aligned with the existing nominal-record drafts. This spec
   owns the event-specific behavior; the type drafts own general type semantics.

2. Add the minimum identity-preserving selector construction and host contract.
   Verify import aliases, type/field validation, snapshots at registration, and
   that constructing a selector creates no scene nodes or ComponentIds.

3. Implement native kind/scope/name selection before MMS payload/queue work.
   Verify exact matches, unrelated names, intentional catch-all, scoped events,
   ordering, one delivery per binding and registration-failure cleanup. Exercise
   direct Rust handlers and MMS callbacks against the same matching rules.

4. Migrate the graphics/XR settings panels to narrow selectors. Verify genuine
   settings changes still update all appropriate panels; VelocityChanged queues
   ZERO settings-panel callbacks. Keep the same vehicle controls, physics, scene
   geometry, mirror state and route when benchmarking the CPU difference.

5. Unify handle control and migrate observer-router users in reviewable slices.
   Test native bypass followed by MMS resume, the reverse direction, queued
   cancellation, source removal, stale handles, idempotent setup and independent
   scoped editor instances. Keep this separate from vehicle controller redesign.

6. Preserve historical benchmark captures and record the new CPU/frame results.
   The remaining vehicle callback/runtime cost and temporal growth still require
   their own investigation; do not call this a complete 45 FPS solution.

## Related documents

- ../draft/record-types-and-values.md
- ../draft/typed-declarations-and-functions.md
- ../draft/type-system-epic.md
- ../draft/runtime-spec-host-binding-model.md
- ../draft/host-values-resources-and-bound-receivers.md
- ../../../../docs/task/ship-driving-cpu-profile.md

No runtime, parser, host API, routing behavior or performance fix is implemented
by adding this specification. Revise this file as the primary proposal rather
than keeping incompatible copies in the ship profiling notes.
*/
