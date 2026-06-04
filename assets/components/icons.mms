// assets/components/icons.mms — basic UI icons (=^･ω･^=)

export fn pencil_icon() {
    return T {
        name = "pencil_icon"
        // Simplified pencil: a tilted rectangle
        T.position(0.0, 0.0, 0.0) {
            T.rotation(0.0, 0.0, 0.785) { // 45 degrees
                T.scale(0.3, 1.2, 0.1) {
                    R.cube() {
                        C.rgba(0.8, 0.6, 0.4, 1.0)
                    }
                }
            }
        }
    }
}

export fn line_icon() {
    return T {
        name = "line_icon"
        T.position(0.0, 0.0, 0.0) {
            T.rotation(0.0, 0.0, 0.785) {
                T.scale(0.1, 1.8, 0.1) {
                    R.cube() {
                        C.rgba(0.9, 0.9, 0.9, 1.0)
                    }
                }
            }
        }
    }
}

export fn spray_can_icon() {
    return T {
        name = "spray_can_icon"
        // Body
        T.scale(0.8, 1.2, 0.1) {
            R.cube() { C.rgba(0.7, 0.2, 0.2, 1.0) }
        }
        // Cap
        T.position(0.0, 0.8, 0.01) {
            T.scale(0.4, 0.4, 0.1) {
                R.cube() { C.rgba(0.2, 0.2, 0.2, 1.0) }
            }
        }
    }
}

export fn fill_icon() {
    return T {
        name = "fill_icon"
        // Tilted bucket-ish shape
        T.rotation(0.0, 0.0, -0.4) {
            T.scale(1.2, 1.0, 0.1) {
                R.cube() { C.rgba(0.2, 0.6, 0.9, 1.0) }
            }
        }
    }
}

export fn erase_icon() {
    return T {
        name = "erase_icon"
        T.scale(1.2, 0.6, 0.1) {
            R.cube() { C.rgba(0.9, 0.4, 0.6, 1.0) }
        }
    }
}
