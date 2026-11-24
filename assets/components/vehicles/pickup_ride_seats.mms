import { hold_rider_walking } from "./seat_rider_walking.mms"

// Ride-only places: the passenger seat in the cab, and the bed as a deck you
// stand and walk on. Neither ever takes the driving controls.
// Chassis space, like pickup_driving. Occupancy is not replicated; each peer's
// rider is seen through their avatar pose.
//
// Each Mountable goes on the truck root beside the driver's. A grip anywhere
// on the truck mounts the free seat whose entry zone holds the rider's head,
// so the zones must not overlap: the driver's covers the left door (x < -0.35,
// z < 0.75), the passenger's the right door, and the bed's the whole bed plus
// a margin outside its walls and tailgate (z > 0.8).
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

// A solid box in the bed's collision context, in chassis space.
fn deck_box(position, size) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
    }
}

// The bed is a carry deck, not a seat: feet entering it (x +-0.74, z ~0.8 to
// 2.56, from the floor at ~0.6 up to 2.2, so jumping in over the walls
// counts) put the rider on the truck's movement basis, and they keep walking,
// crouching and jumping there until they leave the bed's entry zone (the bed
// plus about 0.4 m past its walls and tailgate). While aboard they collide
// with this context's floor, walls, tailgate and bench, plus the world
// outside the truck, but not the truck's own hull.
// The deck frame and the context follow the chassis, so a rider tilts and
// lifts with the hydraulics as well as with the truck on a ramp; passthrough
// still skips the whole truck.
// Returns { frames, mountable, context }: attach frames and context to the
// chassis, and the mountable to the truck root.
export fn pickup_bed_deck(chassis, truck) {
    let board_frame=T.position(0,1.4,1.68) { name="pickup_bed_board_frame" }
    let board=Zone.cube([0.74,0.8,0.88]).at(board_frame) { name="pickup_bed_board_zone" }
    let entry_frame=T.position(0,1.4,1.9) { name="pickup_bed_entry_frame" }
    let entry=Zone.cube([1.2,1.3,1.1]).at(entry_frame).role("vehicle_entry") { name="pickup_bed_entry_zone" }
    let context=CollisionContext.passthrough(true).exclude(truck) {
        name="pickup_bed_context"
        deck_box([0,0.55,1.68],[1.60,0.10,1.84])
        for side in [-1,1] { deck_box([side*0.84,0.98,1.68],[0.12,0.76,1.84]) }
        // The cab's back wall stands taller, so the bed never leads into the cab.
        deck_box([0,1.30,0.70],[1.80,1.40,0.12])
        deck_box([0,0.98,2.66],[1.80,0.76,0.12])
        deck_box([0,0.775,2.36],[1.50,0.35,0.40])
    }
    let mountable=Mountable.carry().entry_zone(entry).auto_mount_zone(board).context(context).mount_anchor(chassis) {
        name="pickup_bed_mountable"
    }
    let bench=T {
        name="pickup_bed_bench"
        bench_block([0,0.735,2.38],[1.40,0.27,0.30])
        bench_block([0,0.91,2.35],[1.50,0.08,0.40])
    }
    let frames=T { name="pickup_bed_seat" board_frame board entry_frame entry bench }
    return { frames=frames mountable=mountable context=context }
}
