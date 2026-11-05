// Rounded chassis contact approximation using rotation-safe existing spheres.
// All zones correct the same vehicle root; these do not simulate suspension.
export fn pickup_sliding_colliders(truck) {
    return T {
        name="pickup_sliding_colliders"
        for index in range(3) {
            T.position(0,0.87,-0.70+index*1.25) {
                Zone.sphere(0.85) {
                    name="pickup_slide_chassis_"+index
                    Collidable.slide().movement_target(truck) {}
                }
            }
        }
        T.position(0,1.50,0.10) {
            Zone.sphere(0.60) {
                name="pickup_slide_cab"
                Collidable.slide().movement_target(truck) {}
            }
        }
    }
}
