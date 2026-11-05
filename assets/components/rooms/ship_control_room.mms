import { ship_box, ship_solid, ship_label, ship_deck, ship_door_wall } from "../ship_parts.mms"
// import { computer_cabinet } from "../computer_cabinet.mms"
// import { tape_cabinet } from "../tape_cabinet.mms"
// import { monitor_cabinet } from "../monitor_cabinet.mms"
import { chair } from "../chair.mms"
import { ship_path_markers, ship_light_pool } from "../ship_lighting.mms"
import { xr_settings_panel } from "../panels/xr_settings_panel.mms"
// import { ship_store_accordion } from "../ship_store_accordion.mms"

// Bridge: X=-6..6, Z=-12..0. Rear doorway at [0,0,0]. Forward is -Z.
export fn ship_control_room() {
    let wall_color=[0.82,0.76,0.66]
    let frame_color=[0.70,0.62,0.53]
    return T {
        name = "ship_control_room"
        ship_deck([0.0,-6.0],12.0,12.0,3.3,true,[0.86,0.81,0.73])
        ship_door_wall(12.0,wall_color)
        T.position(-5.86, 2.70, -1.6).rotation(0.0, Math.pi/2.0, 0.0).scale(0.75, 0.75, 0.75) {
            xr_settings_panel({ root_name = "ship_bridge_xr_settings" title = "XR / height adjustment" })
        }
        for x in [-3.0, 3.0] {
            for z in [-8.5, -3.5] {
                ship_light_pool([x, 2.8, z], [1.0,0.94,0.85], 5.0, 6.0)
            }
        }
        for side in [-1.0, 1.0] {
            ship_path_markers([side * 5.86, 0.14, -10.8], [0.0, 1.0], 6, 1.8, [0.25, 0.72, 0.85])
        }
        for side in [-1.0, 1.0] {
            ship_solid([side * 6.0, 1.6, -6.0], [0.16, 3.2, 12.0], wall_color)
        }
        // Mostly vertical windshield: rounded upper corners, flat sill.
        // Only its upper outline curves. A separate box blocks walking outside.
        ship_solid([0.0, 0.48, -12.0], [12.0, 0.96, 0.16], frame_color)
        ship_solid([0.0, 3.08, -12.0], [12.0, 0.24, 0.16], frame_color)
        for side in [-1.0, 1.0] {
            ship_solid([side * 5.65, 1.96, -12.0], [0.70, 2.0, 0.16], frame_color)
        }
        T.position(0.0, 0.96, -11.99) {
            name = "ship_bridge_windscreen"
            // Clear aperture; retain the pressure boundary without an alpha pane.
            T.position(0.0, 1.0, 0.0).scale(10.6, 2.0, 0.10) {
                Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
            }
        }
        // Opaque corner caps fill around the rounded glass outline.
        for side in [-1.0, 1.0] {
            T.position(0.0, 0.96, -11.98).scale(side, 1.0, 1.0) {
                R.polygon("ship/windshield-corner/v1", [[4.6, 2.0], [4.88, 1.91], [5.10, 1.76], [5.25, 1.58], [5.3, 1.38], [5.3, 2.0]]) { C.rgba(frame_color[0],frame_color[1],frame_color[2],1.0) }
            }
        }
        ship_label([-1.2, 2.75, -11.85], "FLIGHT CONTROL", 0.18)

        // Unload the equipment independently when investigating bridge cost.
        // The control stays by the aft doorway, facing into the room.
        // PERF ISOLATION: restore this block to instantiate its accordion and content.
        // ship_store_accordion("ship_bridge_computers","BRIDGE / COMPUTERS & CABINETS",fn() {
        //     return T {
        //         for row in range(2) {
        //             let z = -9.2 + row * 3.4
        //             for side in [-1.0, 1.0] {
        //                 T.position(side * 4.7, 0.0, z) { computer_cabinet() }
        //                 T.position(side * 1.5, 0.0, z) { tape_cabinet() }
        //                 T.position(side * 3.1, 0.0, z) { monitor_cabinet() }
        //             }
        //         }
        //     }
        // },null,null,[2.0,2.35,-0.15],true,Math.pi)

        // Two rows, with a wide centre aisle and chairs on the aft side.
        for row in range(2) {
            let z = -9.2 + row * 3.4
            for side in [-1.0, 1.0] {
                // Muted object accents contrast with the warm wall palette.
                let desk_accent=[0.56,0.72,0.72]
                if side>0 {desk_accent=[0.68,0.62,0.76]}
                // Writing surface between the equipment; top at 0.84 m.
                ship_solid([side * 2.3, 0.80, z + 0.25], [0.40, 0.08, 0.85], desk_accent)
                ship_solid([side * 3.9, 0.80, z + 0.25], [0.40, 0.08, 0.85], desk_accent)
                ship_box([side * 2.3, 0.845, z + 0.30], [0.22, 0.006, 0.30], [0.88, 0.86, 0.69])
                T.position(side * 3.1, 0.0, z + 1.15) { chair() }
            }
        }
    }
}
