# Task: concise MMS event streams and motion readiness policy

Date: 2026-10-06
Status: investigation only. No code, parser, runtime, or API changes authorized by this task.

## Motivation

Gravity activation now has the right prerequisite: AVC publishes `CapsuleReady`
after its generated contact capsule is usable. However, subscribing to a
generic DataEvent, checking its name, calling a setter, and separately handling
late readiness requires substantial boilerplate for a common scene policy.

Investigate concise wiring from an event stream to a method or value consumer:

```text
AVC DataEvent -> filter CapsuleReady -> trigger or true -> Gravity.set_enabled
```

The stream need not be an engine `Signal`. It can be an MMS value representing
a subscription/computation over observed engine events. Keep events, retained
values, directed engine intents, and method calls distinct in the model.

A second question is whether an avatar's outer world-motion layer should offer
a standard readiness policy. That layer can include transform wrappers, pose
drivers, Velocity, and Gravity/other velocity-driver wrappers. Explore this
alongside general stream syntax; do not hide avatar discovery in physics again.

## Existing behavior and earlier proposals

The current working API is:

```mms
on(avatar, "DataEvent", fn(event) {
    if event == "CapsuleReady" {
        gravity.set_enabled(true)
    }
})
if avatar.capsule_ready() {
    gravity.set_enabled(true)
}
```

Gravity is authored disabled. `DataEvent` callbacks currently receive the
event name, not a general typed event envelope. `capsule_ready()` is a getter,
not an observable. Subscription is implemented through retained callbacks.
Velocity no longer discovers avatars or traverses their subtrees for readiness.

Earlier design work directly overlaps this task:

- [MMS event handlers and signal wiring](../meow_meow/analysis/event-handlers.md)
  proposed `->` / `<-`, source and sink values, map stages, and one-shot handlers.
  Its descriptions of then-current MMS capabilities are historical; do not
  treat its proposed syntax as implemented.
- [MMS event signal pipelines](../meow_meow/draft/event-signal-pipelines.md)
  considered imperative listen/map/route operations and component-local event
  projection. It left declarative operators open.
- [Engine event signal pipelines](../draft/event-signal-pipelines.md) describes
  event projection/routing without replacing ancestor bubbling.
- [Event operators and coalescing](../analysis/event-signal-operators-and-coalescing.md)
  examines filter/map/coalesce and the boundary between observation and mutation.
- [Generic scalar signal providers](generic-scalar-signal-providers.md)
  explicitly separates retained sampled values from the engine Signal bus.
- [Velocity-driver readiness](velocity-driver-readiness-and-dependencies.md)
  compares readiness dependencies and provider activation, and documents the
  implemented CapsuleReady event.

Reuse these concepts and resolve conflicting assumptions rather than inventing
an unrelated reactive runtime.

## Types and meanings to compare

| Value | Meaning | Gravity example |
| --- | --- | --- |
| Event stream of names/payloads | Zero or more occurrences, usually no replay | AVC DataEvent names |
| Trigger/unit stream | An occurrence with no meaningful value; not an absence of events | A filtered CapsuleReady occurrence |
| Boolean stream | Each occurrence supplies true or false | Map readiness events into enabled values |
| Retained boolean / readiness value | A current value exists independently of subscribers | Avatar currently has a usable capsule |
| One-shot completion / promise-like value | A prerequisite completes once, with defined late-subscription behavior | Capsule has become ready for startup |
| Sink / method target | Receives a value or performs an explicitly supplied action | `gravity.set_enabled(enabled)` |

A trigger cannot supply the boolean argument to `set_enabled` by itself. Either
map it to `true`, or supply an action that ignores the payload and passes true.
Do not introduce implicit click-to-toggle, implicit true, or primary-event
coercion into an otherwise generic pipe operation without an explicit rule.

For an arbitrary MMS-value stream, establish type checking and conversion at
the sink. Event filtering and value mapping should not require a new native
EventSignal variant for each derived value.

## Candidate syntax

Everything in this section is illustrative and unimplemented. Compare syntax
and semantics before choosing names or extending the grammar.

### Method chains over first-class stream values

Boolean stream, explicit callback sink:

```mms
events(avatar, "DataEvent")
    .filter(fn(name) { return name == "CapsuleReady" })
    .take(1)
    .map_to(true)
    .subscribe(fn(enabled) { gravity.set_enabled(enabled) })
```

Trigger stream, explicit constant action:

```mms
events(avatar, "DataEvent")
    .named("CapsuleReady")
    .take(1)
    .subscribe(fn(_) { gravity.set_enabled(true) })
```

