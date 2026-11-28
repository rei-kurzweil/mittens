# Stereo GPU profile module split

Status: proposed before extending `vulkano_ship_profile.rs`.

Split the oversized profiling fixture by responsibility: scene and camera
setup, sample collection, and report aggregation. Keep the offscreen stereo
renderer and query-pool instrumentation shared so comparisons use identical
render paths.

Preserve the current timestamp placement, asynchronous GPU behavior, sample
cadence, and output schema. Avoid per-draw allocations, extra submissions or
pipeline creation in the measured path. Validate matching images, draw counts,
GPU pass timings and CPU frame timings on the same GPU before and after the
split, including unsupported timestamp handling.
