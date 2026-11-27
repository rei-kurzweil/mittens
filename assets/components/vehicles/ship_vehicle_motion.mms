// Existing native Velocity integrates the vehicle. Forward commands are resolved
// against its current heading; the ship controller refreshes them while turning.
// Rocket blasts push it away and turn it, staying upright (Blastable).
export fn ship_vehicle_motion(body, label) {
    let motion = Velocity.rotation_basis(body) {
        name=label
        body
        Blastable.mass(1.6).spin(0.3) { name=label+"_blastable" }
    }
    body.velocity = motion
    return motion
}
