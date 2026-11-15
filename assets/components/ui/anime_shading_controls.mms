// Content for info_panel: edits one retained Anime-input Material.
// The caller owns panel chrome, restore, and the original Reset values.
fn fixed_2(value) {
    let scaled = Math.round(value * 100.0)
    let whole = Math.floor(scaled / 100.0)
    let fraction = scaled - whole * 100.0
    let padding = ""
    if fraction < 10.0 { padding = "0" }
    return "" + whole + "." + padding + fraction
}

fn anime_slider(slider_name, initial, minimum, maximum) {
    let track = T.scale(2.0, 0.025, 0.10) { R.cube() { C.rgba(0.24, 0.45, 0.65, 1.0) Raycastable.enabled() { interaction_priority(120.0) } } }
    let thumb = T.scale(0.1125, 0.225, 0.30) { R.sphere() { C.rgba(1.0, 0.46, 0.64, 1.0) Raycastable.enabled() { interaction_priority(120.0) } } }
    return Slider.range(minimum, maximum).step(0.01).value(initial).width(4.0)
        .track(track)
        .thumb(thumb) { name = slider_name }
}

fn anime_slider_row(label, slider, readout_name, initial) {
    return T {
        Style {
            display("flex") flex_direction("row") width(100%) height(3.0)
            align_items("center") gap(0.5)
        }
        T {
            Style { display("flex") width(11.0) height(3.0) align_items("center") }
            T.position(0.0, 0.0, 0.03) { Text { label } }
        }
        T {
            Style {
                display("flex") width(18.0) height(3.0) flex_grow(1.0)
                align_items("center") justify_content("center")
            }
            slider
        }
        T {
            Style {
                display("flex") width(4.0) height(3.0)
                align_items("center") justify_content("center")
            }
            T.position(0.0, 0.0, 0.03) { Text { name = readout_name fixed_2(initial) } }
        }
    }
}

fn clamp(value, minimum, maximum) {
    if value < minimum { return minimum }
    if value > maximum { return maximum }
    return value
}

// Inputs keep the ranges the Anime controls have always used; thresholds
// stay ordered, raising or lowering the other one.
fn set_shade_threshold(target, value) {
    let shade = value
    if shade < 0.0 { shade = 0.0 }
    target.set_input("shade_threshold", shade)
    if target.get_input("lit_threshold") < shade { target.set_input("lit_threshold", shade) }
}

fn set_lit_threshold(target, value) {
    let lit = value
    if lit < 0.0 { lit = 0.0 }
    target.set_input("lit_threshold", lit)
    if target.get_input("shade_threshold") > lit { target.set_input("shade_threshold", lit) }
}

