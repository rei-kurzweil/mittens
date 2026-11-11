# Sakura scene responsibility split

Status: proposed. The scene has grown to include environment construction,
avatar setup, target placement, and weapon input in one example file.

Extract cohesive scene families into reusable components as future work: keep
roof/building assembly, avatar and input setup, and weapon-table interactions
at separate boundaries. Preserve shared materials and existing batched geometry;
do not introduce per-frame queries or per-object resource creation. Validate
both XR and desktop variants, held input, target interactions, and scene startup;
compare startup and frame performance if the extraction changes batching or
runtime subscriptions. This is not a prerequisite for scoped scene changes.
