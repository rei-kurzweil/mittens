# MMS host and component registration module split

The MMS host dispatch, runtime configuration, and component registry combine
unrelated API families in large files. Move component configuration and
application for GLTF and other cohesive families into their own modules.
Keep service registration and dispatch with each service, following the XR
settings API. Keep the central files responsible for wiring the modules.

Preserve direct operation-ID dispatch and component application without extra
lookups or allocations. Keep settings services idle between edits. Validate
RuntimeSpec component binding, scene serialization, settings callbacks, and
avatar imports before and after any split.

Keep `Acceleration` builders and vehicle methods together as the motion
vocabulary grows. Validate projectile acceleration, vehicle throttle release,
and explicit braking without adding a second script step.
