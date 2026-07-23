import { bisket_secondary_motion_with } from "bisket.mms"

// Canonical Bisket shirt hem. These six skin joints are leaves, so each chain
// extrapolates a virtual tail from the joint's rest-space direction.
export fn bisket_shirt_physics(config) {
    let shirt_chains = [
        { root = "[name='J_Sec_L_TopsUpperLegBack_01']" virtual_end_length_ratio = 1.0 stiffness = 2.0 drag_force = 0.35 gravity_power = 3.0 gravity_dir = [0, -1, 0] },
        { root = "[name='J_Sec_L_TopsUpperLegFront_01']" virtual_end_length_ratio = 1.0 stiffness = 2.0 drag_force = 0.35 gravity_power = 3.0 gravity_dir = [0, -1, 0] },
        { root = "[name='J_Sec_L_TopsUpperLegSide_01']" virtual_end_length_ratio = 1.0 stiffness = 2.0 drag_force = 0.35 gravity_power = 3.0 gravity_dir = [0, -1, 0] },
        { root = "[name='J_Sec_R_TopsUpperLegBack_01']" virtual_end_length_ratio = 1.0 stiffness = 2.0 drag_force = 0.35 gravity_power = 3.0 gravity_dir = [0, -1, 0] },
        { root = "[name='J_Sec_R_TopsUpperLegFront_01']" virtual_end_length_ratio = 1.0 stiffness = 2.0 drag_force = 0.35 gravity_power = 3.0 gravity_dir = [0, -1, 0] },
        { root = "[name='J_Sec_R_TopsUpperLegSide_01']" virtual_end_length_ratio = 1.0 stiffness = 2.0 drag_force = 0.35 gravity_power = 3.0 gravity_dir = [0, -1, 0] },
    ]
    return bisket_secondary_motion_with(config, shirt_chains)
}
