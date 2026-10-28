// Bisket's relaxed A-pose arm rotations, with Lumae's bind translations/scales.
// Leave the legs at Lumae's rest pose: copying Bisket's full capture would
// shorten Lumae's thighs (0.4388 m to 0.4062 m) and lift the feet off the roof.
export fn pose() {
    return PoseCapturePose.new("relaxed")
        .joint("#J_Bip_L_UpperArm", [-0.0000000020954757928848267, 0.06765186041593552, -0.00000012200325727462769], [-0.5505774617195129, 0.060794174671173096, 0.04025041684508324, 0.831593930721283], [1.0, 1.0, 0.9999998807907104])
        .joint("#J_Bip_R_UpperArm", [0.000000018044374883174896, 0.06765186041593552, -0.00000012293457984924316], [-0.5844499468803406, -0.05908013507723808, -0.04272674024105072, 0.8081472516059875], [1.0, 0.9999999403953552, 0.9999999403953552])
}
