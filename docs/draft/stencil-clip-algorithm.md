# Stencil Clip — Algorithm & Draw Order

Companion to `layout-clip-shaders+pipelines.md`.
Covers **why INCR/DECR is required for nesting**, the exact per-frame draw sequence,
and what sort order the draw loop must emit.

---

## 1. Why REPLACE + ALWAYS Breaks for Nesting

The initial pipeline table in the spec used `REPLACE ref=N, test ALWAYS` for the stencil
write pass. This is correct for a single clip level but silently wrong with nesting.

```
ECS tree:
  outer_clip  (ref=1)
    some_stuff_A
    inner_clip  (ref=2)
      some_other_stuff_B
```

**With REPLACE + ALWAYS:**

```
Step 1: draw outer_clip quad
        stencil op: REPLACE ref=1, test ALWAYS
        stencil buffer after:

        ┌─────────────────────────┐
        │  0  0  0  0  0  0  0   │
        │  0  1  1  1  1  1  0   │
        │  0  1  1  1  1  1  0   │
        │  0  1  1  1  1  1  0   │
        │  0  0  0  0  0  0  0   │
        └─────────────────────────┘

Step 2: draw inner_clip quad (REPLACE ref=2, test ALWAYS)
        inner_clip is physically smaller but test is ALWAYS —
        it writes ref=2 regardless of the current stencil value:

        ┌─────────────────────────┐
        │  0  0  0  0  0  0  0   │
        │  0  1  1  1  1  1  0   │
        │  0  1  2  2  2  1  0   │   ← inner area = 2, as expected
        │  0  1  1  1  1  1  0   │
        │  0  0  0  0  0  0  0   │
        └─────────────────────────┘

   This looks OK here, but only because inner_clip is physically
   inside outer_clip. In 3D / VR a transform could place inner_clip
   so it extends outside outer_clip:

        ┌─────────────────────────┐
        │  0  0  2  2  0  0  0   │   ← ref=2 leaked outside ref=1 region!
        │  0  1  2  2  1  1  0   │
        │  0  1  2  2  1  1  0   │
        │  0  0  0  0  0  0  0   │
        └─────────────────────────┘

Step 3: draw some_other_stuff_B with test EQUAL ref=2
        → passes in the leaked region → content visible outside outer_clip ❌
```

Even when transforms are well-behaved, the approach is fragile: correctness depends
on the physical geometry of the clip quads rather than the stencil buffer logic.

---

## 2. INCR / DECR — Correct Nesting

Instead of REPLACE, use INCR (enter) and DECR (exit), both with `test EQUAL ref`.
The single `reference` value is used for both the test operand and the INCR/DECR:

- **Enter clip at depth N:** `set_stencil_reference(N-1)`, draw clip quad,
  `test EQUAL, op INCR` — only increments where stencil == parent depth.
- **Draw clipped content at depth N:** `set_stencil_reference(N)`, `test EQUAL`.
- **Exit clip at depth N:** `set_stencil_reference(N)`, draw clip quad,
  `test EQUAL, op DECR` — only decrements where stencil == N (i.e. inside the clip).

**Frame trace — single nesting level:**

```
Initial stencil buffer: all 0

── Enter outer_clip (depth 1, parent=0) ──────────────────────────────────────
  set_stencil_reference(0)
  draw outer_clip quad → test EQUAL ref=0, op INCR

  before:  0 0 0 0 0       after:    0 0 0 0 0
           0 0 0 0 0                 0 1 1 1 0
           0 0 0 0 0                 0 1 1 1 0
           0 0 0 0 0                 0 0 0 0 0

  Pixels where stencil was 0 (and quad covered them) → incremented to 1.
  Pixels where stencil was already != 0 → test fails → unchanged.

── Draw some_stuff_A (depth 1) ──────────────────────────────────────────────
  set_stencil_reference(1)
  draw some_stuff_A → test EQUAL ref=1, op KEEP
  → only pixels inside outer_clip region pass. ✓

── Enter inner_clip (depth 2, parent=1) ──────────────────────────────────────
  set_stencil_reference(1)
  draw inner_clip quad → test EQUAL ref=1, op INCR

  Suppose inner_clip quad is larger than outer_clip (extends outside):

  before:  0 0 0 0 0       after:    0 0 0 0 0
           0 1 1 1 0                 0 1 2 1 0   ← only where stencil was 1
           0 1 1 1 0                 0 1 2 1 0
           0 0 0 0 0                 0 0 0 0 0

  The leaked region (stencil=0) fails EQUAL ref=1 → NOT incremented. ✓
  inner_clip can never escape outer_clip's boundary.

── Draw some_other_stuff_B (depth 2) ────────────────────────────────────────
  set_stencil_reference(2)
  draw some_other_stuff_B → test EQUAL ref=2, op KEEP
  → only pixels inside both clips pass. ✓
  B does not need to know about outer_clip — INCR ensured it.

── Exit inner_clip (depth 2) ─────────────────────────────────────────────────
  set_stencil_reference(2)
  draw inner_clip quad → test EQUAL ref=2, op DECR

  before:  0 0 0 0 0       after:    0 0 0 0 0
           0 1 2 1 0                 0 1 1 1 0   ← decremented back to 1
           0 1 2 1 0                 0 1 1 1 0
           0 0 0 0 0                 0 0 0 0 0

  Outer clip region restored. Siblings of inner_clip can draw with ref=1. ✓

── Draw more_stuff_A (more depth-1 content after inner subtree) ──────────────
  set_stencil_reference(1)
  draw → test EQUAL ref=1 ✓

── Exit outer_clip (depth 1) ─────────────────────────────────────────────────
  set_stencil_reference(1)
  draw outer_clip quad → test EQUAL ref=1, op DECR → stencil back to 0
```

