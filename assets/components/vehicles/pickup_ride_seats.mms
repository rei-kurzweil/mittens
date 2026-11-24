import { hold_rider_walking } from "./seat_rider_walking.mms"

// Ride-only seats: the passenger side of the cab and a forward-facing bench
// across the back of the bed. Mounting here never takes the driving controls.
// Chassis space, like pickup_driving. Occupancy is not replicated; each peer's
// rider is seen through their avatar pose.
//
// Each seat's Mountable goes on the truck root beside the driver's. A grip
// anywhere on the truck mounts the free seat whose entry zone holds the
// rider's head, so the zones must not overlap: the driver's covers the left
// door (x < -0.35, z < 0.75), the passenger's the right door, and the bed's
// the whole bed plus a margin outside its walls and tailgate (z > 0.8).
// Feet landing inside the bed mount the bench too, without a grip, so a
// player who jumps in rides the truck instead of sliding off once it moves.
fn bench_block(position, size) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { C.rgba(0.34, 0.27, 0.22, 1.0) }
    }
}

// Builders run only in the constructor chain, not on a live component, so
// each variant is its own chain; returning it from a function builds it.
fn ride_mountable(prefix, entry, seat, dismount, auto_zone) {
    if auto_zone {
        return Mountable.entry_zone(entry).mount_anchor(seat).dismount_anchor(dismount).auto_mount_zone(auto_zone).on_grip() {
            name=prefix+"_mountable"
        }
    }
    return Mountable.entry_zone(entry).mount_anchor(seat).dismount_anchor(dismount).on_grip() {
        name=prefix+"_mountable"
    }
}

fn ride_seat(prefix, entry_center, entry_half_extents, eye, exit, auto_zone) {
    let entry_frame=T.position(entry_center[0],entry_center[1],entry_center[2]) { name=prefix+"_entry_frame" }
    let entry=Zone.cube(entry_half_extents).at(entry_frame).role("vehicle_entry") { name=prefix+"_entry_zone" }
    let seat=T.position(eye[0],eye[1],eye[2]) { name=prefix+"_rider_eye_anchor" }
    let dismount=T.position(exit[0],exit[1],exit[2]) { name=prefix+"_dismount_anchor" }
    let mountable=ride_mountable(prefix, entry, seat, dismount, auto_zone)
    hold_rider_walking(mountable)
    // Bind with let so the frames are built and callers can attach to them;
    // a component written inline in a table stays an unbuilt expression.
    let frames=T { name=prefix+"_seat" entry_frame entry seat dismount }
    return { frames=frames mountable=mountable }
}

// Mirrors the driver's entry, eye and exit across the cab.
export fn pickup_passenger_seat() {
    return ride_seat("pickup_passenger", [1.2,1.05,-0.25], [0.85,1.05,1.0], [0.33,1.78,0.30], [1.95,1.65,-0.25], null)
}

// The bench sits behind the hot dog stand's fridge (z ~1.2-1.85). Its entry
// zone spans the bed (x +-0.78, z ~0.76-2.6) and reaches about 0.4 m past
// the walls and tailgate, from the roof up to a standing head in the bed.
// The auto-mount zone is the bed interior at foot height (floor ~0.6, walls
// to ~1.36), so standing beside the truck never mounts. The dismount anchor
// is behind the tailgate, outside it, so dismounting does not remount.
export fn pickup_bed_seat() {
    let feet_frame=T.position(0,0.95,1.68) { name="pickup_bed_feet_frame" }
    let feet=Zone.cube([0.74,0.5,0.88]).at(feet_frame) { name="pickup_bed_feet_zone" }
    let seat=ride_seat("pickup_bed", [0,1.4,1.9], [1.2,1.3,1.1], [0,1.70,2.30], [0,1.65,3.55], feet)
    // attach takes built components: bind each with let first.
    let bench=T {
        name="pickup_bed_bench"
        bench_block([0,0.735,2.38],[1.40,0.27,0.30])
        bench_block([0,0.91,2.35],[1.50,0.08,0.40])
    }
    seat.frames.attach(feet_frame)
    seat.frames.attach(feet)
    seat.frames.attach(bench)
    return seat
}
