import { f32, vec3 } from "types"

let shared = { definition=null }

// Single-pass straight-alpha surface. Brightness is shader-provided, without
// the opaque-only emission-extraction contract or an additional glow mesh.
export fn halo_material(options) {
    if options==null {options={}}
    if shared.definition==null {
        shared.definition=MaterialDefinition(MaterialBase())
            .fragment("assets/shaders/halo.frag")
            .inputs({ gain:f32, rim_strength:f32, rim_power:f32, rim_color:vec3 })
    }
    let gain=1.8
    let rim_strength=1.4
    let rim_power=2.5
    if options["emission_gain"]!=null {gain=options.emission_gain}
    if options["rim_strength"]!=null {rim_strength=options.rim_strength}
    if options["rim_power"]!=null {rim_power=options.rim_power}
    return Material(shared.definition).inputs({
        gain=gain rim_strength=rim_strength rim_power=rim_power
        rim_color=[1.0,0.92,0.98]
    })
}
