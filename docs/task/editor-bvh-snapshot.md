# Editor BVH snapshot

The Editor settings panel includes **Show BVH snapshot**, **Refresh BVH
snapshot**, and **Animate BVH**. All controls are present by default; visualization
and animation start off.

Enable it to capture orange wire boxes for the collision broad phase, the
triangle BVHs inside damaged chunks, and the scene raycast BVH. Refresh captures
the current hierarchy after changes such as pistol cuts. Disable removes the
snapshot and releases its mesh. Refresh while disabled does nothing.

The snapshot stays frozen in world space. Neither moving an object nor cutting
it automatically traverses or rebuilds the snapshot. The diagnostic itself is
one batched custom-material visual per requesting EditorUI, with no pickable,
collidable or serializable scene nodes. Editor UI is excluded from the captured
hierarchy. Planar bounds produce squares rather than duplicated flat cubes.

Configure the controls independently:

```mms
EditorUI {
    panels([{
        panel = "settings"
        config = { show_bvh = true show_bvh_refresh = true show_bvh_animation = true }
    }])
}
```

These flags author controls, rather than enabling the diagnostic. Rust callers
can use `set_bvh_snapshot_visible(visible, scope_roots)` and
`refresh_bvh_snapshot()` on EditorUIComponent.

**Animate BVH** highlights depth zero, then the next depth every 250 ms. All
other boxes stay visible at their normal intensity. Shallow leaves dim when
the frontier moves deeper; the traversal loops only after the deepest captured
level has had its turn. Slow frames advance a single level, without skipping
levels. Turning animation off restores the static appearance; hiding the
snapshot stops animation. Refreshing restarts at the roots of the new snapshot.
The animation toggle does nothing while the snapshot is hidden.

The wire mesh stores each node's depth in its UVs. Animation updates only a
cached material parameter, without new BVH traversal, mesh uploads or per-frame
allocations. It uses the same one-batch custom surface path. Animation is over
the frozen snapshot, not the live scene. A truncated snapshot animates only its
captured levels. Rust callers use `set_bvh_animation_enabled(enabled)`.

The snapshot uses existing retained BVHs; it never builds an alternate tree.
Parents are captured before their descendants. Triangle trees begin one level
below their owning collision leaf. The collision and raycast trees retain their
own roots and animate together. Exact duplicate boxes/edges at the same depth
are coalesced. Capture gathers metadata separately from the 4,096-box draw
budget: outer roots and damaged-cell mesh roots are retained first, then shallow
nodes across the scene. Reaching the draw cap reduces fine detail instead of
stopping at one subtree and losing entire older/newer regions. The console
reports when detail is limited. Capture metadata itself has a 131,072-node cap.
Wire rails use two crossed double-sided quads, reducing vertices by 3x and
triangles by one third compared with cube rails. Retained snapshots still cost
geometry to draw each frame, but unchanged snapshots do no BVH traversal or mesh
upload.

Orange rails are self-lit through an opaque custom surface shader, with a
brighter centre. Both the custom program handle and material binding are set,
so desktop uses the diagnostic shader as well. This works in the current Android
albedo renderer without adding a bloom pass or requiring a legacy material. It is not a postprocess glow
halo. Visibility uses normal depth testing, so solid surfaces occlude internal
boxes; cuts expose the local hierarchy.

Validation covers laziness, frozen snapshots, explicit refresh, toggle-off and
owner-deletion cleanup, upload failure retaining the previous snapshot, planar
bounds, output limits, scope ownership, independently authored controls and
serialization. The Android renderer smoke check accepts `--bvh-snapshot`, writes
`/tmp/mittens-bvh-snapshot.ppm`, checks orange pixels, exercises a full animation
cycle and verifies that stopping animation restores the static boxes. It writes
`/tmp/mittens-bvh-animation.ppm` and verifies that hiding the snapshot restores
the previous rendered scene.
