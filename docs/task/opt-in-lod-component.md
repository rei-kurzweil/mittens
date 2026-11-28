# Opt-in level of detail and scoped change events

Design a general opt-in LOD component that selects authored levels for an
object relative to an observer. Emit `LODChanged` on the component's scope
when its selected level changes, with the previous and new level and enough
observer context for scripts to choose behavior. Define initial delivery,
observer loss, multiple observers, hysteresis, and serialization explicitly.

Use distance thresholds with hysteresis as an initial policy; keep visibility,
update rate, animation and secondary-motion decisions with the consumer.
Support different thresholds per object and avoid per-frame script events or
per-object allocations. Decide whether multiple views require the highest
detail demanded by any observer and how networked entities select observers.

For NPC Mitten, experiment first with periodic MMS line-of-sight checks and
per-avatar secondary-motion pausing. Compare zero NPCs, two NPCs with full
motion, and two using visibility control; record CPU phases, GPU deformation,
ray cost, transition artifacts, and headset frame-time tails. Test close and
far NPCs, occlusion, moving observers, avatar replacement and resume after a
long pause. Distance-based LOD and `LODChanged` are future component work.
