import { ship_label } from "../ship_parts.mms"

fn comparison_button(index,trucks) {
    let title = "REST"
    if index == 1 { title = "COMPRESS" }
    if index == 2 { title = "LEFT BUMP" }
    let button = T.position(index*0.88,0,0) {
        name = "pickup_suspension_compare_" + index
        T.scale(0.80,0.26,0.06) {
            R.cube() {
                C.rgba(0.20,0.33,0.34,1)
                Raycastable.click_only() { interaction_priority(120) }
            }
        }
        ship_label([-0.34,-0.025,0.04],title,0.055)
    }
    on(button,"Click",fn(event) {
        for truck in trucks {
            truck.set_suspension_compression(null,1.0)
            if index == 1 { truck.set_suspension_compression(null,0.50) }
            if index == 2 {
                truck.set_suspension_compression("fl",0.50)
                truck.set_suspension_compression("rl",0.50)
            }
        }
    })
    return button
}

// One shared pose control compares the two hierarchies without running physics.
export fn pickup_suspension_comparison(fork_vehicle, sibling_vehicle) {
    let trucks = [fork_vehicle.query("#pickup_truck"),sibling_vehicle.query("#pickup_truck")]
    return T {
        name = "pickup_suspension_comparison"
        ship_label([-0.4,0.35,0],"PICKUP SUSPENSION A / B",0.075)
        for index in range(3) { comparison_button(index,trucks) }
    }
}
