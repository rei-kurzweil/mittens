import { ship_avatar_material, ship_face_material } from "./materials/ship_anime_materials.mms"
import { ship_solid, ship_box, ship_label } from "./ship_parts.mms"
import { ship_light_pool, ship_path_markers } from "./ship_lighting.mms"
import { pickup_truck } from "./vehicles/pickup_truck.mms"
import { mini_fridge } from "./mini_fridge.mms"
import { hot_dog_grill } from "./hot_dog_grill.mms"
import { fountain_drink_machine } from "./fountain_drink_machine.mms"
import { hot_dog } from "./food/hot_dog.mms"
import { pose as vendor_pose } from "./characters/lumae/poses/hot-dog-vendor.pose.mms"

// Open floor kiosk, front -Z. Origin is the grill centre. The truck sits to
// its left, alongside the wall; it does not need a restaurant bay or new room.
export fn hot_dog_stand() {
    return T {
        name = "ship_hot_dog_stand"
        T.position(-2.2, 0.0, -1.1) { pickup_truck(null, mini_fridge(), null, true) }
        T.rotation(0.0, Math.pi, 0.0) { hot_dog_grill() }
        ship_solid([1.30, 0.47, 0.0], [1.20, 0.94, 0.72], [0.48, 0.42, 0.29])
        T.position(1.30, 0.94, 0.0).rotation(0.0, Math.pi, 0.0) { fountain_drink_machine() }
        T.position(0.0, 2.45, -0.50).rotation(0.0, Math.pi, 0.0) {
            ship_box([0.0, 0.0, -0.02], [2.5, 0.46, 0.08], [0.25, 0.20, 0.11])
            ship_label([-1.05, -0.08, 0.035], "ORBIT HOT DOGS", 0.17)
        }
        for side in [-1.0, 1.0] {
            ship_solid([side*0.9, 1.20, -0.50], [0.05, 2.40, 0.05], [0.43, 0.45, 0.37])
        }
        ship_light_pool([0.0, 2.7, 0.3], [1.0, 0.82, 0.57], 4.0, 3.6)
        ship_path_markers([-0.8, 0.04, -1.0], [1.0, 0.0], 3, 0.8, [0.86, 0.58, 0.16])

        T.position(0.0, 0.0, 0.70).rotation(0.0, Math.pi, 0.0) {
            name = "ship_hot_dog_vendor_root"
            let held_hot_dog = T.position(0.0, 0.07, -0.02).rotation(-Math.pi/2.0, 0.0, 0.0) {
                name = "ship_vendor_held_hot_dog"
                hot_dog(true)
            }
            held_hot_dog
            let vendor = GLTF("assets/components/characters/lumae/lumae.glb") {
                name = "ship_hot_dog_vendor"
                ship_avatar_material()
                vendor_pose()
            }
            vendor
            // The tray follows her posed wrist and any later avatar animation.
            on(vendor, "GLTFInitialized", fn(event) {
                // Imported renderables retain a GLTF material scope. Override
                // the face primitive itself after asynchronous loading finishes.
                for face in event.gltf.query_all("[name='Face'] > renderable") {
                    face.attach(ship_face_material())
                }
                let hand = event.gltf.query("[name='J_Bip_R_Hand']")
                if hand {
                    hand.attach(held_hot_dog)
                }
            })
            T.position(0.0, 0.85, 0.0) {
                Zone.capsule_y(0.23, 0.62) { Collidable.static().friction(0.8) {} }
            }
        }
    }
}
