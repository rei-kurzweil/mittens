// Existing native Velocity integrates the vehicle. Forward commands are resolved
// against its current heading; the ship controller refreshes them while turning.
export fn ship_vehicle_motion(body, label) {
    let motion = Velocity.rotation_basis(body) { name=label body }
    body.velocity = motion
    return motion
}
