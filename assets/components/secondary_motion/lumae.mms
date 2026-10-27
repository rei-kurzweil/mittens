// Inspected assets/models/lumae.glb: 19 independent, unbranched hair chains
// under J_Bip_C_Head (87 skin joints total). Chain lengths by suffix:
// 01..08: 3; 09: 7; 10..14: 6; 15..16: 5; 17..18: 6; 19: 4.
// Discover the full skin chains instead of truncating them to Bisket's tips.
// Lumae has no tail; bust joints stay in their authored rest pose.
export fn lumae_secondary_motion() {
    let short_hair_roots = [
        "[name='J_Sec_Hair1_01']", "[name='J_Sec_Hair1_02']",
        "[name='J_Sec_Hair1_03']", "[name='J_Sec_Hair1_04']",
        "[name='J_Sec_Hair1_05']", "[name='J_Sec_Hair1_06']",
        "[name='J_Sec_Hair1_07']", "[name='J_Sec_Hair1_08']",
    ]
    let long_hair_roots = [
        "[name='J_Sec_Hair1_09']", "[name='J_Sec_Hair1_10']",
        "[name='J_Sec_Hair1_11']", "[name='J_Sec_Hair1_12']",
        "[name='J_Sec_Hair1_13']", "[name='J_Sec_Hair1_14']",
        "[name='J_Sec_Hair1_15']", "[name='J_Sec_Hair1_16']",
        "[name='J_Sec_Hair1_17']", "[name='J_Sec_Hair1_18']",
        "[name='J_Sec_Hair1_19']",
    ]
    let hair_colliders = [
        "[name='lumae_collider_head']", "[name='lumae_collider_neck']",
        "[name='lumae_collider_upper_chest']", "[name='lumae_collider_spine']",
        "[name='lumae_colliders_hands']", "[name='lumae_colliders_lower_arms']",
        "[name='lumae_colliders_upper_arms']",
    ]
    return SecondaryMotion {
        for root in short_hair_roots {
            SpringBone.from_root(root)
                .virtual_end_length_ratio(1.0)
                .stiffness(1.0)
                .drag_force(0.35)
                .gravity(3.0, [0, -1, 0])
                .colliders(hair_colliders)
                .hit_radius(0.012)
        }
        // More damping and less sag preserve the longer locks' authored curls.
        for root in long_hair_roots {
            SpringBone.from_root(root)
                .virtual_end_length_ratio(1.0)
                .stiffness(1.5)
                .drag_force(0.45)
                .gravity(0.8, [0, -1, 0])
                .colliders(hair_colliders)
                .hit_radius(0.015)
        }
    }
}
