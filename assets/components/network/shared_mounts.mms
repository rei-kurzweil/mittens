// Shared attachment frames addressed by authored numeric ids. Each peer
// registers its own copy of a frame (a truck bed) under the same id, so a
// shared item is mounted with one event and then rides that frame locally.
export fn shared_mounts() {
    let frames = {}
    fn register(id, frame) { frames["" + id] = frame }
    fn frame(id) { return frames["" + id] }
    return { register = register frame = frame }
}

// Releases into `zone` attach the item there at its release pose, with no
// limit on how many items it holds. When `mounts` and `id` are given, the
// zone is also a shared mount: a shared item announces the new mount to its
// peer through `shared_mount_changed`. Unshared items attach locally only.
export fn attach_drops(zone, mounts, id) {
    if mounts && id != null { mounts.register(id, zone) }
    on(zone, "ZoneDropped", fn(drop) {
        let pose = drop.target.world_trs()
        zone.attach(drop.target)
        drop.target.world_trs(pose)
        if id != null && drop.target.shared_mount_changed {
            drop.target.shared_mount_changed(id)
        }
    })
}
