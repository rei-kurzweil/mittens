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

// Inspection-scene prototype, shared by the studio-stage avatars for XR testing.
// Bias both elbows down; exclude the two observed left inward-bend regions.
// Right-arm exclusions remain empty until its bad angles are measured.
export fn vroid_arm_ik_bend_plane_prototype() {
    return {
        left = {
            pole_direction = [1.0, -1.5, 1.0]
            forbidden_bend_normal_z_degrees = [[-178.0, -115.0], [-100.0, -60.0]]
        }
        right = {
            pole_direction = [-1.0, -1.5, 1.0]
            forbidden_bend_normal_z_degrees = []
        }
    }
}
