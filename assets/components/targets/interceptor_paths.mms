import { mitten_route } from "../characters/mitten_equipment.mms"

// Scene-local XZ routes across the rooftop. Obstacles include room for the
// avatar around the closed walls, door jambs and the hull's footprint.
let obstacles = [
    [-10.55, -9.50, -12.65, 12.65], [9.50, 10.55, -12.65, 12.65],
    [-10.55, -6.35, -12.65, -11.50], [6.35, 10.55, -12.65, -11.50],
    [-10.55, -6.35, 11.50, 12.65], [6.35, 10.55, 11.50, 12.65],
    [-2.9, 2.65, -3.05, 3.2],
]

fn intersects(a, b, bounds) {
    let entry = 0.0
    let exit = 1.0
    for i in [0, 2] {
        let delta = b[i] - a[i]
        if Math.abs(delta) < 0.0001 {
            if a[i] < bounds[i] || a[i] > bounds[i + 1] { return false }
        } else {
            let low = (bounds[i] - a[i]) / delta
            let high = (bounds[i + 1] - a[i]) / delta
            if low > high {
                let swap = low
                low = high
                high = swap
            }
            if low > entry { entry = low }
            if high < exit { exit = high }
            if entry > exit { return false }
        }
    }
    return entry <= exit
}

fn clear_path(a, b) {
    if Math.abs(b[0]) > 13.3 || Math.abs(b[2]) > 16.3 { return false }
    for bounds in obstacles { if intersects(a, b, bounds) { return false } }
    return true
}

fn distance(a, b) {
    let x = b[0] - a[0]
    let z = b[2] - a[2]
    return Math.sqrt(x * x + z * z)
}

export fn interceptor_path_finding(roof_y) {
    let nodes = [[-11.35, roof_y, -13.5], [11.35, roof_y, -13.5],
        [-5.9, roof_y, -13.5], [5.9, roof_y, -13.5],
        [-5.9, roof_y, -5.0], [5.9, roof_y, -5.0],
        [-5.9, roof_y, 4.5], [5.9, roof_y, 4.5],
        [-5.9, roof_y, 13.5], [5.9, roof_y, 13.5],
        [-11.35, roof_y, 13.5], [11.35, roof_y, 13.5]]
    let edges = {}
    for i in range(len(nodes)) {
        for j in range(i) {
            if clear_path(nodes[i], nodes[j]) {
                let weight = distance(nodes[i], nodes[j])
                edges[i + ":" + j] = weight
                edges[j + ":" + i] = weight
            }
        }
    }

    // The two endpoints are added for this request. A* only runs when the
    // NPC chooses cover or equipment, with distances between fixed nodes cached.
    fn plan(here, goal) {
        if clear_path(here, goal) { return mitten_route(here, [goal]) }
        let start = len(nodes)
        let finish = start + 1
        let count = finish + 1
        fn point(i) {
            if i == start { return here }
            if i == finish { return goal }
            return nodes[i]
        }
        let scores = {}
        let closed = {}
        let parents = {}
        scores["" + start] = 0.0
        for pass in range(count) {
            let best = null
            let estimate = 1000000.0
            for i in range(count) {
                let traveled = scores["" + i]
                if traveled != null && !closed["" + i] {
                    let total = traveled + distance(point(i), goal)
                    if total < estimate { best = i estimate = total }
                }
            }
            if best == null { return null }
            if best == finish {
                let reverse = {}
                let size = 0
                let cursor = finish
                while cursor != start {
                    reverse["" + size] = point(cursor)
                    size = size + 1
                    cursor = parents["" + cursor]
                }
                return { count = size length = scores["" + finish]
                    point = fn(i) { return reverse["" + (size - i - 1)] } }
            }
            closed["" + best] = true
            for i in range(count) {
                if !closed["" + i] {
                    let weight = edges[best + ":" + i]
                    if (best == start || i == finish) && clear_path(point(best), point(i)) {
                        weight = distance(point(best), point(i))
                    }
                    if weight != null {
                        let traveled = scores["" + best] + weight
                        if scores["" + i] == null || traveled < scores["" + i] {
                            parents["" + i] = best
                            scores["" + i] = traveled
                        }
                    }
                }
            }
        }
        return null
    }

    fn retreat(here, player) {
        let side = 1.0
        if player[0] > 0.0 { side = -1.0 }
        let z = -9.4
        if here[2] > 0.0 { z = 9.4 }
        return plan(here, [side * 11.35, roof_y, z])
    }
    return { plan = plan retreat = retreat }
}
