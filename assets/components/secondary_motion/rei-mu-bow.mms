// Rei(mu)'s two ribbon halves are separate eight-joint chains under head_bow.
// The ribbon mesh in the updated GLB has weights on both chains.
export fn rei_mu_bow_secondary_motion() {
    return SecondaryMotion {
        SpringBone.from_root("[name='head_bow.001']")
            .virtual_end_length_ratio(1.0)
            .stiffness(1.5)
            .drag_force(0.45)
            .gravity(1.5, [0, -1, 0])
            .colliders(["[name='bisket_collider_head']"])
            .hit_radius(0.01) {
                ReturnToRestWhenStill {
                    motion_threshold(0.02)
                    still_for(0.4)
                }
            }
        SpringBone.from_root("[name='head_bow.009']")
            .virtual_end_length_ratio(1.0)
            .stiffness(1.5)
            .drag_force(0.45)
            .gravity(1.5, [0, -1, 0])
            .colliders(["[name='bisket_collider_head']"])
            .hit_radius(0.01) {
                ReturnToRestWhenStill {
                    motion_threshold(0.02)
                    still_for(0.4)
                }
            }
    }
}
