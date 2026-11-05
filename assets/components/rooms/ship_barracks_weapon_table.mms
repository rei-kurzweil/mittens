import { ship_solid } from "../ship_parts.mms"
import { ship_machine_pistol } from "../weapons/ship_machine_pistol.mms"

// Lightweight table used while the full inventory prefab is disabled for profiling.
export fn ship_barracks_weapon_table() {
    return T {
        name = "ship_barracks_weapon_table"
        ship_solid([-5.7,1.06,-2.35],[2.1,0.08,0.65],[0.64,0.58,0.48])
        for x in [-6.6,-4.8] {
            ship_solid([x,0.51,-2.35],[0.12,1.02,0.50],[0.29,0.34,0.31])
        }
        for index in range(3) {
            let x = -6.25 + index * 0.55
            T.position(x,1.15,-2.40) { ship_machine_pistol("ship_rack_pistol_"+index) }
        }
    }
}

