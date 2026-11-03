import { ship_retail_store } from "./ship_retail_store.mms"

export fn ship_convenience_store() {
    return T {
        name = "ship_convenience_store"
        ship_retail_store("DECKSIDE CONVENIENCE", [0.66, 0.40, 0.16], true)
    }
}
