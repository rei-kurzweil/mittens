// Static contact proxies in normalized vehicle coordinates; cab sides stay open.
fn box(position, size) {
    return T.position(position[0],position[1],position[2]).scale(size[0],size[1],size[2]) {
        Zone.cube([0.5,0.5,0.5]) { Collidable.static().friction(0.8) {} }
    }
}

export fn pickup_model_colliders() {
    return T {
        name="pickup_model_colliders"
        box([0,0.49,0.52],[1.72,0.18,4.10])
        box([0,0.85,-1.02],[1.72,0.48,1.0])
        // Closed imported parts receive their own triangle collision. Keep
        // proxies only for the excluded lower cabin, so cut panels leave holes.
    }
}
