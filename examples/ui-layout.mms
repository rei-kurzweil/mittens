BGC {
    C.rgba(0.9, 0.9, 0.9, 1.0)
}

BG {
    T.position(0.0, 0.0, -5.0).scale(10.0, 10.0, 1.0) {
        R.cube() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
        }
    }
    T.position(0.0, 0.0,  4.9).scale(9.5, 9.5, 1.0) {
        R.cube() {
            C.rgba(0.8, 0.8, 0.8, 1.0)

        }
    }
}


I {
    speed(1.0)
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }
    T.position(0.0, 1.2, 3.5) {
        C3D {}
        Pointer {}
    }
}

// scrollable view:
// T.position(0, 0, -5.0) {
//     Scrolling.new(1.0, 20) {
//         for y in range(100) {
//             T.position(0, y, 0.0).scale(0.9, 0.9, 0.9) {
//                 Text {
//                     "item "+y
//                     C.rgba(0.6, 0.6, 0.6, 1.0)
//                 }
//             }
//         }
//     }
// }

// lighting
AL {
    C.rgba(0.04, 0.04, 0.06, 1.0)
}
T.position(-1, -1, 0) {
    DL {
        intensity(1.1)
        C.rgba(1.0, 0.92, 0.78, 1.0)
    }
}

// perimeter cubes
T.position(0.0, 0.0, -5.0) {
    for x in range(-5, 6) {
        for y in range(-5, 6) {
            if x % 2 == 0 && y % 2 == 0 {
                T.position(x, y, 0.0).scale(0.9, 0.9, 0.9) {
                    R.cube() {
                        C.rgba(1.0, 0.6, 0.6, 1.0)
                        Emissive.on()
                    }
                }
            }
        }
    }
}