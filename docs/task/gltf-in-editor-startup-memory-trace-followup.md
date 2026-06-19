# GLTF-In-Editor Startup Memory Trace Follow-up

Date: 2026-06-19

Status: investigation note based on first coarse RSS tracing pass.

Update: code inspection plus follow-up instrumentation added on 2026-06-19.

Second update: targeted editor bootstrap instrumentation run completed on 2026-06-19.

Third update: retained startup plateau reduced from about `19 GiB` to about `9.15 GiB` on
2026-06-19 after removing duplicated per-template MMS module clones in the editor paint template
path.

## Problem

GLTF-heavy editor examples are hitting extreme process memory growth during startup.

The first coarse RSS tracing pass was added to answer:

- whether memory keeps growing every frame
- or whether there is one or two large startup retention spikes and then a plateau

The current result strongly points to the second shape.

## First important result

From `/tmp/cat-engine-memory.log`:

```text
[memory] #000 after world panel content rebuild rss=10.22 GiB delta_prev=+0 B delta_base=+0 B peak=12.33 GiB
[memory] #001 event loop resumed rss=18.70 GiB delta_prev=+8.47 GiB delta_base=+8.47 GiB peak=23.73 GiB
[memory] #002 window created rss=18.70 GiB delta_prev=+540.00 KiB delta_base=+8.47 GiB peak=23.73 GiB
[memory] #003 before renderer init rss=18.70 GiB delta_prev=+0 B delta_base=+8.47 GiB peak=23.73 GiB components=18215 instances=0 mirrors=0 cpu_meshes=7 imported_meshes=0 gltf_tracked=1
[memory] #004 after renderer init rss=19.02 GiB delta_prev=+333.89 MiB delta_base=+8.80 GiB peak=23.73 GiB components=18215 instances=0 mirrors=0 cpu_meshes=7 imported_meshes=0 gltf_tracked=1
```

## Immediate interpretation

### 1. `after world panel content rebuild` is already too late

The first visible sample is already:

- `rss=10.22 GiB`

That means a large amount of memory growth has already happened before the first sampled
world-panel checkpoint.

So:

- the world-panel rebuild path may still be expensive
- but it cannot explain the full startup blowup by itself

### 2. There is another major jump after that point

Between:

- `#000 after world panel content rebuild`
- and `#001 event loop resumed`

RSS rises by:

- `+8.47 GiB`

That is the most important early observation so far.

It means the process roughly:

- reached `10.22 GiB`
- then jumped again to `18.70 GiB`
- then stayed mostly steady through window creation and renderer init except for a smaller
  `+333.89 MiB` renderer-init cost

### 3. This does not look like a steady per-frame leak

Observed runtime shape so far:

- very large startup-time jumps
- later checkpoints mostly flat or close to flat

That points more toward:

- one-time retained structures
- scene/materialization/import duplication
- large CPU-side retained assets
- repeated startup work that finishes before the frame loop stabilizes

and less toward:

- unbounded per-frame growth after startup

## What this rules out

This evidence weakens the simplest version of:

- “layout tick is the leak”
- “world-panel rebuild is the whole problem”

because:

- the first panel-related sample is already enormous
- a second giant jump occurs after that sample
- later steady-state checkpoints are comparatively flat

Layout or panel rebuild may still contribute, but they are not yet the earliest known cause.

## What the counters say at `before renderer init`

At `#003 before renderer init`:

- `components=18215`
- `instances=0`
- `mirrors=0`
- `cpu_meshes=7`
- `imported_meshes=0`
- `gltf_tracked=1`

This is important because it shows:

- the huge memory residency at that checkpoint is not yet explained by registered imported meshes
  in `RenderAssets`
- it is also not yet explained by live `VisualWorld` instances

That shifts suspicion earlier toward:

- editor/runtime tree materialization
- GLTF-loaded resource retention before `RenderAssets` registration
- duplicated intermediate decoded/imported structures
- stopgap editor UI subtree/materialization work

## Current working hypothesis

