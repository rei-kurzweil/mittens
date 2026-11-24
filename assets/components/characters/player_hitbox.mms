// Attach to the tracked head. Rocket sweeps need a box collision zone.
// The matching invisible mesh and the avatar meshes share its pistol hit tag.
export fn player_hitbox(name) {
    let zone = Zone.cube([0.24, 0.50, 0.22]) {
        name = name
        Collidable.slide().response("read_only") {}
    }
    let proxy = R.cube() { Opacity.opacity(0.0).multiple_layers() Raycastable.enabled() }
    proxy.player_hit_zone = zone
    let root = TOP.fork_trs().extract_yaw().drop("scale") {
        T.position(0.0, -0.38, 0.0) {
            Selectable.off()
            zone
            T.scale(0.48, 1.0, 0.44) { proxy }
        }
    }
    fn tag_model(model) {
        for part in model.query_all("renderable") { part.player_hit_zone = zone }
    }
    return { root = root zone = zone tag_model = tag_model }
}