A `.named(...)` convenience could remove the generic name predicate without
making CapsuleReady special. Another possible sink is
`.pipe_to(gravity, "set_enabled")` after `.map_to(true)`. Compare its validation,
discoverability, and rename behavior with passing a bound method value.
Component methods are not assumed to be first-class callable values merely
because calling `gravity.set_enabled(true)` works today.

### Operator sugar over the same runtime

Preferred working draft as of 2026-10-07, subject to the contracts below:

```mms
events(avatar, "DataEvent").named("CapsuleReady").take(1).map_to(true)
    -> gravity.set_enabled
```

This would require stream/sink semantics and a bound-method contract, not just
an arrow token. Compare against chains for parser cost, debugging, explicit
subscription lifetime, and consistency with the earlier wiring proposal.

## Runtime stream values versus AST lowering

`events(...)` naturally reads as an expression returning a stream value. Treat
that as the preferred semantic direction: streams can be assigned, passed to
functions, returned by helpers, and composed independently of the eventual
subscriber. This does not require a particular Rust representation or a new
engine Signal type.

Conceptually this is `Stream<Value>`. In a dynamically typed MMS `Value` enum,
the implementation could instead be `Value::Stream(StreamHandle)` referencing
a session-owned stream description. The handle need not contain a live queue
of Value objects. Element types/signatures can be metadata used for validation;
the public language need not expose generic type syntax in the first slice.
Do not confuse the existing engine-owned compatibility Value with the
host-neutral scripting runtime's Value and transport/host-handle boundary.
Implementation investigation must choose where stream descriptions live and
how handles cross that boundary.

AST lowering and runtime stream values are compatible. Lowering describes
how syntax executes; a Value describes what an expression produces. Compare:

| Approach | Meaning | Tradeoff |
| --- | --- | --- |
| Syntax-only chain DSL | Recognize the whole expression and generate handler/filter/map bookkeeping directly. | Small for literal examples, but stored streams, helper returns and dynamically chosen sources require additional rules or become unsupported. |
| Runtime stream value | `events` produces a composable handle; each operator produces a derived description; a sink creates a subscription. | Clear expression semantics and ordinary function composition; adds stream values, lifecycle and operator dispatch to the runtime. |
| Stream values plus lowering | Lower chains to normal stream operations and lower the arrow to an explicit subscription/action. Optimize descriptions into existing handler machinery when attached. | Preserves first-class behavior while avoiding a separate heavyweight reactive scheduler; preferred direction to investigate. |

Even syntax-only lowering needs runtime state for `take(1)`, subscription
cleanup and captured references. It cannot make those requirements disappear.
Likewise, runtime streams do not require allocating a native handler at every
operator stage: a composed description can use one host event-source adapter
and evaluate its operators in the MMS runtime with per-subscription state.

### Preferred ownership boundary: MMS streams, host event producers

Working direction as of 2026-10-07: a stream is an observable-like, MMS-owned
runtime value. Constructing `events(...)` resembles constructing a component
expression in authoring style, but the resulting object is not necessarily an
ECS component and need not live in the scene's component tree.

The interpreter/session owns stream descriptions, operator execution,
subscriptions, retained values and completion state. The host supplies values
through the source adapter at normal event-delivery boundaries. Engine events
can feed a stream; a future other host could feed the same abstraction from a
different source without implementing the operators itself.

```text
engine event delivery
    -> host source adapter / delivery into the owning MMS session
    -> MMS-owned stream operators and optional history
    -> MMS callback or method sink
    -> host component method
```

"Operators live in MMS" means their semantics and state belong to the scripting
runtime. Built-in operator implementations can be Rust code inside the
interpreter; they need not be user-written MMS functions. Host delivery must
respect the session's execution/borrowing boundaries rather than arbitrarily
calling into the interpreter from an audio, XR or worker thread.

The host still owns native event production and scope routing. MMS does not
become the engine event bus. A host-source registration token can be associated
with an MMS stream without turning each `filter`, `map` or `take` into a native
engine handler/component. Closing the owning session must unregister adapters
and dispose stream state.

Keep immutable composition descriptions separate from mutable runtime delivery
state. Sending a value into a source advances its subscribers/operators; it
does not replace the stream object with that value. AST lowering remains an
implementation technique for wiring these MMS values, not a substitute for
their runtime semantics.

### Opt-in history and replay builders

Ordinary event streams remain future-only. If historical delivery is supported,
expose it as an explicit builder/operator option rather than replaying all
engine events implicitly. Illustrative syntax:

```mms
let ready = events(avatar, "DataEvent")
    .named("CapsuleReady")
    .replay(1)

ready.take(1).map_to(true) -> gravity.set_enabled
```

Names and behavior are not selected. Prefer bounded retention (one value is
enough for this example), optionally with a time window when there is a concrete
consumer. Replay delivers retained values to a new subscriber before subsequent
live values, in defined order, through the same downstream operators.

