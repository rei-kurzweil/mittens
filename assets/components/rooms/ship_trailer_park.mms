import { ship_biodome } from "../ship_biodome.mms"
import { ship_box, ship_solid, ship_deck, ship_label, ship_lamp } from "../ship_parts.mms"
import { ship_path_markers, ship_light_pool } from "../ship_lighting.mms"
import { recreational_vehicle } from "../vehicles/recreational_vehicle.mms"
import { pickup_truck } from "../vehicles/pickup_truck.mms"
import { motorcycle } from "../vehicles/motorcycle.mms"
import { lumae_motorcycle_patrol } from "../vehicles/lumae_motorcycle_patrol.mms"
import { ship_convenience_store } from "./ship_convenience_store.mms"
import { ship_liquor_store } from "./ship_liquor_store.mms"

// Larger vehicle biodome beyond Cherry Grove; west entrance [-12,0,-1].
// Dome spans the 24 x 32 m vehicle lot and rear shops.
export fn ship_trailer_park() {
    return T {
        name = "ship_trailer_park"
        ship_solid([0,-0.1,3], [24,0.2,26], [0.19,0.23,0.25])
        T.position(0,0,-1) { ship_biodome("ship_vehicle_biodome",24,36,3.0,8.0,12,-1) }
        for z in [-10.0,16.0] {
            ship_solid([0,0.55,z], [24,1.1,0.16], [0.25,0.30,0.28])
        }
        ship_solid([12,0.55,3], [0.16,1.1,26], [0.25,0.30,0.28])
        for z in [-6.1,8.1] {
            let length = 7.8
            if z > 0 { length = 15.8 }
            ship_solid([-12,0.55,z], [0.16,1.1,length], [0.25,0.30,0.28])
        }
        ship_box([0.0, 0.01, -1.0], [24.0, 0.02, 2.0], [0.37, 0.38, 0.30])
        for z in [-2.1, 0.1] {
            ship_path_markers([-10.5, 0.04, z], [1.0, 0.0], 12, 1.9, [0.64, 0.49, 0.22])
        }
        // Four motorhomes, including visibly extended lounge sections.
        for x in [-6.5, 6.5] {
            T.position(x, 0.0, 5.0) { recreational_vehicle(0.9) }
            T.position(x, 0.0, -6.0).rotation(0.0, Math.pi, 0.0) { recreational_vehicle(0.0) }
        }
        T.position(0.0, 0.0, 5.0) { pickup_truck() }
        T.position(0.0, 0.0, -6.0).rotation(0.0, Math.pi, 0.0) { pickup_truck() }
        T.position(-10.5, 0.0, 5.0) { pickup_truck() }
        lumae_motorcycle_patrol()
        T.position(10.2, 0.0, 7.5) { motorcycle([0.22, 0.38, 0.32]) }
        for x in [-8.0, 8.0] {
            ship_lamp([x, 4.55, -1.0], [1.2, 0.06, 0.30], [0.70, 0.63, 0.42])
            ship_light_pool([x, 3.8, -1.0], [0.84, 0.75, 0.53], 2.0, 6.0)
        }
        // Service walkway remains open between the parked vehicles and shops.
        ship_box([0.0, 0.015, 9.4], [24.0, 0.03, 1.2], [0.48, 0.47, 0.35])
        T.position(-6.0, 0.0, 10.0) { ship_convenience_store() }
        T.position(6.0, 0.0, 10.0) { ship_liquor_store() }
        T.position(-11.8, 2.0, -1.0).rotation(0.0, -Math.pi / 2.0, 0.0) {
            ship_label([-0.9, 0.0, 0.0], "DECKSIDE TRAILER PARK", 0.15)
        }
    }
}
