import { ship_box } from "./ship_parts.mms"
import { orange, banana, toilet_paper_roll, peanut_butter_jar } from "./groceries.mms"

fn jar_grid(layer) {
    return T {
        for row in range(4) {
            for column in range(9) {
                T.position(-0.72 + column * 0.18, 0.151 + row * 0.47, 0.0) { peanut_butter_jar(layer) }
            }
        }
    }
}

export fn grocery_shelf_stock(kind, product_color) {
    let anime = Material(MaterialDefinition.anime())
    return T {
        if kind == "peanut_butter" {
            CombineMesh { anime jar_grid("body") jar_grid("lid") }
            CombineMesh {
                anime Texture.with_uri("assets/textures/groceries/peanut-butter-wrapper.png")
                TextureFiltering.linear()
                jar_grid("label")
            }
            CombineMesh {
                anime Texture.with_uri("assets/textures/groceries/peanut-butter-lid.png")
                TextureFiltering.linear()
                jar_grid("seal")
            }
        } else if kind != "empty" {
            CombineMesh {
                anime
                for row in range(4) {
                    let y = 0.151 + row * 0.47
                    if kind == "oranges" || kind == "bananas" {
                        // Open-top produce bin with separate front, back and sides.
                        ship_box([0.0, y, 0.0], [1.65, 0.025, 0.46], [0.38, 0.28, 0.13])
                        for z in [-0.23, 0.23] { ship_box([0.0, y + 0.065, z], [1.65, 0.13, 0.025], [0.48, 0.36, 0.17]) }
                        for x in [-0.82, 0.82] { ship_box([x, y + 0.065, 0.0], [0.025, 0.13, 0.46], [0.48, 0.36, 0.17]) }
                    }
                    if kind == "oranges" {
                        for layer in range(2) {
                            for column in range(10) {
                                for depth in range(2) {
                                    T.position(-0.69 + column * 0.15 + layer * 0.015, y + 0.09 + layer * 0.105, -0.09 + depth * 0.17) { orange() }
                                }
                            }
                        }
                    } else if kind == "bananas" {
                        for column in range(6) {
                            for bunch in range(3) {
                                T.position(-0.69 + column * 0.25, y + 0.17 + bunch * 0.026, -0.12)
                                    .rotation(0.45, bunch * 0.13, -0.18).scale(0.75, 0.75, 0.75) { banana() }
                            }
                        }
                    } else if kind == "toilet_paper" {
                        for layer in range(2) {
                            for column in range(12) {
                                for depth in range(3) {
                                    T.position(-0.72 + column * 0.13, y + layer * 0.11, -0.14 + depth * 0.14) { toilet_paper_roll() }
                                }
                            }
                        }
                    } else {
                        for column in range(6) {
                            ship_box([-0.72 + column * 0.29, y + 0.15, 0.0], [0.18, 0.30, 0.24], product_color)
                            ship_box([-0.72 + column * 0.29, y + 0.15, 0.125], [0.12, 0.10, 0.012], [0.88, 0.84, 0.65])
                        }
                    }
                }
            }
        }
    }
}
