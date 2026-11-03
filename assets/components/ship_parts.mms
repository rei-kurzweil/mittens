import { emissive_surface as ship_emissive } from "./materials/emissive_surface.mms"

// Shared blockout helpers. Metres, Y up, equipment fronts face +Z.
export fn ship_box(position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) }
    }
}

export fn ship_solid(position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) Raycastable.enabled() }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
    }
}

export fn ship_label(position, text, size) {
    return T.position(position[0], position[1], position[2]).scale(size, size, size) {
        Text { text C.rgba(0.72, 0.95, 0.88, 1.0) Unlit {} }
    }
}

export fn ship_lamp(position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() { C.rgba(color[0], color[1], color[2], 1.0) ship_emissive() EM.on().intensity(1.2) }
    }
}

// All room floors have their top at Y=0. Doorways have 2.6 m clearance.
export fn ship_deck(center, width, length, ceiling_height, ceiling_lit) {
    if ceiling_height == null { ceiling_height = 3.30 }
    if ceiling_lit == null { ceiling_lit = true }
    return T {
        ship_solid([center[0], -0.10, center[1]], [width, 0.20, length], [0.19, 0.23, 0.25])
        ship_solid([center[0], ceiling_height, center[1]], [width, 0.16, length], [0.35, 0.39, 0.39])
        if ceiling_lit {
            ship_lamp([center[0], ceiling_height - 0.10, center[1]], [0.24, 0.04, length * 0.7], [0.73, 0.90, 0.88])
        }
    }
}

// Wall parallel to X, centred doorway; use a Y rotation for a side doorway.
export fn ship_door_wall(width) {
    return T {
        for side in [-1.0, 1.0] {
            ship_solid([side * (width / 4.0 + 0.65), 1.6, 0.0], [width / 2.0 - 1.3, 3.2, 0.16], [0.40, 0.46, 0.46])
        }
        ship_solid([0.0, 2.9, 0.0], [2.6, 0.6, 0.16], [0.40, 0.46, 0.46])
    }
}