Distinguish three concerns:

1. **Capture:** when does this stream start receiving events from the host?
2. **Retention:** which received values does it keep, and for how long?
3. **Replay:** which retained values does a new subscriber receive?

An MMS replay buffer cannot recover events emitted before it started capturing.
A host source with its own retained history can explicitly support historical
delivery, but a replay builder cannot assume every host does. No historical
source capability exists merely because ordinary event subscriptions exist.
Unsupported source-history requests need a defined error or documented limit;
do not silently promise history that was never recorded.

The default lazy-construction model above must therefore be reconciled with
replay. Compare explicit connection/start of capture, eager capture for an
opt-in retaining stream, and a subscription to an already-retained host source.
If opting into replay also starts capture, document that side effect and its
lifetime. If capture remains lazy, `.replay(1)` only replays values observed
since activation, not all past engine events. Do not leave the choice implicit.

Replay state belongs to the retaining stream instance; `take(1)` counters belong
to each downstream subscription. Putting `.named("CapsuleReady")` before
retention avoids an unrelated later DataEvent evicting the readiness event.
Define whether disposing the last subscriber stops capture, clears history,
or leaves a session-owned connection active, and what happens when a source
component is removed or replaced.

Historical values need snapshot semantics: retaining a mutable table reference
may show its later contents rather than the value originally emitted. Component
handles in retained payloads can become stale. Resolve these representation and
lifetime rules before promising arbitrary `Stream<Value>` history.

Replay of CapsuleReady is still historical truth, not current readiness. An old
event can remain in the buffer after contact becomes unavailable. A startup
latch may deliberately use history; an ongoing readiness policy should consume
current state and transitions. Keep these as separate explicit author choices.

### Proposed expression semantics

The following is a design target, not current MMS behavior:

```mms
let ready_events = events(avatar, "DataEvent").named("CapsuleReady")
let enable_values = ready_events.take(1).map_to(true)
enable_values -> gravity.set_enabled
```

Each expression before the arrow describes a stream; merely constructing it
does not install a listener. Attaching a sink activates a subscription to the
underlying hot engine event source. Derived descriptions can share the source
identity without silently sharing subscriber counters: two sinks should each
have their own `take(1)` state unless multicast/share behavior is explicit.
Immutable descriptions are the initial candidate; validate this against
dynamic source changes and session ownership before selecting an implementation.

The arrow can lower to the conceptual equivalent of:

```mms
enable_values.subscribe(fn(enabled) { gravity.set_enabled(enabled) })
```

It creates wiring rather than invoking `set_enabled` at construction time.
Decide whether arrow wiring returns a subscription handle or is a statement
owned by the enclosing scene/component. Explicit disposal and automatic owner
cleanup must agree whichever syntax is used.

`gravity.set_enabled` in the arrow's sink position need not immediately imply
general first-class method references everywhere in MMS. Two viable first
slices are a bound callable value, or sink-specific lowering that captures the
receiver once and generates a validated method call. Document any restriction
on storing/passing sinks rather than making the syntax look more general than
it is. Ordinary existing calls still evaluate immediately.

Prefer treating `named`, `take`, and `map_to` as stream operations with normal
runtime implementations. Parser sugar should not encode CapsuleReady or
Gravity behavior. AST optimizations may fuse a chain without changing its
observable ordering, validation, subscription lifetime or error behavior.

### What this syntax does not solve automatically

The preferred chain observes future matching events, takes the first one,
and supplies true to the setter. `map_to(true)` makes the trigger-to-boolean
conversion explicit. It does not replay a CapsuleReady event that already
happened and does not provide a continuous readiness binding.

The first stream slice must retain a separate initial-readiness mechanism or
provide an explicitly stateful readiness source. Do not silently reinterpret
`events(...)` as replaying state to eliminate the getter boilerplate. Compare
a concise ready/completion source with explicit initial sampling as part of
the readiness investigation below.

### Reusable readiness helper or provider policy

```mms
enable_gravity_when_capsule_ready(gravity, avatar)
```

This could package today's callback and getter without changing MMS syntax.
Alternatively, a declarative provider policy might read:

```mms
Gravity.enable_when(ready(avatar)) { motion }
```

Both sketches must say whether they enable once or continuously follow a
readiness value. A helper is a useful baseline: general stream machinery should
demonstrably improve more than the length of this one snippet.

## Startup latch versus ongoing binding

For startup, CapsuleReady currently only signals a ready transition, while the
getter provides current state. A filtered hot event stream with `.take(1)` can
still miss an event that occurred before subscription. Syntax sugar must retain
the existing late-subscriber behavior.

