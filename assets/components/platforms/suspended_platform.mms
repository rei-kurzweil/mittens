// A narrow suspended walkway with four free-standing suspension cables.
// The platform's long axis is local Z, which makes instances straightforward
// to place end-to-end without rotating the prefab.

fn cable(x, z) {
    // Start each four-metre cable at the platform's top surface and let its
    // upper end disappear into the air above the prefab.
    return T.position(x, 2.025, z).scale(0.035, 4.0, 0.035) {
        R.cube() { C.rgba(0.16, 0.18, 0.21, 1.0) }
    }
}

export fn suspended_platform() {
    return T {
        name = "suspended_platform"

        T.scale(1.5, 0.05, 15.0) {
            name = "walkway"
            Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
            R.cube() { C.rgba(0.30, 0.32, 0.35, 1.0) }
        }

        // Z = +/-4.5 is 20% of the 15 m span in from either end. The X
        // positions put a cable near each edge at both suspension stations.
        cable(-0.65, -4.5)
        cable( 0.65, -4.5)
        cable(-0.65,  4.5)
        cable( 0.65,  4.5)
    }
}
