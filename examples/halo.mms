// Desktop inspection: cargo run --release -- load examples/halo.mms
// The head stand moves on a native animation; upper rings show accumulated lag.
import { concentric_halo } from "../assets/components/accessories/concentric_halo.mms"
import { ui_unlit_material } from "../assets/components/materials/ui_unlit.mms"

RendererSettings {window_size(1200,900)}
BGC.rgba(0.08,0.07,0.12,1)
AL.rgb(0.4,0.4,0.4)
let unlit=ui_unlit_material()
T.position(0,0.8,0) {
    T.scale(2.2,1.8,0.04) {R.cube() {unlit C.rgba(0.18,0.16,0.24,1)}}
    // A contrasting background makes the 30% transparency visible.
    for index in range(9) {
        T.position(-0.96+index*0.24,0,0.04).scale(0.10,1.8,0.02) {
            R.cube() {unlit C.rgba(0.45,0.40,0.54,1)}
        }
    }
}
let head=T.position(0,0.8,0.6) {
    name="halo_demo_head"
    Transition {duration_beats(1.0) ease_in_out_sine() replace_same_target()}
    T.scale(0.28,0.34,0.28) {R.sphere() {unlit C.rgba(0.70,0.66,0.77,1)}}
    concentric_halo("halo_demo",{opacity=0.70 emission_gain=1.8 rim_strength=1.4})
}
head
Animation.looping().length(8.0) {
    Keyframe.at(0.0) {head.update_transform([-0.28,0.8,0.6],[0,0,-0.12],[1,1,1])}
    Keyframe.at(2.0) {head.update_transform([0.28,0.8,0.6],[0,0,0.12],[1,1,1])}
    Keyframe.at(4.0) {head.update_transform([0.28,0.8,0.6],[0,0,0.12],[1,1,1])}
    Keyframe.at(6.0) {head.update_transform([-0.28,0.8,0.6],[0,0,-0.12],[1,1,1])}
}
I.speed(1.0) {
    InputTransformMode.forward_z().fps_rotation()
    T.position(0,1.2,3.1).rotation(-0.12,0,0) {C3D {Pointer {}}}
}