There are likely at least two distinct retained-memory steps:

1. a large pre-`after world panel content rebuild` step that already reaches about `10 GiB`
2. another large step between that point and `event loop resumed` that adds about `8.5 GiB`

Then:

- renderer init adds a much smaller but still real `~334 MiB`
- later startup/render checkpoints appear mostly stable relative to those two larger jumps

## Stronger code-level finding

Code inspection points to `GLTFSystem` as a much stronger suspect than the world-panel row model itself.

The important details are:

1. `GLTFSystem::load_gltf_resources(...)` decodes and retains CPU-side meshes and RGBA textures
   in `resources_by_uri`.
2. before renderer init, that cache is **not** reflected in `RenderAssets.imported_meshes`, which
   matches the existing trace:
   - `imported_meshes=0`
   - `gltf_tracked=1`
3. later, `GLTFSystem::flush_imports(...)` clones those cached meshes into `RenderAssets`,
   meaning the previous code path could temporarily or permanently hold:
   - one copy in `LoadedGltf.meshes`
   - another copy in `RenderAssets.cpu_meshes`
4. the previous code also kept decoded RGBA texture blobs in the GLTF cache even after upload.
5. `GLTFSystem::tick_with_queue(...)` performs another `gltf::import(...)` to walk the document
   and node tree for spawning, which means startup may also see a large transient duplicate import
   spike on top of the retained cache.

That combination is consistent with the observed shape:

- a large retained pre-renderer residency that does **not** show up in `RenderAssets` counters
- another large startup-time spike while import/spawn work is still active

## What changed in code for the next pass

Instrumentation now exposes GLTF cache size directly in memory trace samples:

- `gltf_cached_resources`
- `gltf_cached_meshes`
- `gltf_cached_textures`
- `gltf_cached_cpu`

The GLTF cache path was also tightened so that after handoff:

- mesh payloads are dropped after registration into `RenderAssets`
- texture RGBA payloads are dropped after upload

This does **not** remove the second `gltf::import(...)` yet, but it does remove a clear retained
CPU-side duplication path and makes the next trace much more interpretable.

## Revised interpretation

At this point the leading explanation is:

1. decoded GLTF CPU assets are a major part of the pre-renderer resident set
2. the editor panel code may still add cost, but it is unlikely to be the dominant source of the
   multi-GiB footprint by itself
3. the extra import performed for node-tree spawning is a likely explanation for part of the
   startup peak / transient spike

## What the next editor-targeted trace proved

The editor bootstrap trace now isolates the first large retained startup jump much more precisely.

Key result from `/tmp/cat-engine-memory.log`:

```text
🟧✏ [editor-memory] editor spawn_panel_layout:after asset_panel expr
[memory] #013 editor spawn_panel_layout:after asset_panel expr rss=9.96 GiB delta_prev=+1.70 GiB delta_base=+1.70 GiB peak=12.31 GiB
```

That means:

- the jump happens during editor panel layout spawn
- specifically at the `asset_panel` expression/materialization boundary
- not during world-panel scene-model rebuild
- not during world-panel row-model build
- not during world-panel content rerender
- not during asset-panel item population

Other relevant checkpoints from the same run:

```text
[memory] #000 editor setup_panels_for_editor:start rss=8.26 GiB
[memory] #004 editor rebuild_world_panel_scene_model:end rss=8.26 GiB
[memory] #012 editor spawn_panel_layout:after world_panel expr rss=8.26 GiB
[memory] #013 editor spawn_panel_layout:after asset_panel expr rss=9.96 GiB delta_prev=+1.70 GiB
[memory] #025 editor spawn_panel_layout:end rss=9.90 GiB
[memory] #032 editor setup_panels_for_editor:end rss=9.90 GiB
[memory] #033 event loop resumed rss=19.31 GiB delta_prev=+9.41 GiB
```

## Updated conclusions

At this point the evidence says the memory issue is **not primarily GLTF retained CPU asset data**.

The current breakdown is:

