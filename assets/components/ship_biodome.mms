import { ship_environment_material } from "./materials/ship_anime_materials.mms"
import { ship_box } from "./ship_parts.mms"

// Rectangular greenhouse frame. Clear pressure panels have no alpha overlay;
// their collision is separate from the single combined structural mesh.
export fn ship_biodome(label, width, length, height, door_z, west_door, east_door) {
    let frame = [0.35,0.49,0.46]
    return T {
        name = label
        CombineMesh {
            ship_environment_material()
            for x in [-width/2.0,width/2.0] {
                let door = west_door
                if x > 0 { door = east_door }
                for y in [height/2.0,height] {
                    ship_box([x,y,0], [0.10,0.10,length], frame)
                }
                for index in range(Math.floor(length/4.0)+1) {
                    let z = -length/2.0 + index*4.0
                    if !door || Math.abs(z-door_z) > 1.3 {
                        ship_box([x,height/2.0,z], [0.10,height,0.10], frame)
                    }
                }
                if door {
                    for z in [door_z-1.3,door_z+1.3] {
                        ship_box([x,height/2.0,z], [0.12,height,0.12], frame)
                    }
                    ship_box([x,2.9,door_z], [0.12,0.12,2.6], frame)
                }
            }
            for z in [-length/2.0,length/2.0] {
                for y in [0.06,height/2.0,height] {
                    ship_box([0,y,z], [width,0.10,0.10], frame)
                }
                for index in range(Math.floor(width/4.0)+1) {
                    ship_box([-width/2.0+index*4.0,height/2.0,z], [0.10,height,0.10], frame)
                }
            }
            // Orthogonal roof grid makes the rectangular clear roof legible.
            for index in range(Math.floor(width/8.0)+1) {
                ship_box([-width/2.0+index*8.0,height,0], [0.10,0.12,length], frame)
            }
            for index in range(Math.floor(length/8.0)+1) {
                ship_box([0,height,-length/2.0+index*8.0], [width,0.12,0.10], frame)
            }
        }
        for x in [-width/2.0,width/2.0] {
            let door = west_door
            if x > 0 { door = east_door }
            if door {
                for side in [-1,1] {
                    let a = -length/2.0
                    let b = door_z-1.3
                    if side > 0 { a = door_z+1.3 b = length/2.0 }
                    T.position(x,height/2.0,(a+b)/2.0) {
                        Zone.cube([0.05,height/2.0,(b-a)/2.0]) { Collidable.static() {} }
                    }
                }
                T.position(x,(height+2.9)/2.0,door_z) {
                    Zone.cube([0.05,(height-2.9)/2.0,1.3]) { Collidable.static() {} }
                }
            } else {
                T.position(x,height/2.0,0) { Zone.cube([0.05,height/2.0,length/2.0]) { Collidable.static() {} } }
            }
        }
        for z in [-length/2.0,length/2.0] {
            T.position(0,height/2.0,z) { Zone.cube([width/2.0,height/2.0,0.05]) { Collidable.static() {} } }
        }
        T.position(0,height,0) { Zone.cube([width/2.0,0.05,length/2.0]) { Collidable.static() {} } }
    }
}
