let slot_names = ["equipment_left_thigh", "equipment_right_thigh", "equipment_back"]
let avatar_attachment_index = 0

// Each placement is [bone name, local center, local radius, round pitch] in rig units and radians.
export fn install_avatar_equipment(model, placements) {
    on(model, "GLTFInitialized", fn(event) {
        for i in [0, 1, 2] {
            let placement = placements[i]
            let bone = event.gltf.query("[name='" + placement[0] + "']")
            if bone {
                let center = placement[1]
                let zone = Zone.sphere(placement[2]).receive_drops(avatar_attachment_index) {
                    name = slot_names[i]
                }
                let anchor = T.position(center[0], center[1], center[2]).rotation(placement[3], 0, 0) { zone }
                bone.attach(anchor)
                on(zone, "ZoneDropped", fn(drop) {
                    let pose = drop.target.world_trs()
                    if drop.target.align_to_equipment_slot {
                        for item in zone.query_all("transform") {
                            if item.align_to_equipment_slot { return }
                        }
                        let rest = anchor.world_trs()
                        pose = { translation = rest.translation rotation_quat_xyzw = rest.rotation_quat_xyzw scale = pose.scale }
                    }
                    zone.attach(drop.target)
                    drop.target.world_trs(pose)
                })
            }
        }
    })
}

export fn detach_avatar_equipment(model) {
    for name in slot_names {
        for item in model.query_all("[name='" + name + "'] > transform") {
            let pose = item.world_trs()
            item.detach()
            item.world_trs(pose)
        }
    }
}
