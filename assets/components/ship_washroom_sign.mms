import { ship_box, ship_label } from "./ship_parts.mms"

// +Z faces the reader. A reusable, lighting-independent polygon pictogram.
fn person(kind,x) {
    return T.position(x,0,0.04) {
        T.position(0,0.19,0).scale(0.16,0.16,1) { R.circle_2d() { C.rgba(0.92,0.98,1,1) } }
        if kind == "women" {
            R.polygon("ship/sign/woman/v1",[[-0.065,0.075],[0.065,0.075],[0.16,-0.16],[-0.16,-0.16]]) { C.rgba(0.92,0.98,1,1) }
        } else {
            R.polygon("ship/sign/man/v1",[[-0.11,0.075],[0.11,0.075],[0.11,-0.12],[-0.11,-0.12]]) { C.rgba(0.92,0.98,1,1) }
        }
        for side in [-1,1] { ship_box([side*0.045,-0.22,0],[0.055,0.16,0.012],[0.92,0.98,1]) }
    }
}
export fn ship_washroom_sign(kind,direction,caption) {
    return T {
        name = "ship_washroom_sign"
        Unlit {}
        ship_box([0,0,-0.02],[1.60,0.72,0.06],[0.025,0.13,0.19])
        if kind == "washrooms" { person("women",-0.57) person("men",-0.23) }
        else { person(kind,-0.49) }
        if direction != "none" {
            let angle=0.0
            if direction=="left" {angle=Math.pi}
            if direction=="up" {angle=Math.pi/2}
            T.position(0.52,0.08,0.05).rotation(0,0,angle) {
                R.polygon("ship/sign/arrow/v1",[[-0.18,-0.04],[0.06,-0.04],[0.06,-0.12],[0.23,0],[0.06,0.12],[0.06,0.04],[-0.18,0.04]]) { C.rgba(0.92,0.98,1,1) }
            }
        }
        ship_label([-0.70,-0.32,0.05],caption,0.10)
    }
}
