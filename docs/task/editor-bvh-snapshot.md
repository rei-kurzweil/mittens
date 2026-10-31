# Editor BVH snapshot

The Editor settings panel includes **Show BVH snapshot** and **Refresh BVH
snapshot**. Both controls are present by default; the visualization starts off.

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
        config = { show_bvh = true show_bvh_refresh = true }
    }])
}
```

These flags author controls, rather than enabling the diagnostic. Rust callers
can use `set_bvh_snapshot_visible(visible, scope_roots)` and
`refresh_bvh_snapshot()` on EditorUIComponent.

The snapshot uses existing retained BVHs; it never builds an alternate tree.
Detail collision nodes are captured first. Exact duplicate boxes/edges are
coalesced and output is capped at 4,096 boxes. The console reports box count and
whether the snapshot was truncated. Retained snapshots still cost geometry to
draw each frame, but unchanged snapshots do no BVH traversal or mesh upload.

Orange rails are self-lit through an opaque custom surface shader, with a
brighter centre. This works in the current Android albedo renderer without
adding a bloom pass or requiring a legacy material. It is not a postprocess glow
halo. Visibility uses normal depth testing, so solid surfaces occlude internal
boxes; cuts expose the local hierarchy.

Validation covers laziness, frozen snapshots, explicit refresh, toggle-off and
owner-deletion cleanup, upload failure retaining the previous snapshot, planar
bounds, output limits, scope ownership, independently authored controls and
serialization. The Android renderer smoke check accepts `--bvh-snapshot`, writes
`/tmp/mittens-bvh-snapshot.ppm`, checks orange pixels and checks that turning the
snapshot off restores the previous rendered scene.
