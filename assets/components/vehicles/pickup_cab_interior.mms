import { ship_solid, ship_box } from "../ship_parts.mms"

fn dial(label,x,y,radius,angle) {
    return T.position(x,y,-0.85) {
        name = "pickup_gauge_" + label
        T.scale(radius*2,radius*2,1) { R.circle_2d() { C.rgba(0.035,0.045,0.04,1) } }
        for mark in range(9) {
            let a = -2.3+mark*0.575
            T.position(Math.sin(a)*radius*0.80,Math.cos(a)*radius*0.80,0.005).rotation(0,0,-a) {
                ship_box([0,0,0],[0.006,radius*0.18,0.008],[0.78,0.83,0.71])
            }
        }
        T.rotation(0,0,angle) { ship_box([0,radius*0.35,0.012],[0.006,radius*0.70,0.01],[0.95,0.35,0.08]) }
    }
}

// All geometry is emitted into the caller's retained body batch. -Z is forward.
export fn pickup_cab_interior() {
    return T {
        name = "pickup_cab_interior"
        for side in [-1,1] {
            T.position(side*0.48,0,0) {
                name = "pickup_front_seat_" + side
                ship_solid([0,0.99,-0.08],[0.66,0.18,0.64],[0.16,0.17,0.15])
                ship_solid([0,1.38,0.20],[0.66,0.72,0.14],[0.19,0.20,0.17])
                ship_solid([0,1.82,0.20],[0.38,0.22,0.14],[0.16,0.17,0.15])
                ship_box([0,0.88,-0.08],[0.49,0.08,0.43],[0.30,0.32,0.29])
            }
        }
        // Dashboard surrounds an open DIN-sized radio recess on the right.
        ship_solid([-0.45,1.28,-0.98],[0.98,0.28,0.30],[0.22,0.24,0.21])
        ship_solid([0.47,1.13,-0.98],[0.86,0.06,0.30],[0.22,0.24,0.21])
        ship_solid([0.47,1.42,-0.98],[0.86,0.07,0.30],[0.22,0.24,0.21])
        for x in [0.04,0.90] { ship_solid([x,1.28,-0.98],[0.05,0.26,0.30],[0.22,0.24,0.21]) }
        T { name = "pickup_radio_recess"
            ship_box([0.47,1.28,-1.12],[0.78,0.20,0.015],[0.055,0.065,0.055])
        }
        // Three paired mounting rails on the top dashboard, ahead of the recess.
        for index in range(3) {
            T.position(0.15+index*0.28,1.48,-0.99) {
                name = "pickup_dashboard_mount_" + index
                for side in [-1,1] { ship_box([side*0.035,0,0],[0.018,0.035,0.18],[0.42,0.45,0.40]) }
            }
        }
        ship_solid([-0.45,1.50,-0.905],[0.88,0.33,0.08],[0.12,0.14,0.12])
        dial("rpm",-0.67,1.52,0.105,-0.8)
        dial("speed",-0.40,1.52,0.105,0.8)
        dial("fuel",-0.17,1.50,0.067,-0.35)
        T { name = "pickup_gauge_odometer"
            ship_box([-0.41,1.255,-0.818],[0.24,0.065,0.025],[0.045,0.065,0.055])
        }
        // Wheel, column and spokes face the seated driver.
        T.position(-0.48,1.43,-0.57).rotation(Math.pi/2.0-0.30,0,0) {
            name = "pickup_steering_wheel"
            R.helix(0.16,0.20,0,0,Math.pi*2,24,0.025) { C.rgba(0.12,0.14,0.12,1) }
            for angle in [0,2.0944,4.1888] {
                T.rotation(0,angle,0) { ship_box([0.08,0,0],[0.16,0.035,0.035],[0.42,0.45,0.40]) }
            }
            ship_box([0,0,0],[0.075,0.055,0.075],[0.19,0.21,0.18])
        }
        T { name = "pickup_accelerator_pedal"
            ship_box([-0.30,0.87,-0.92],[0.09,0.05,0.19],[0.10,0.12,0.10])
        }
        T { name = "pickup_brake_pedal"
            ship_box([-0.58,0.88,-0.92],[0.17,0.06,0.16],[0.10,0.12,0.10])
        }
        T { name = "pickup_gear_shift"
            ship_box([0,1.0,-0.24],[0.16,0.16,0.26],[0.16,0.18,0.15])
            ship_box([0,1.18,-0.24],[0.025,0.26,0.025],[0.48,0.51,0.46])
            ship_box([0,1.32,-0.24],[0.085,0.065,0.085],[0.12,0.14,0.12])
        }
    }
}
