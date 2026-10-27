// Hair collision volumes scoped to each Lumae GLTF instance.
export fn lumae_colliders() {
    return SpringColliders {
        SpringCollider.sphere("[name='J_Bip_C_Head']", 0.10) { name = "lumae_collider_head" }
        SpringCollider.sphere("[name='J_Bip_C_Neck']", 0.055) { name = "lumae_collider_neck" }
        SpringCollider.sphere("[name='J_Bip_C_UpperChest']", 0.08) { name = "lumae_collider_upper_chest" }
        SpringCollider.sphere("[name='J_Bip_C_Spine']", 0.09) { name = "lumae_collider_spine" }
        SpringCollider.spheres(["[name='J_Bip_L_Hand']", "[name='J_Bip_R_Hand']"], 0.075) { name = "lumae_colliders_hands" }
        SpringCollider.spheres(["[name='J_Bip_L_LowerArm']", "[name='J_Bip_R_LowerArm']"], 0.04) { name = "lumae_colliders_lower_arms" }
        SpringCollider.spheres(["[name='J_Bip_L_UpperArm']", "[name='J_Bip_R_UpperArm']"], 0.045) { name = "lumae_colliders_upper_arms" }
    }
}
