import { ship_label } from "../ship_parts.mms"
import { ui_unlit_material } from "../materials/ui_unlit.mms"

fn button(name,title,x,y,width,action) {
    let font=0.018
    if width<0.1 {font=0.013}
    let root=T.position(x,y,0) {
        name="pickup_hydraulics_"+name
        Grabbable.off()
        T.scale(width,0.065,0.025) {
            R.cube() {
                C.rgba(0.16,0.23,0.25,1)
                Raycastable.click_only() { interaction_priority(180) }
            }
        }
        ship_label([-width*0.38,-0.008,0.018],title,font)
    }
    on(root,"Click",fn(event) {action()})
    return root
}

export fn pickup_hydraulics_panel(ride) {
    let selected={corner="all"}
    let readout=Text {name="pickup_hydraulics_readout" "RIDE all 100%"}
    fn refresh() {
        let status=ride.status(selected.corner)
        readout.set_text("RIDE "+selected.corner+" "+Math.round(status.ratio*100)+"%")
    }
    let panel=T.position(0.04,1.80,-0.70) {
        name="pickup_hydraulics_panel"
        ui_unlit_material()
        Grabbable.off()
        T {
            name="pickup_hydraulics_content"
            Selectable.off()
            T.position(-0.04,0.12,0.02).scale(0.022,0.022,0.022) {readout}
            let index=0
            for corner in ["all","front","rear","left","right"] {
                button("select_"+corner,corner,index*0.095,0.03,0.085,fn() {
                    selected.corner=corner refresh()
                })
                index=index+1
            }
            index=0
            for corner in ["fl","fr","rl","rr"] {
                button("select_"+corner,corner,0.0475+index*0.095,-0.045,0.085,fn() {
                    selected.corner=corner refresh()
                })
                index=index+1
            }
            button("raise","UP",0.04,-0.135,0.13,fn() {ride.adjust(selected.corner,0.05) refresh()})
            button("lower","DOWN",0.19,-0.135,0.13,fn() {ride.adjust(selected.corner,-0.05) refresh()})
            button("reset","REST",0.34,-0.135,0.13,fn() {ride.reset() refresh()})
        }
    }
    return panel
}
