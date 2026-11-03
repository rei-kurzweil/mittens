import { ship_box, ship_lamp } from "./ship_parts.mms"

// Desk-top CRT, origin on desk, screen toward +Z.
export fn crt_monitor() {
    return T {
        name = "crt_monitor"
        ship_box([0.0, 0.035, 0.0], [0.46, 0.07, 0.36], [0.32, 0.35, 0.32])
        ship_box([0.0, 0.28, -0.06], [0.60, 0.46, 0.48], [0.58, 0.60, 0.49])
        ship_box([0.0, 0.29, 0.185], [0.51, 0.36, 0.025], [0.025, 0.045, 0.038])
        ship_lamp([0.0, 0.30, 0.202], [0.44, 0.28, 0.012], [0.035, 0.23, 0.15])
        for row in range(5) {
            ship_lamp([-0.05, 0.39 - row * 0.042, 0.211], [0.27 - row * 0.025, 0.008, 0.004], [0.32, 0.90, 0.50])
        }
        ship_lamp([0.23, 0.10, 0.204], [0.025, 0.015, 0.008], [0.40, 1.0, 0.18])
    }
}