1. `GLTFSystem` retained decoded CPU asset data exists, but measured only about `83 MiB` before
   render flush and drops to about `10 KiB` after handoff.
2. a confirmed editor/UI/materialization jump of about `+1.70 GiB` occurs at:
   - `editor spawn_panel_layout:after asset_panel expr`
3. a second, much larger jump of about `+9.41 GiB` occurs **after**
   `setup_panels_for_editor:end` but **before** `event loop resumed`

This shifts the investigation away from:

- world-panel scene traversal
- GLTF retained cache as the dominant source

and toward:

- asset-panel expression/materialization
- deferred panel layout mount / subtree spawn / init work
- bootstrap or event-loop-adjacent materialization/layout work after editor setup returns

## Current leading suspects

### Suspect A: asset panel expression construction

The `asset_panel` expression boundary is the first clearly measured multi-GiB jump.

Likely code paths:

- `build_panel_component_expr(...)` for `asset_panel`
- MMS module load/eval/materialization for the asset panel
- any large `MaterializedCE` / CE tree duplication done while preparing panel content

### Suspect B: post-setup deferred bootstrap work

The larger jump still not explained by editor-setup-local checkpoints is:

- `editor setup_panels_for_editor:end`
- to `event loop resumed`
- about `+9.41 GiB`

Likely code paths:

- deferred subtree attach/init after panel setup returns
- `spawn_panel_layout_mount(...)` follow-on work not bounded by the current inner markers
- pre-event-loop layout/materialization/bootstrap work
- window/bootstrap code that realizes queued editor panel trees after registration returns

## Most likely next gaps in instrumentation

The current tracer starts too late to explain the first `10.22 GiB`.

The next pass should add earlier checkpoints around the editor/bootstrap path that runs before the
current world-panel content sample.

Highest-priority missing boundaries:

1. editor panel shell materialization start/end
2. world-panel scene-model build start/end
3. world-panel content item/model build start/end
4. inspector panel shell/content materialization start/end
5. GLTF document/resource load start/end
6. GLTF node subtree spawn start/end
7. any large MeowMeow module spawn/materialization boundaries used by editor panels

## Recommended interpretation discipline

Do not over-interpret `#000 delta_base=+0 B`.

That only means:

- it was the first sample the tracer recorded in that run

It does **not** mean:

- the preceding world-panel work was cheap

The meaningful fact is the absolute RSS at that first sample:

- about `10.22 GiB`

## Next actions

1. Move the earliest coarse samples farther up the startup/editor bootstrap path so the first
   observed sample is well before world-panel content rebuild.
2. Add matching samples around inspector panel materialization, not just world-panel content.
3. Compare the same early startup checkpoints between:
   - a control scene without GLTF in editor
   - `vtuber-editor-example`
   - `vtuber-mirror-example`
4. Check whether the `LoadedGltf` resource cache is retaining large decoded CPU blobs before
   `RenderAssets` registration makes that visible in counters.
5. Capture one full clean run with the log file truncated beforehand so sample numbering is easier
   to compare across runs.

## Next run checklist

On the next `CAT_DEBUG_MEMORY=1` run, compare:

- `before gltf.tick_with_queue`
- `after gltf.tick_with_queue`
- `after gltf queue.flush`
- `prepare_render:start`
- `prepare_render:after renderable flush`

If `gltf_cached_cpu` is already very large before renderer init, that confirms the retained GLTF
cache is a primary contributor.

If RSS drops materially or stops climbing after `prepare_render` once cached payloads are released,
that confirms the old duplication path was a real part of the problem.

If the peak is still much larger than `gltf_cached_cpu`, the next target should be eliminating the
second `gltf::import(...)` by keeping only the minimal node/document metadata needed for spawn.

## Next instrumentation pass

The next pass should focus on the two newly isolated hotspots:

1. inside `build_panel_component_expr(...)`
   - before module load/eval
   - after module eval/materialization
   - after any decoration/cloning or CE post-processing
