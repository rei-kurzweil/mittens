import { ship_box, ship_solid } from "./ship_parts.mms"
import { grocery_shelf_stock } from "./grocery_shelf_stock.mms"

// Four horizontal product shelves, front toward +Z.
export fn retail_shelf(product_color, stock) {
    if product_color == null { product_color = [0.66, 0.40, 0.16] }
    if stock == null { stock = "packages" }
    return T {
        name = "retail_shelf"
        ship_solid([0.0, 0.95, -0.23], [1.8, 1.9, 0.08], [0.32, 0.38, 0.34])
        CombineMesh {
            for row in range(4) {
                let y = 0.12 + row * 0.47
                ship_box([0.0, y, 0.0], [1.8, 0.055, 0.52], [0.62, 0.65, 0.54])
            }
        }
        grocery_shelf_stock(stock, product_color)
    }
}
