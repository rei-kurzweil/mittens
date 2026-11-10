// Mitten relaxed pose: upper arms 15 degrees from the sides (75 below horizontal) in the model's rest frame.
// Captures only upper arms; preserve mitten.glb bind-local translations/scales.
// Direct GLTF children overlay once at import, so tracked arm IK can take over.
export fn pose() {
    return PoseCapturePose.new("mitten-relaxed-arms")
        .joint("#J_Bip_L_UpperArm", [-0.0000000023283064, 0.07559478282928467, 0.0000000642612576], [-0.547834805619, 0.060926454520, 0.040049989917, 0.833403258158], [1.0, 0.9999999403953552, 1.0000001192092896])
        .joint("#J_Bip_R_UpperArm", [-0.0000000155996531, 0.07559477537870407, 0.0000000651925802], [-0.547834759559, -0.060926497714, -0.040050069902, 0.833403317686], [0.9999999403953552, 0.9999999403953552, 1.0])
}