2. around `spawn_panel_layout_mount(...)`
   - before spawn
   - after spawn/materialization
   - after subtree init / attach
3. in the bootstrap path between:
   - `register_editor(...)` return
   - `event loop resumed`

That should tell us:

- whether the `asset_panel` jump is MMS/module/materialization specific
- whether the larger `+9.41 GiB` jump is caused by deferred attach/init/layout work outside the

## Final root cause for the retained `~19 GiB` plateau

The editor bootstrap trace eventually isolated the dominant retained jump to:

- `editor register_editor:after scoped handler install`

Earlier instrumentation showed:

- `setup_panels_for_editor:end` at about `9.13 GiB`
- then `register_editor:after scoped handler install` jumping to about `18.62 GiB`
- with that larger residency staying flat afterward

That meant the long-lived startup plateau was **not** coming from:

- GLTF retained CPU cache
- panel mount spawn
- world-panel scene-model rebuild
- asset-panel item population

It was coming from work performed during editor scoped-handler installation.

## Confirmed code-level cause

The decisive code path was:

- `SystemWorld::register_editor(...)`
  - calls `self.asset_system.paint_templates()`
  - passes the returned templates into `EditorPaintSystem::install_scoped_handlers_for_editor(...)`

Before the fix, `AssetSystem::paint_templates()` built one `PaintAssetTemplate` per asset item and
each template stored:

- a full cloned `LoadedMmsModule`

That was the wrong ownership shape.

`LoadedMmsModule` contains the already-evaluated MMS module payload:

- `named_exports`
- `sequence`
- source metadata

So if many paintable asset items came from the same MMS module, the old code did **not** share one
loaded module between them. Instead, it duplicated the full evaluated module graph once per
template.

In other words:

1. one asset module was loaded into `AssetSystem.modules`
2. `paint_templates()` iterated paintable items
3. every item cloned the entire `LoadedMmsModule`
4. `EditorPaintSystem` retained that cloned vector in shared state
5. startup residency scaled with the number of paint templates, not just the number of distinct
   modules

That matches the trace shape exactly:

- the jump happened when editor paint setup received/stored template data
- the jump remained steady afterward because those duplicated module graphs were retained

## The fix

The fix changed MMS module ownership from:

- per-template deep clone

to:

- shared `Arc<LoadedMmsModule>`

Specifically:

- `AssetModule.module` now stores `Arc<LoadedMmsModule>`
- `PaintAssetTemplate.module` now stores `Arc<LoadedMmsModule>`
- `paint_templates()` now clones the `Arc`, not the module contents

That means all templates that refer to the same underlying module now share one module allocation.

This does **not** make the module smaller.

It fixes the bug because it removes duplicated retained copies of the same module graph.

## Measured result after the fix

The trace after the `Arc` change shows:

- `editor register_editor:after setup_panels_for_editor` at about `9.15 GiB`
- `editor register_editor:after scoped handler install` still at about `9.15 GiB`
- `event loop resumed` still at about `9.15 GiB`

The previous retained jump:

- about `+9.49 GiB`

collapsed to:

- about `+48 KiB`

So the retained startup plateau dropped from:

- about `18.6–19 GiB`

to:

- about `9.15 GiB`

## Revised final interpretation

The main retained startup blowup was **not** GLTF.

It was duplicated editor paint-template MMS module residency caused by copying full
`LoadedMmsModule` values into every `PaintAssetTemplate`.

The `Arc` fix worked because:

- the heavy evaluated MMS module data only needs shared ownership
- it does **not** need independent per-template copies

What remains after the fix is a smaller editor/UI cost, especially around:

- `asset_panel` materialization at roughly `+0.9–1.0 GiB`

but the catastrophic retained `~19 GiB` plateau appears resolved.
  editor setup function

## Related

- [docs/task/armature-visualization-startup-followup.md](./armature-visualization-startup-followup.md)
- [docs/task/editor-panel-selection-refresh-perf-investigation.md](./editor-panel-selection-refresh-perf-investigation.md)
