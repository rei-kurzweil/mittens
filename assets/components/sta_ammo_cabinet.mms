import { sta_rocket_ammo } from "./projectiles/sta_rocket_ammo.mms"

fn panel(position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

export fn sta_ammo_cabinet(label) {
    let frame = [0.27, 0.30, 0.33]
    let shelves = [0.43, 0.46, 0.49]
    return T {
        name = label
        panel([0.0, 0.98, -0.18], [0.96, 1.96, 0.05], frame)
        for side in [-1.0, 1.0] {
            panel([side * 0.46, 0.98, 0.01], [0.04, 1.96, 0.43], frame)
        }
        panel([0.0, 1.94, 0.01], [0.88, 0.04, 0.43], frame)
        panel([0.0, 0.075, 0.01], [0.88, 0.15, 0.43], frame)
        for row in range(4) {
            T.position(0.0, 0.18 + 0.43 * row, 0.0) {
                name = label + "_shelf_" + row
                panel([0.0, -0.025, 0.01], [0.88, 0.05, 0.43], shelves)
                for column in range(5) {
                    T.position((column - 2) * 0.15, 0.179, 0.10).rotation(Math.pi / 2, 0.0, 0.0) {
                        sta_rocket_ammo(label + "_round_" + row + "_" + column)
                    }
                }
            }
        }
    }
}
