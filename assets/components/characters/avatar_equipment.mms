let slot_names = ["equipment_left_thigh", "equipment_right_thigh", "equipment_back", "equipment_head"]
let avatar_attachment_index = 0

// Places an item in a slot as a drop would: items marked
// align_to_equipment_slot take the slot's rest pose, one per slot. Returns
// false when the slot already holds one.
fn place_in_slot(anchor, zone, target) {
    let pose = target.world_trs()
    if target.align_to_equipment_slot {
        for item in zone.query_all("transform") {
            if item.align_to_equipment_slot { return false }
        }
        let rest = anchor.world_trs()
        pose = { translation = rest.translation rotation_quat_xyzw = rest.rotation_quat_xyzw scale = pose.scale }
    }
    zone.attach(target)
    target.world_trs(pose)
    return true
}

// Each placement is [bone name, local center, local radius, round pitch] in
// the GLB's own units and radians; they follow any scale baked on import.
export fn install_avatar_equipment(model, placements) {
    on(model, "GLTFInitialized", fn(event) {
        for i in range(len(placements)) {
            let placement = placements[i]
            let bone = event.gltf.query("[name='" + placement[0] + "']")
            if bone {
                let k = event.gltf.import_scale()
                let center = [placement[1][0] * k, placement[1][1] * k, placement[1][2] * k]
                let zone = Zone.sphere(placement[2] * k).receive_drops(avatar_attachment_index) {
                    name = slot_names[i]
                }
                let anchor = T.position(center[0], center[1], center[2]).rotation(placement[3], 0, 0) {
                    name = slot_names[i] + "_anchor"
                    zone
                }
                bone.attach(anchor)
                on(zone, "ZoneDropped", fn(drop) { place_in_slot(anchor, zone, drop.target) })
            }
        }
    })
}

export fn install_vroid_equipment(model) {
    install_avatar_equipment(model, [
        ["J_Bip_L_UpperLeg", [0.14, 0.17, 0.0], 0.18, Math.pi / 2.0],
        ["J_Bip_R_UpperLeg", [-0.14, 0.17, 0.0], 0.18, Math.pi / 2.0],
        ["J_Bip_C_UpperChest", [0.0, 0.05, -0.30], 0.23, -1.29],
        ["J_Bip_C_Head", [0.0, 0.24, 0.0], 0.19, -Math.pi / 2.0]
    ])
}

// Puts item in a named slot (for example "equipment_left_thigh") without a
// drop, as an NPC equipping itself does. False before the model has loaded
// or when the slot is taken.
export fn mount_equipment(root, slot, item) {
    let anchor = root.query("[name='" + slot + "_anchor']")
    if !anchor { return false }
    let zone = anchor.query("[name='" + slot + "']")
    if !zone { return false }
    return place_in_slot(anchor, zone, item)
}

export fn detach_avatar_equipment(root) {
    for name in slot_names {
        for item in root.query_all("[name='" + name + "'] > transform") {
            let pose = item.world_trs()
            item.detach()
            item.world_trs(pose)
        }
    }
}
