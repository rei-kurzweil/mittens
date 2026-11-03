import { ship_solid, ship_box } from "../ship_parts.mms"

// A single grey alpha plane, outside the opaque body batch. No duplicate back
// face or solid wall behind it. The thin collision boundary is independent.
export fn vehicle_glass(position, width, height, yaw) {
    return T.position(position[0],position[1],position[2]).rotation(0,yaw,0) {
        name = "vehicle_grey_glass"
        T.scale(width,height,1) {
            R.plane() { C.rgba(0.65,0.68,0.69,0.18) Unlit {} }
        }
        Zone.cube([width/2,height/2,0.012]) { Collidable.static() {} }
    }
}
// Split a solid wall around a real rectangular window opening. 'along_x'
// selects the long horizontal axis; all returned opaque panels can be batched.
export fn vehicle_window_wall(center, size, window_center, window_size, color, along_x) {
    let axis = 2
    if along_x { axis = 0 }
    let lo = center[axis]-size[axis]/2
    let hi = center[axis]+size[axis]/2
    let bottom = center[1]-size[1]/2
    let top = center[1]+size[1]/2
    let a = window_center[axis]-window_size[0]/2
    let b = window_center[axis]+window_size[0]/2
    let low = window_center[1]-window_size[1]/2
    let high = window_center[1]+window_size[1]/2
    return T {
        for band in range(4) {
            let x0=lo let x1=hi let y0=bottom let y1=top
            if band==0 { y1=low }
            if band==1 { y0=high }
            if band==2 { x1=a y0=low y1=high }
            if band==3 { x0=b y0=low y1=high }
            let pos=[center[0],(y0+y1)/2,center[2]]
            let extent=[size[0],y1-y0,size[2]]
            if along_x { pos=[(x0+x1)/2,(y0+y1)/2,center[2]] extent=[x1-x0,y1-y0,size[2]] }
            else { pos=[center[0],(y0+y1)/2,(x0+x1)/2] extent=[size[0],y1-y0,x1-x0] }
            ship_solid(pos,extent,color)
        }
    }
}
