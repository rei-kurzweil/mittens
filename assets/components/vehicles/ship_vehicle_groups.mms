import { recreational_vehicle } from "./recreational_vehicle.mms"
import { pickup_truck } from "./pickup_truck.mms"
import { motorcycle } from "./motorcycle.mms"
import { lumae_motorcycle_patrol } from "./lumae_motorcycle_patrol.mms"

export fn ship_motorhome_group() {
    return T { name = "ship_motorhome_group"
        // Four motorhomes, including visibly extended lounge sections.
        for x in [-6.5, 6.5] {
            T.position(x, 0.0, 5.0) { recreational_vehicle(0.9) }
            T.position(x, 0.0, -6.0).rotation(0.0, Math.pi, 0.0) { recreational_vehicle(0.0) }
        }
    }
}
export fn ship_driving_vehicle_group() {
    return T { name = "ship_driving_vehicle_group"
        T.position(0.0, 0.0, 5.0) { pickup_truck() }
        T.position(0.0, 0.0, -6.0).rotation(0.0, Math.pi, 0.0) { pickup_truck() }
        T.position(-10.5, 0.0, 5.0) { pickup_truck() }
        lumae_motorcycle_patrol()
        T.position(10.2, 0.0, 7.5) { motorcycle([0.22, 0.38, 0.32]) }
    }
}
