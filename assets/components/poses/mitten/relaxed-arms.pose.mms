// Mitten A-pose: upper arms 45 degrees below horizontal in the model's rest frame.
// Captures only upper arms; preserve mitten.glb bind-local translations/scales.
// Direct GLTF children overlay once at import, so tracked arm IK can take over.
export fn pose() {
    return PoseCapturePose.new("mitten-relaxed-arms")
        .joint("#J_Bip_L_UpperArm", [-0.0000000023283064, 0.07559478282928467, 0.0000000642612576], [-0.313467152535, 0.069216120496, 0.022916383126, 0.946795813172], [1.0, 0.9999999403953552, 1.0000001192092896])
        .joint("#J_Bip_R_UpperArm", [-0.0000000155996531, 0.07559477537870407, 0.0000000651925802], [-0.313467092012, -0.069216172347, -0.022916457766, 0.946795859523], [0.9999999403953552, 0.9999999403953552, 1.0])
}
