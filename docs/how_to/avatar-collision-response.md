# Avatar collision response

`AVC.collision_response("slide" | "read_only")` chooses whether the collision
solver may correct the avatar's movement. Collision participation stays enabled.
Configure it as a constructor/builder chain or inside the avatar body:

```mms parse-only
AVC.collision_response("read_only") {}
```

```mms parse-only
AVC {
    collision_response("read_only")
}
```

The default is `slide`, preserving existing local locomotion response. The
multiplayer example selects `read_only` for its remote copy on each computer,
independently of host/client role and XR/desktop input mode.

| Mode | Generated capsule | Contact queries | Collision correction of root/velocity | Foot/arm IK |
| --- | --- | --- | --- | --- |
| `slide` | Enabled when geometry/tracking are ready | Enabled | Permitted with a valid movement target | Enabled according to existing solver ownership |
| `read_only` | Enabled when geometry/tracking are ready | Enabled | Never applied to this body | Enabled according to existing solver ownership |
| Existing `collision_disabled()` | Automatic capsule disabled | No automatic capsule participation | No automatic capsule response | Existing separate visual-rig support fallback |

`read_only` does not undo an explicit `collision_disabled()`. It is a collision
response permission for the generated AVC capsule, not a blanket override of
other manually authored colliders and not a general transform lock: input, MMS/typed bindings and
visual IK keep their own write authority. Geometry and query self-filtering are
independent of the response target. Read-only capsules can become ready even
when their response target is missing; correcting capsules fail closed if a
required target does not resolve.

For manually authored geometry the equivalent is:

```mms parse-only
Zone.capsule_y(0.2, 0.8) {
    Collidable.slide().response("read_only") {}
}
```

`Collidable.slide()` retains its existing name and moving-zone identity; the new
`response` setting overrides correction permission. Static scenery remains
`Collidable.static()`; assigning slide response to static geometry is an error.
Both settings serialize through MMS. Invalid response strings and missing
arguments return errors.

## Detection and observed contacts

Read-only moving proxies participate in the shared retained collision index.
Transform changes refit their bounds rather than creating a per-avatar tree or
rebuilding the index on every movement. They can be detected as obstacles by
ordinary correcting movers. Body-part/self filtering excludes their own geometry.
In this first slice foot IK excludes those proxies from ground-support selection.
That does not disable capsule/capsule blocking in the movement response pass;
configurable blocking/support filters remain future work.

Native Rust callers can inspect `StaticContactSystem::contacts()` for read-only
blocking overlap/sweep observations from the latest contact pass. Observations
identify the two colliders/zones and a hypothetical world-space separation vector;
that vector was not applied to the read-only participant. The buffer is reused.
A correcting mover encountering a read-only proxy also records an observation
for that proxy. Ordinary slide/slide body response remains unchanged.

These are observations encountered during the pass, not a persistent contact
manifold or enter/stay/exit event stream. Multiple correcting iterations may
produce observations of the same pair. Resting ground is handled by support
queries. There are no new MMS contact signals in this slice. General relative-motion
CCD between moving bodies and configurable avatar blocking/support masks remain
future work; existing zone/triangle narrow-phase shape limitations still apply.

## Scope of the implementation

The remote Mitten now has an enabled read-only capsule and uses the existing
capsule-based ground-support path. Its root/head/hand/blink data still comes
through typed channels and its IK runs locally. The special collision-disabled
support cache remains for explicitly opted-out rigs; it was not redesigned here.

The [broader proposal](../draft/avatar-collision-response-and-ground-support.md)
retains the planned unified bounded ground-support query, improved readiness
states, moving-index choices and richer contact APIs. Those are separate from
this implemented response builder.

Validation covers both builder forms and serialization, invalid arguments,
read-only overlaps and swept contacts without position/velocity/friction changes,
missing response targets, independent self-filtering, retained proxy refits,
response switches and the two-peer Mitten example's enabled remote capsule.

## Scene profile caveat

A profiled run of `multiplayer_basic_example_runs_as_host_and_client` passed with
both actual Mitten rigs and enabled remote capsules. Its contact diagnostics saw
six query surfaces and two movers per scene after initialization; rebuild counts
stayed stable in the recorded windows. The dedicated moving-proxy test verifies
that changing a read-only proxy's position refits rather than rebuilds the index.

Rei reported using the workstation during the run. Treat all timing output as
exploratory and unsuitable for speedup/headroom claims. This was a debug headless
scene check, not a controlled frame-time or four-user performance benchmark.
The functional assertions and structural refit checks remain valid. Repeat timing
measurements in a quiet window only if a performance comparison is needed.
