// Upright, yaw-safe rounded hull. Coarse solid-body contact, not wheel suspension.
// Asset wheel bottoms are Y=0.04635. The coarse hull reaches X=+/-4.40;
// the current car-8 mesh reaches X=+/-4.57 and Y=5.57.
// Overlapping spheres avoid the unsupported rotating moving-box path.
export fn display_car_sliding_colliders(body) {
    return T {
        name = "car_sliding_colliders"
        for side in [-1, 1] {
            T.position(side * 1.65, 0.54635, -0.53) {
                Zone.sphere(0.5) {
                    name = "car_slide_wheel"
                    Collidable.slide().movement_target(body) {}
                }
            }
        }
        for x in [-2.9, -1.45, 0, 1.45, 2.9] {
            T.position(x, 2.5, 0.5) {
                Zone.sphere(1.5) {
                    name = "car_slide_body"
                    Collidable.slide().movement_target(body) {}
                }
            }
        }
        T.position(0, 4.3, 0.3) {
            Zone.sphere(1.27) {
                name = "car_slide_roof"
                Collidable.slide().movement_target(body) {}
            }
        }
    }
}
