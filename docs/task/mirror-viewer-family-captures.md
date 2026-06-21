# Task: Mirror Viewer-Family Captures

## Problem

The current mirror model is still shaped like:

- one `MirrorComponent`
- one `VisualMirror`
- one runtime texture key such as `capture.mirror.<guid>.color`

That is not sufficient when both of these may exist independently in the same frame:

- an active monoscopic camera
- an active stereoscopic camera

In that situation, one planar mirror does not need one capture. It needs a **capture family per
active viewer family**.

## Why the current shape is wrong

A single shared mirror texture forces the engine to pick one source family:

- monoscopic only, or
- stereoscopic only

That loses information whenever both are active.

Symptoms from the current shape include:

- the mirror view tracking the wrong viewer pose
- the player standing in front of one part of the mirror while the reflection is sourced from a
  different camera family
- a desktop mirror and XR mirror implicitly fighting over one published texture

## Intended model

The correct semantic model is:

- monoscopic viewer state is independent from stereoscopic viewer state
- a mirror is one authored logical surface
- each mirror may require multiple related capture requests in one frame
- capture requests are grouped by viewer family

Recommended language:

- `active monoscopic camera`
- `active stereoscopic camera`

The renderer may still use the word `eye` for the concrete per-view draw inside a stereoscopic
family, but `eye` should not be the top-level conceptual unit.

## Proposed runtime shape

Instead of one logical capture per mirror:

```rust
struct VisualMirror {
    mirror_component: ComponentId,
    camera: VisualCamera,
    target_key: String,
    ...
}
```

move toward:

```rust
enum MirrorViewerFamily {
    Monoscopic,
    Stereoscopic,
}

struct MirrorCaptureRequest {
    family: MirrorViewerFamily,
    view_index: usize, // 0 for mono; 0/1/... for stereo views
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
    source_transform: Transform,
    target_key: String,
}

struct VisualMirror {
    mirror_component: ComponentId,
    captures: Vec<MirrorCaptureRequest>,
    ...
}
```

The important change is not the exact struct names. The important change is:

- one mirror owns many capture requests
- capture requests are tagged by viewer family
- published runtime textures are keyed per mirror **and** per viewer family

## Texture publication implications

One selector like:

```text
capture.mirror.<guid>.color
```

is not enough.

At minimum the publication key likely needs a family discriminator, for example:

```text
capture.mirror.<guid>.mono.color
capture.mirror.<guid>.stereo.left.color
capture.mirror.<guid>.stereo.right.color
```

The final naming can change, but the semantics should be:

- mirror identity
- viewer family
- concrete view index within that family

## RenderView implications

`RenderViewKind::Mirror` should no longer mean only:

- "this draw belongs to mirror X"

It also needs:

- which viewer family it serves
- which concrete view inside that family it serves

Conceptually:

```rust
Mirror {
    mirror_component: ComponentId,
    family: MirrorViewerFamily,
    view_index: usize,
    excluded_instance: Option<InstanceHandle>,
}
```

## Surface sampling implications

Once a mirror can publish more than one capture, the visible mirror surface also needs a rule for
which capture it samples during the main pass.

That means the main render path must eventually be aware of viewer family too:

- monoscopic main pass should sample the monoscopic mirror capture
- stereoscopic left-eye main pass should sample the left stereoscopic mirror capture
- stereoscopic right-eye main pass should sample the right stereoscopic mirror capture

Without that, per-family mirror capture generation alone is incomplete.

## Scope for the refactor

This task is specifically about correcting the model and contracts:

1. rename the concepts in docs to use `active monoscopic camera` and `active stereoscopic camera`
2. stop describing one mirror as owning one shared capture
3. introduce viewer-family capture requests into the design
4. describe runtime texture publication as per-family, not per-mirror-only
5. leave `eye` as an implementation detail term for per-view stereo draws unless a better name is
   chosen later

## Done when

- mirror docs no longer imply "prefer one camera family"
- mirror docs no longer imply "one mirror = one texture"
- the design explicitly supports both active monoscopic and active stereoscopic cameras in the same
  frame
- `eye` is treated as a concrete stereoscopic sub-view term, not the top-level viewer abstraction
