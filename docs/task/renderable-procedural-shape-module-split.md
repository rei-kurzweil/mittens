# Separate procedural shape construction from renderable lifecycle

`src/engine/ecs/component/renderable.rs` holds procedural shape parameters,
mesh registration, authored MMS serialization and ECS lifecycle code. Move
procedural construction and its authored representation into a cohesive
submodule. Keep the component lifecycle and instance handling in the parent.

Keep mesh registration and reuse rules intact. Perform geometry transforms
only when a mesh is built, and preserve the existing instance and batching
paths without extra per-frame allocations or dispatch.

Validate procedural shape MMS round trips and the pool slide liner geometry.
Compare mesh registration and draw batch counts on the same scene before and
after the split.
