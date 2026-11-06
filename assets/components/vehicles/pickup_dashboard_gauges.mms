import { ship_box, ship_solid, ship_label } from "../ship_parts.mms"
import { ship_environment_material } from "../materials/ship_anime_materials.mms"

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

// Original dashboard geometry, reusable within the procedural truck's body batch.
export fn pickup_gauge_geometry() {
    return T {
        ship_solid([-0.45,1.50,-0.905],[0.88,0.33,0.08],[0.12,0.14,0.12])
        dial("rpm",-0.67,1.52,0.105,-0.8)
        dial("speed",-0.40,1.52,0.105,0.8)
        dial("fuel",-0.17,1.50,0.067,-0.35)
        T { name = "pickup_gauge_odometer"
            ship_box([-0.41,1.255,-0.818],[0.24,0.065,0.025],[0.045,0.065,0.055])
        }
    }
}

// Standalone cluster for the imported truck, without the old cab or steering wheel.
export fn pickup_dashboard_gauges() {
    let root=T.position(-0.45,1.54,-0.55) {
        name="pickup_dashboard_gauges"
        T.position(0.45,-1.52,0.85) {
            Selectable.off()
            CombineMesh.keep_transforms() {
                name="pickup_dashboard_gauge_mesh"
                ship_environment_material()
                pickup_gauge_geometry()
            }
            ship_label([-0.53,1.235,-0.797],"00000.0 km",0.025)
            ship_label([-0.77,1.66,-0.80],"RPM     SPEED    FUEL",0.026)
        }
    }
    // Retain the placement authored with the temporary layout gizmos.
    root.trs({translation=[-0.44999998807907104,1.336553692817688,-0.5249140858650208]
        rotation_quat_xyzw=[0.002408880740404129,0.000028632610337808728,0.0010148161090910435,0.9999964833259583]
        scale=[1,1,1]})
    return root
}
