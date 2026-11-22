import { install_avatar_equipment } from "./avatar_equipment.mms"

// Runtime adaptation for the optional TDA export. Joint names are the names
// retained by its importer; the numeric suffixes distinguish the two sides.
export fn miku_twintails() {
    return SecondaryMotion {
        for root in ["[name='������.R_032']", "[name='������.L_041']"] {
            SpringBone.from_root(root).stiffness(2.4).drag_force(0.55)
                .gravity(0.8, [0, -1, 0]).virtual_end_length_ratio(1.0)
        }
    }
}

export fn install_miku_equipment(model) {
    install_avatar_equipment(model, [
        ["���D.L_0162", [1.55, -2.0, 0.0], 1.35],
        ["���D.R_0172", [-1.55, -2.0, 0.0], 1.35],
        ["���������2_010", [0.0, 0.2, -2.15], 1.8]
    ])
}

// AVC splices the head away from the torso and solves shoulder/elbow/wrist as
// a direct chain. This export nests the right shoulder beneath the tongue
// and inserts twist joints before both elbows and wrists. Keep the weighted
// helpers on the arm while giving the solver direct parents. Each new local
// pose is inverse(new parent bind matrix) * joint bind matrix, in GLB units.
export fn prepare_miku_arms(model) {
    let links = [
        ["���������2_010", "���.R_054", [-0.234880, 2.184774, -0.003031], [-0.528596, 0.388626, 0.585067, 0.476710]],
        ["���.R_055", "������.R_057", [0, 2.742810, 0], [0.001809, -0.000006, 0.015105, 0.999884]],
        ["������.R_057", "������.R_059", [0, 2.411374, 0], [-0.019763, -0.000248, -0.030723, 0.999333]],
        ["���.L_0100", "������.L_0102", [0, 2.742811, 0], [-0.001809, 0.000002, 0.015105, 0.999884]],
        ["������.L_0102", "������.L_0104", [0, 2.411374, 0], [0.019763, 0.000253, -0.030723, 0.999333]],
    ]
    for link in links {
        let parent = model.query("[name='" + link[0] + "']")
        let child = model.query("[name='" + link[1] + "']")
        if !parent || !child { return false }
        parent.attach(child)
        child.trs({ translation = link[2] rotation_quat_xyzw = link[3] scale = [1, 1, 1] })
    }
    return true
}

// Held-object offsets are in metres. Preserve the calibrated wrist orientation
// and cancel the model scale on the generated hand grips alone.
export fn normalize_miku_grips(model, model_scale) {
    for wrist in ["������.L_0104", "������.R_059"] {
        let grip = model.query("[name='" + wrist + "'] > [name='avatar_hand_grip_attachment']")
        if grip {
            let pose = grip.trs()
            let inverse_scale = 1.0 / model_scale
            grip.trs({ translation = pose.translation rotation_quat_xyzw = pose.rotation_quat_xyzw
                scale = [inverse_scale, inverse_scale, inverse_scale] })
        }
    }
}