Compare a readiness source combining a getter and transition subscription,
an explicit initial sample, and a replayable one-shot completion. Define
registration order so sampling and subscribing cannot miss a transition or
deliver it twice. Do not silently make all DataEvents replayable.

For continuous binding, a single ready event is insufficient: readiness loss,
component removal, reload, and recovery need state transitions. Also decide
whether tracking loss should affect gravity at all; losing a headset sample is
different from losing contact geometry.

Promise.all-like startup completion means all prerequisites have completed
once. A combine-latest/all-ready value means all are currently ready and may
become false again. These are different policies. Nested subscriptions can
miss an earlier event and should not be the recommended all-ready pattern.

For this first consumer, preserve the startup latch: no accumulated gravity
while waiting, normal integration of existing velocity, and no automatic
clearing of velocity when enabled. Reset and contact remain independent.

## Motion-layer policy and topology

Within `Gravity { ... }`, identify which readiness producer controls activation
without treating every descendant as a prerequisite. Gravity already reaches
the first Velocity on each branch; a provider may therefore affect several
independent movers. One avatar becoming ready must not accidentally release
gravity for all others that are still loading.

Compare:

| Policy location | Advantages | Questions |
| --- | --- | --- |
| Explicit source-to-provider wire | Simple, reusable; no implicit discovery | How to group several sources or choose one source per mover? |
| Provider `.enable_when(source)` policy | Keeps the condition beside Gravity | Is eligibility provider-wide or per driven Velocity? |
| Per-mover world-motion policy | Can coordinate pose, velocity and animation wrappers | Which motion channels actually wait? Who owns the policy? |
| Reusable avatar motion wrapper/factory | Hides common wiring with ordinary composition | Can callers override drivers, readiness source and lifecycle policy? |
| Inferred producer within provider subtree | Minimal authoring | Ambiguity with several avatars, decorative models and nested Velocity; topology invalidation and observability cost |

Favor explicit producer identity or a validated scoped policy over “first
descendant to emit CapsuleReady.” A ComponentRef may identify the source,
but its existence alone is not readiness. References should resolve and
invalidate outside physics substeps.

Do not block tracked head/hand pose input just because world gravity is pending.
Likewise, an animation pose driver and a falling Velocity layer have different
readiness requirements. If a motion policy groups channels, the author must
be able to understand which channels it gates.

## Runtime contract to investigate

- Define stream creation as cold or hot, and whether construction alone
  subscribes. Define which object/component owns subscriptions and how removal,
  scene replacement, detach and cloning dispose or rebind them.
- Preserve existing scoped event delivery. A subscription on AVC can observe
  bubbling descendant events; decide whether the stream supports exact-source
  filtering as well as subtree observation. DataEvent name alone is not a
  globally unique producer identity.
- Establish how native event envelopes become MMS values. Keep event source,
  optional component payload and name available if the stream API needs them;
  avoid silently changing the existing callback's string argument.
- Keep event dispatch, MMS callback evaluation, setters and directed mutation
  ordering consistent with current drain points. A pipe must not secretly run
  arbitrary script during physics substeps or introduce physics-time discovery.
- Define retained closures, stale handles, subscriber errors and disposal.
  Promises/readiness may need failed/cancelled states rather than waiting forever.
- Serialize authored wiring descriptions if necessary, not live callbacks,
  promise state, subscriptions or generated capsule component identities.
- Make initial values and readiness transitions observable in diagnostics.
  Operators should not allocate or execute when nobody consumes their output.
- If exposing property sinks later (`gravity.enabled`), route them through a
  supported setter contract. Direct component field assignment is not currently
  an implemented substitute for `set_enabled`.

## Investigation deliverables

1. Reconcile the earlier MMS wiring drafts with current retained callbacks,
   runtime component methods and the generic DataEvent payload contract.
2. Prototype authoring sketches for CapsuleReady-to-Gravity, a boolean UI
   stream-to-setter, and startup completion over two independent prerequisites.
   Keep sketches explicitly separate from working MMS.
3. Compare ordinary helper functions, chain APIs, operator sugar and provider
   policies by boilerplate, required runtime machinery and lifecycle clarity.
4. Choose whether a stream value, readiness value and completion value should
   be distinct types or related interfaces; specify conversions and sinks.
5. Define multi-mover provider behavior and subscription ownership before
   implementing any topology inference.
6. Propose the smallest useful implementation task, including tests for late
   subscribers, arrival order, duplicate events, unrelated DataEvents, multiple
   avatars, disposal/reload and mutation ordering.

No implementation or example migration belongs to this investigation task.
The existing CapsuleReady callback and readiness getter remain the reference
behavior that any sugar or policy must preserve.
