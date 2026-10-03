// Two-bone arm IK policies for the VRoid humanoid bone layout.
// Pass each scene's existing body-local pole Y and Z to preserve its pose.
export fn vroid_arm_ik(pole_y, pole_z) {
    return {
        left = { pole_direction = [1.0, pole_y, pole_z] }
        right = { pole_direction = [-1.0, pole_y, pole_z] }
    }
}

// Preserves AVC's original unconfigured pole directions.
export fn vroid_arm_ik_defaults() {
    return {
        left = { pole_direction = [-1.0, 0.0, -1.0] }
        right = { pole_direction = [1.0, 0.0, -1.0] }
    }
}