export fn anime_shading_controls(target, reset_values) {
    let shade_strength = target.get_input("shade_strength")
    let shade_threshold = target.get_input("shade_threshold")
    let lit_threshold = target.get_input("lit_threshold")
    let rim_strength = target.get_input("rim_strength")
    let rim_power = target.get_input("rim_power")
    let content = T {
        name = "anime_shading_controls"
        Style {
            display("block") width(100%) padding(0.65)
            font_size(0.8) color([0.96, 0.96, 0.98, 1.0])
            background_color([0.075, 0.075, 0.09, 0.96]) background_z(-0.01)
        }
        anime_slider_row("Shade strength", anime_slider("anime_shade_strength_slider", shade_strength, 0.0, 1.0), "anime_shade_strength_readout", shade_strength)
        anime_slider_row("Shade threshold", anime_slider("anime_shade_threshold_slider", shade_threshold, 0.0, 2.0), "anime_shade_threshold_readout", shade_threshold)
        anime_slider_row("Lit threshold", anime_slider("anime_lit_threshold_slider", lit_threshold, 0.0, 2.0), "anime_lit_threshold_readout", lit_threshold)
        anime_slider_row("Rim strength", anime_slider("anime_rim_strength_slider", rim_strength, 0.0, 1.0), "anime_rim_strength_readout", rim_strength)
        anime_slider_row("Rim power", anime_slider("anime_rim_power_slider", rim_power, 0.01, 16.0), "anime_rim_power_readout", rim_power)
        T {
            Style {
                display("flex") width(100%) height(2.5)
                align_items("center") justify_content("flex-end")
            }
            T {
                name = "anime_shading_reset"
                Raycastable.enabled() { interaction_priority(120.0) }
                Style {
                    display("flex") width(8.0) height(2.0)
                    align_items("center") justify_content("center")
                    background_color([0.35, 0.20, 0.30, 1.0]) background_z(-0.02)
                }
                T.position(0.0, 0.0, 0.03) { Text { "Reset" } }
            }
        }
    }
    // Threshold callbacks refresh both rows because the setters maintain shade <= lit.
    let shade_strength_slider = content.query("#anime_shade_strength_slider")
    let shade_strength_readout = content.query("#anime_shade_strength_readout")
    let shade_threshold_slider = content.query("#anime_shade_threshold_slider")
    let shade_threshold_readout = content.query("#anime_shade_threshold_readout")
    let lit_threshold_slider = content.query("#anime_lit_threshold_slider")
    let lit_threshold_readout = content.query("#anime_lit_threshold_readout")
    let rim_strength_slider = content.query("#anime_rim_strength_slider")
    let rim_strength_readout = content.query("#anime_rim_strength_readout")
    let rim_power_slider = content.query("#anime_rim_power_slider")
    let rim_power_readout = content.query("#anime_rim_power_readout")
    let reset = content.query("#anime_shading_reset")
    on(shade_strength_slider, "SliderChanged", fn(event) {
        target.set_input("shade_strength", clamp(event["value"], 0.0, 1.0))
        let effective = target.get_input("shade_strength")
        shade_strength_slider.sync_value(effective)
        shade_strength_readout.set_text(fixed_2(effective))
    })
    on(shade_threshold_slider, "SliderChanged", fn(event) {
        set_shade_threshold(target, event["value"])
        let shade = target.get_input("shade_threshold")
        let lit = target.get_input("lit_threshold")
        shade_threshold_slider.sync_value(shade)
        shade_threshold_readout.set_text(fixed_2(shade))
        lit_threshold_slider.sync_value(lit)
        lit_threshold_readout.set_text(fixed_2(lit))
    })
    on(lit_threshold_slider, "SliderChanged", fn(event) {
        set_lit_threshold(target, event["value"])
        let shade = target.get_input("shade_threshold")
        let lit = target.get_input("lit_threshold")
        shade_threshold_slider.sync_value(shade)
        shade_threshold_readout.set_text(fixed_2(shade))
        lit_threshold_slider.sync_value(lit)
        lit_threshold_readout.set_text(fixed_2(lit))
    })
    on(rim_strength_slider, "SliderChanged", fn(event) {
        target.set_input("rim_strength", clamp(event["value"], 0.0, 1.0))
        let effective = target.get_input("rim_strength")
        rim_strength_slider.sync_value(effective)
        rim_strength_readout.set_text(fixed_2(effective))
    })
    on(rim_power_slider, "SliderChanged", fn(event) {
        target.set_input("rim_power", clamp(event["value"], 0.01, 128.0))
        let effective = target.get_input("rim_power")
        rim_power_slider.sync_value(effective)
        rim_power_readout.set_text(fixed_2(effective))
    })
    on(reset, "Click", fn(event) {
        target.set_input("shade_strength", clamp(reset_values.shade_strength, 0.0, 1.0))
        set_shade_threshold(target, reset_values.shade_threshold)
        set_lit_threshold(target, reset_values.lit_threshold)
        target.set_input("rim_strength", clamp(reset_values.rim_strength, 0.0, 1.0))
        target.set_input("rim_power", clamp(reset_values.rim_power, 0.01, 128.0))
        let shade_strength_value = target.get_input("shade_strength")
        let shade_threshold_value = target.get_input("shade_threshold")
        let lit_threshold_value = target.get_input("lit_threshold")
        let rim_strength_value = target.get_input("rim_strength")
        let rim_power_value = target.get_input("rim_power")
        shade_strength_slider.sync_value(shade_strength_value)
        shade_strength_readout.set_text(fixed_2(shade_strength_value))
        shade_threshold_slider.sync_value(shade_threshold_value)
        shade_threshold_readout.set_text(fixed_2(shade_threshold_value))
        lit_threshold_slider.sync_value(lit_threshold_value)
        lit_threshold_readout.set_text(fixed_2(lit_threshold_value))
        rim_strength_slider.sync_value(rim_strength_value)
        rim_strength_readout.set_text(fixed_2(rim_strength_value))
        rim_power_slider.sync_value(rim_power_value)
        rim_power_readout.set_text(fixed_2(rim_power_value))
    })
    return content
}
