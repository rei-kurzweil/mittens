import { ship_retail_store } from "./ship_retail_store.mms"

export fn ship_liquor_store() {
    return T {
        name = "ship_liquor_store"
        ship_retail_store("ORBITAL LIQUOR", [0.20, 0.39, 0.24])
    }
}
