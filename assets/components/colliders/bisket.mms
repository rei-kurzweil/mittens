// Explicit Bisket spring-bone collision volumes. These target authored skeleton
// nodes only; imported nodes whose names contain "collider" are intentionally
// ignored by SecondaryMotionSystem.
export fn bisket_colliders() {
    return SpringColliders {
        SpringCollider.sphere("[name='J_Bip_C_Head']", 0.11) { name = "bisket_collider_head" }
        SpringCollider.sphere("[name='J_Bip_C_Neck']", 0.055) { name = "bisket_collider_neck" }
        SpringCollider.sphere("[name='J_Bip_C_UpperChest']", 0.065) { name = "bisket_collider_upper_chest" }
        SpringCollider.sphere("[name='J_Bip_C_Spine']", 0.09) { name = "bisket_collider_spine" }
        SpringCollider.spheres(["[name='J_Bip_L_Hand']", "[name='J_Bip_R_Hand']"], 0.045) { name = "bisket_colliders_hands" }
        SpringCollider.spheres(["[name='J_Bip_L_LowerArm']", "[name='J_Bip_R_LowerArm']"], 0.04) { name = "bisket_colliders_lower_arms" }
        SpringCollider.spheres(["[name='J_Bip_L_UpperArm']", "[name='J_Bip_R_UpperArm']"], 0.05) { name = "bisket_colliders_upper_arms" }
        SpringCollider.spheres(["[name='J_Bip_L_UpperLeg']", "[name='J_Bip_R_UpperLeg']"], 0.075) { name = "bisket_colliders_upper_legs" }
        // Bisket has no authored hips collider node, so use the hips bone center.
        SpringCollider.sphere("[name='J_Bip_C_Hips']", 0.11) { name = "bisket_collider_hips" }
    }
}
