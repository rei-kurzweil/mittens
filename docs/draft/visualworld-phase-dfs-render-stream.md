# VisualWorld — Per-Phase DFS Render Stream

Draft note for representing stencil-clipped rendering in `VisualWorld` without
turning `VisualInstance` itself into a tree-walk problem at draw time.

This complements:
- `stencil-clip-algorithm.md` — correctness and draw-order semantics
- `layout-clip-shaders+pipelines.md` — pipeline / attachment changes

---

## Problem

Once stencil clipping is introduced, flat per-phase sort-by-material batching is no
longer sufficient.

Why:
- nested clip regions require **enter / draw / exit** ordering
- sibling clip regions at the same depth cannot be merged into one flat batch
- the renderer must follow **DFS subtree order** within each render phase
- a clip boundary may also be a visible draw (for example `__bg`)

So the renderer needs more than:

```rust
Vec<VisualInstance>
Vec<DrawBatch>
```

It needs a precomputed, phase-local draw stream that preserves hierarchy-sensitive
ordering while still allowing batching where legal.

---

## Core Model

Keep these two layers separate:

### 1. Stored render data

`VisualWorld` still owns one authoritative `VisualInstance` per live renderable.

That instance stores the usual immutable-or-slow-changing draw data:
- mesh
- material
- transform
- texture / filtering
- color / opacity
- phase membership
- `stencil_ref`
- `is_stencil_clip`

This is the **data layer**.

### 2. Precomputed render stream

Separately, `VisualWorld` lazily builds a per-phase DFS-ordered command stream when
relevant dirty flags fire.

This stream is the **execution layer**.

It contains references to `VisualInstance`s plus explicit clip enter/exit events.

That means the same underlying `VisualInstance` may appear multiple times in the
stream:
- once as a visible color draw
- once as a stencil enter event
- once as a stencil exit event

This does **not** mean duplicating the `VisualInstance` itself.

---

## Why `Vec<VisualInstance>` Is Not Enough

A plain `Vec<VisualInstance>` is insufficient because it cannot express:
- "enter this clip region now"
- "draw these descendants while clip depth = N"
- "exit this clip region now"

Those are not instance properties. They are **ordering events**.

A clip boundary is therefore best understood as:
- one stored `VisualInstance`
- referenced multiple times by the render stream in different roles

For example:

```text
instance #42 = __bg quad

render stream:
  EnterClip { instance: 42, parent_ref: 0, new_ref: 1 }
  DrawInstance { instance: 42, stencil_ref: 1 }
  DrawBatch { instances: [43..57], stencil_ref: 1 }
  ExitClip { instance: 42, ref: 1 }
```

---

## Recommended Shape

### Per-phase DFS stream

For each render phase, `VisualWorld` should lazily build something conceptually like:

```rust
pub struct PhaseRenderStream {
    pub ops: Vec<RenderOp>,
}

pub enum RenderOp {
    EnterClip {
        instance_index: u32,
        parent_ref: u8,
        new_ref: u8,
    },
    DrawBatch {
        first: u32,
        count: u32,
        pipeline_key: PipelineKey,
        stencil_ref: u8,
    },
    ExitClip {
        instance_index: u32,
        ref_value: u8,
    },
}
```

Where `DrawBatch` references a contiguous run in a separate `draw_instances: Vec<u32>`
array containing `VisualInstance` indices.

That gives:
- DFS-correct ordering
- explicit clip enter/exit control
- good batch locality
- no duplicated `VisualInstance` storage

---

## Why Not `Vec<VisualInstance>` Duplicates?

You *could* duplicate `VisualInstance` entries for clip use, but that is the wrong
abstraction.

Problems with duplication:
- transform/material/color must stay in sync across copies
- dirty updates become more expensive
- the clip/event role gets conflated with the instance data itself
- visible draw vs stencil enter/exit become hard to reason about

A clip boundary is not a second renderable. It is a second **use** of the same
renderable.

So the better split is:
- `VisualInstance` = stored object
- render stream op = scheduled use of that object

---

## Do We Need a Heterogeneous Collection?

### Short answer

Yes, conceptually.

The draw stream is heterogeneous because it contains at least two different kinds of
things:
- draw work
- clip boundary events

The question is only **how** to represent that efficiently.

---

## Representation Options

### Option A — `Vec<RenderOp>` enum

```rust
enum RenderOp {
    EnterClip { ... },
    DrawBatch { ... },
    ExitClip { ... },
}
```

**Pros:**
- easiest to understand
- maps directly to the algorithm doc
- simplest command recording loop
- explicit and debuggable

**Cons:**
- enum tagging / branching per op
- larger element size than a tight SoA layout
- may be mildly less cache-friendly if the stream gets very large

**Verdict:** best starting point unless profiling proves otherwise.

### Option B — tagged command stream + side arrays

