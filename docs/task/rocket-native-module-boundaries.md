# Native rocket support module boundaries

Status: proposed. Velocity integration, component registration, and signal
conversion files are oversized. Keep acceleration and projectile sweep logic in
cohesive dedicated modules, with small integration calls in existing files.
Future extraction should separate motion-provider resolution and signal payload
conversion without per-step resource creation or per-draw allocations. Preserve
fixed-step integration and retained collision broad phases; validate motion,
serialization, and high-speed impact behavior with focused tests.