---

## 3. Sibling Clips at the Same Depth

Two `overflow: hidden` containers that are siblings in the ECS tree both get
`stencil_ref=1`. They must be processed sequentially — write, draw, restore — not
batched together.

```
ECS tree:
  panel_A  (ref=1)
    content_A
  panel_B  (ref=1)   ← sibling, same depth
    content_B
```

```
draw order:

  Enter panel_A:   set_ref(0), draw panel_A quad → INCR
  Draw content_A:  set_ref(1), draw content_A
  Exit panel_A:    set_ref(1), draw panel_A quad → DECR  ← must restore BEFORE panel_B

  Enter panel_B:   set_ref(0), draw panel_B quad → INCR
  Draw content_B:  set_ref(1), draw content_B
  Exit panel_B:    set_ref(1), draw panel_B quad → DECR
```

If panel_A's restore is skipped, panel_A's clip area remains at stencil=1. When
panel_B increments with `test EQUAL ref=0`, it only writes where stencil is still 0 —
the union of both panels' content renders clipped to panel_B's quad, which is wrong.

**Implication for sort order:** you cannot sort all `stencil_ref=1` instances into one
flat group and process them together. The draw loop must follow ECS subtree order so
that each clip quad is bracketed by its own enter/exit.

---

## 4. Required Draw Order

Within each render phase, instances must be emitted in **ECS DFS (depth-first) subtree
order**, not sorted flat by `stencil_ref`. The `stencil_ref` value tells the renderer
which pipeline / reference to use, but ordering comes from the tree.

Pseudocode for the draw loop (one render phase):

```
current_clip_stack: Vec<(stencil_ref: u8, clip_instance_index: usize)> = []

for each instance in DFS order:

  // Close any clips that are no longer ancestors of this instance.
  while let Some(&(ref, idx)) = current_clip_stack.last():
    if instance is still inside this clip's subtree:
      break
    // Exit: DECR
    set_stencil_reference(ref)
    draw stencil_clip_order[idx] quad → pipeline_stencil_decr
    current_clip_stack.pop()

  if instance.is_stencil_clip:
    // Enter: INCR with parent ref
    let parent_ref = current_clip_stack.last().map(|e| e.0).unwrap_or(0)
    set_stencil_reference(parent_ref)
    draw instance quad → pipeline_stencil_incr
    current_clip_stack.push((instance.stencil_ref, instance_index))
    // The clip quad also draws normally in the color pass (double duty).
    draw instance → normal_or_clipped_pipeline, ref = instance.stencil_ref

  else:
    let ref = instance.stencil_ref   // 0 = unclipped
    draw instance → if ref == 0 { normal_pipeline } else { clipped_pipeline },
                    set_stencil_reference(ref)

// End of phase: close any remaining open clips
while let Some((ref, idx)) = current_clip_stack.pop():
  set_stencil_reference(ref)
  draw stencil_clip_order[idx] quad → pipeline_stencil_decr
```

---

## 5. Implications for `stencil_clip_order` in VisualWorld

The current spec sorts `stencil_clip_order` ascending by `stencil_ref`. This ordering
is useful for quickly locating all clip quads at a given depth but is not the primary
ordering for the draw loop — that must be DFS subtree order.

Possible approaches:

**Option A — store DFS index alongside stencil_ref:**
`stencil_clip_order` entries include a `dfs_index` field. The draw loop iterates
instances in DFS order; the clip stack is managed per the pseudocode above.

**Option B — build the sorted instance list in DFS order already:**
`overlay_order` (and equivalent per-phase order arrays) are already built from the
ECS tree traversal. If DFS order is maintained there, `stencil_clip_order` just needs
to be a lookup from instance index → is_stencil_clip, no separate sort needed.

Option B aligns better with the existing `overlay_order` / `DrawBatch` model.

---

## 6. Corrected Pipeline Table

The spec's `pipeline_stencil_write` and `pipeline_stencil_clear` should be replaced:

| Pipeline | Color write | Depth | Stencil test | Stencil op | Note |
|---|---|---|---|---|---|
| `pipeline_stencil_incr` | off | off | EQUAL ref | INCR | enter clip region |
| `pipeline_stencil_decr` | off | off | EQUAL ref | DECR | exit clip region |

Both use `DynamicState::StencilReference`. A separate `pipeline_stencil_clear` (REPLACE
ref=0, ALWAYS) is not needed — DECR unwinds the stack correctly.

Clipped content pipelines are unchanged from the spec:

| Pipeline | Stencil test | Stencil op |
|---|---|---|
| `pipeline_{phase}_clipped` | EQUAL ref=N | KEEP |

---

## 7. Phase-agnostic Notes

The algorithm is identical regardless of render phase (opaque, cutout, transparent,
overlay). Each phase runs its own DFS pass over its own instance list. The stencil
buffer is shared across phases within the same frame, so the draw order between phases
matters:

- Stencil clip quads for opaque-phase content should be drawn inside the opaque pass,
  not before or after it.
- The stencil buffer is cleared to 0 once at the start of the frame (via the
  `stencil_attachment` Clear load op). No cross-phase stencil state is intended.
- If a clip boundary itself renders in a different phase than its children (e.g. an
  opaque clip quad clipping transparent children), the clip write must happen before
  the transparent phase draws — this requires careful ordering or a pre-pass.
  For the initial implementation, assume the clip quad and its children share the same
  render phase.