```rust
struct PhaseRenderStream {
    tags: Vec<OpTag>,
    clip_events: Vec<ClipEvent>,
    draw_batches: Vec<DrawBatch>,
    tag_payload_index: Vec<u32>,
}
```

Where `tags[i]` says whether stream entry `i` is a clip event or draw batch, and the
payload lives in a type-specific side array.

**Pros:**
- avoids large enum payloads
- keeps draw-batch payloads densely packed
- still preserves a single ordered stream

**Cons:**
- more bookkeeping
- more indirection during recording
- harder to inspect/debug

**Verdict:** good second step if the plain enum stream becomes a bottleneck.

### Option C — separate ordered arrays with span/range metadata

For example:
- one DFS-ordered `instance_indices` array
- one `clip_enter_at[i]`
- one `clip_exit_after[i]`
- one `batch_ranges` array

This tries to reconstruct clip events from parallel arrays rather than storing explicit
ops.

**Pros:**
- very compact
- potentially cache-friendly

**Cons:**
- much harder to reason about
- easy to get wrong for nested/sibling clip cases
- renderer has to derive execution semantics from multiple structures

**Verdict:** probably too clever for the first implementation.

---

## Performance Reality

A heterogeneous `Vec<RenderOp>` is unlikely to be the bottleneck.

The expensive parts are more likely to be:
- rebuilding the stream too often
- breaking otherwise-large material batches into many tiny DFS-constrained batches
- issuing more draw calls because clip boundaries bracket subtrees
- command-buffer recording overhead for deeply nested documents

Compared to that, one branch on `match op` per stream element is usually cheap.

So the main optimization is **not** “avoid a heterogeneous vector at all costs”.
The main optimization is:
- rebuild lazily via dirty flags
- precompute DFS order once in `VisualWorld`
- precompute legal batch ranges once in `VisualWorld`
- let the renderer consume a ready-to-record stream with minimal tree logic

---

## Dirty-flag Strategy

Yes — this should be rebuilt lazily.

The DFS stream should be recomputed only when one of these changes:
- ECS topology affecting ancestry / sibling order
- phase membership changes
- `StencilClipComponent` attach/detach
- `stencil_ref` reassignment
- material/texture changes that alter batch boundaries

This suggests a dedicated dirty bit alongside existing draw-cache dirtiness, e.g.:

```rust
pub dirty_phase_streams: bool,
```

or folded into the existing draw-cache rebuild path if that stays manageable.

---

## Phase-local Streams

The target structure is not one giant global stream.

It should be one stream per render phase, because each phase already has distinct:
- pipeline families
- depth/write behavior
- blending behavior
- batching rules

For example:

```rust
pub struct VisualWorld {
    opaque_stream: PhaseRenderStream,       // UI panels, layout quads — stencil clip lives here
    cutout_stream: PhaseRenderStream,       // alpha-tested, may also carry clipped UI
    transparent_stream: PhaseRenderStream,  // transparent UI, fades
    overlay_stream: PhaseRenderStream,      // gizmos only — no stencil clip needed
}
```

Each stream still references the same underlying `instances: Vec<VisualInstance>`.

**Phase policy**: overlay is gizmos and debug only. UI layout elements belong in opaque or
transparent so that scene geometry can occlude them. Stencil clip therefore applies primarily
to opaque (and secondarily transparent), not overlay. See `docs/spec/render-phases.md`.

---

## Batch Semantics

Batching is still important, but only within DFS-legal contiguous runs.

That means:
- order is primary
- batching is opportunistic inside that order

A batch can merge only when all of these match:
- same render phase
- same clip context / effective `stencil_ref`
- same pipeline/material/mesh/texture/filtering state
- no clip enter/exit event occurs between the instances

So the renderer should not try to globally coalesce all `stencil_ref=1` content.
That breaks sibling clip correctness.

---

## Recommended First Implementation

1. Keep `instances: Vec<VisualInstance>` as the canonical storage.
2. Add per-phase DFS render streams in `VisualWorld`. **Start with `opaque_stream`** — that is
   where stencil-clipped UI lives. `overlay_stream` can exist but degenerates to flat DrawBatch
   ops for gizmos (no stencil clip in that phase).
3. Represent streams initially as `Vec<RenderOp>`.
4. Make `DrawBatch` ops reference ranges of instance indices rather than embedding copies.
5. Use dirty flags to rebuild only when topology / clip / batch-affecting state changes.
6. Only optimize the representation further if profiling shows the enum stream itself is hot.

This gives the cleanest path to correct nested clipping for HTML-like documents without
forcing the renderer to rediscover tree structure every frame.

---

## Key Takeaway

The important optimization is not “flatten everything into batches”.

The important optimization is:
- store renderables once,
- precompute DFS-ordered phase-local execution once,
- reuse that precomputed stream during Vulkan command recording.

So yes: the clipped draw model wants a per-phase DFS-ordered list — but that list should
be a list of **references and events**, not a duplicated `Vec<VisualInstance>`.
