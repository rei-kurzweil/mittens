// A forward-facing bench across the back of the bed. Riding only: mounting
// here never takes the driving controls. Chassis space, like pickup_driving.
// Occupancy is not replicated; each peer's rider is seen through their
// avatar pose. The bench sits behind the hot dog stand's fridge (z ~1.2-1.85).
//
// Grip the bench to ride. The Mountable lives on the bench, not the truck
// root: a grip resolves the first Mountable up the hit's ancestry, so the
// cab body keeps resolving to the driver's seat.
fn bench_block(position, size) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { C.rgba(0.34, 0.27, 0.22, 1.0) }
    }
}

export fn pickup_bed_seat() {
    let entry_frame=T.position(0,1.05,3.25) { name="pickup_bed_entry_frame" }
    let entry=Zone.cube([0.85,1.05,0.55]).at(entry_frame).role("vehicle_entry") { name="pickup_bed_entry_zone" }
    let seat=T.position(0,1.70,2.30) { name="pickup_bed_rider_eye_anchor" }
    let exit=T.position(0,1.65,3.55) { name="pickup_bed_dismount_anchor" }
    let mountable=Mountable.entry_zone(entry).mount_anchor(seat).dismount_anchor(exit).on_grip() {
        name="pickup_bed_mountable"
    }
    let bench=T {
        name="pickup_bed_bench"
        bench_block([0,0.735,2.38],[1.40,0.27,0.30])
        bench_block([0,0.91,2.35],[1.50,0.08,0.40])
        mountable
    }
    return T {name="pickup_bed_seat" entry_frame entry seat exit bench}
}
