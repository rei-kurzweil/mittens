// Importable graphics settings panel; GraphicsSettingsSystem owns applied values.
import { info_panel, info_panel_body } from "../ui/info_panel.mms"
import { ui_unlit_material } from "../materials/ui_unlit.mms"

let QUALITY_STEPS = [64, 128, 256, 512, 1024, 1440, 2048]
let UNIT_SCALE = 0.08
let ROW_HEIGHT = 2.4
let TEXT_COLOR = [0.0, 0.0, 0.0, 1.0]
let BODY_COLOR = [0.98, 0.95, 0.84, 1.0]
let ROW_COLOR = [0.95, 0.91, 0.79, 1.0]
let ACCENT_COLOR = [0.57, 0.47, 0.31, 1.0]
let TRACK_WIDTH = 10.5 * UNIT_SCALE
let THUMB_RADIUS = 0.075
let SLIDER_WIDTH = TRACK_WIDTH - THUMB_RADIUS * 2.0 - 0.04

fn enabled_text(enabled) {
    if enabled { return "On" }
    return "Off"
}

fn setting_toggle(enabled, control_name) {
    if enabled { return Toggle.on() { name = control_name } }
    return Toggle.off() { name = control_name }
}

fn read_settings(state) {
    state.bloom = GraphicsSettings.bloom.enabled()
    state.bloom_layers = GraphicsSettings.bloom.layer_count()
    state.enabled = GraphicsSettings.mirrors.enabled()
    state.motion_enabled = GraphicsSettings.secondary_motion.enabled()
    state.motion_paused = GraphicsSettings.secondary_motion.paused()
    state.quality = GraphicsSettings.mirrors.quality_override()
    state.quality_index = 3
    if state.quality != null {
        for index in range(7) {
            if Math.abs(QUALITY_STEPS[index] - state.quality) < Math.abs(QUALITY_STEPS[state.quality_index] - state.quality) {
                state.quality_index = index
            }
        }
    }
}

fn quality_text(state) {
    if state.quality == null { return "Authored" }
    return "" + state.quality
}

fn toggle_control(toggle, label, control_name) {
    return T {
        name = control_name
        Raycastable.click_only() { interaction_priority(120.0) }
        toggle
        Style {
            display("flex") width(5.0) height(2.1)
            align_items("center") justify_content("center") font_size(0.65)
            color(TEXT_COLOR) background_color([0.88, 0.81, 0.66, 1.0])
            background_z(0.01)
        }
        T.position(0.0, 0.0, 0.03) { Text { label } }
    }
}

fn setting_label(text) {
    return T {
        Style {
            display("flex") width(12.0) height(ROW_HEIGHT)
            align_items("center") padding_xy(0.35, 0.0) font_size(0.62)
        }
        T.position(0.0, 0.0, 0.03) { Text { text } }
    }
}

fn section_heading(text, heading_name) {
    return T {
        name = heading_name
        Style {
            display("flex") width(100%) height(2.0)
            align_items("center") padding_xy(0.35, 0.0)
            font_size(0.75) color(TEXT_COLOR)
        }
        T.position(0.0, 0.0, 0.03) { Text { text } }
    }
}

fn setting_row(label, control, readout) {
    return T {
        Style {
            display("flex") flex_direction("row") width(100%) height(ROW_HEIGHT)
            align_items("center") gap(0.35)
            background_color(ROW_COLOR) background_z(-0.02)
        }
        setting_label(label)
        T {
            // Controls own trigger drags even inside a grip-grabbable panel.
            Grabbable.off()
            Style {
                display("flex") width(10.5) height(ROW_HEIGHT) flex_grow(1.0)
                align_items("center") justify_content("center")
            }
            control
        }
        T {
            Style {
                display("flex") width(6.5) height(ROW_HEIGHT)
                align_items("center") justify_content("center") font_size(0.65)
            }
            T.position(0.0, 0.0, 0.03) { readout }
        }
    }
}

fn settings_content(state) {
    read_settings(state)
    let bloom_toggle = setting_toggle(state.bloom, "graphics_bloom_toggle")
    let bloom_control = toggle_control(bloom_toggle, "Toggle", "graphics_bloom_toggle_control")
    let bloom_readout = Text { name = "graphics_bloom_readout" enabled_text(state.bloom) }
    let bloom_track = T.scale(TRACK_WIDTH * 0.5, 0.025, 0.10) {
        R.cube() { C.rgba(0.73, 0.65, 0.50, 1.0) Raycastable.enabled() { interaction_priority(120.0) } }
    }
    let bloom_thumb = T.scale(THUMB_RADIUS, 0.14, 0.14) {
        R.sphere() { C.rgba(ACCENT_COLOR[0], ACCENT_COLOR[1], ACCENT_COLOR[2], ACCENT_COLOR[3]) Raycastable.enabled() { interaction_priority(120.0) } }
    }
    let bloom_layers = Slider.range(1.0, 6.0).step(1.0).value(state.bloom_layers).width(SLIDER_WIDTH)
        .track(bloom_track).thumb(bloom_thumb) { name = "graphics_bloom_layers_slider" }
    let bloom_layers_readout = Text { name = "graphics_bloom_layers_readout" "" + state.bloom_layers }
    on(bloom_toggle, "ToggleChanged", fn(event) {
        GraphicsSettings.bloom.toggle(event.value)
        state.bloom = GraphicsSettings.bloom.enabled()
        bloom_readout.set_text(enabled_text(state.bloom))
    })
    on(bloom_layers, "SliderChanged", fn(event) {
        GraphicsSettings.bloom.layers(event.value)
        state.bloom_layers = GraphicsSettings.bloom.layer_count()
        bloom_layers_readout.set_text("" + state.bloom_layers)
    })
    let mirror_toggle = setting_toggle(state.enabled, "graphics_mirrors_toggle")
    let mirror_control = toggle_control(mirror_toggle, "Toggle", "graphics_mirrors_toggle_control")
    let toggle_readout = Text {
        name = "graphics_mirrors_enabled_readout"
        enabled_text(state.enabled)
    }
    let track = T.scale(TRACK_WIDTH * 0.5, 0.025, 0.10) {
        R.cube() {
            C.rgba(0.73, 0.65, 0.50, 1.0)
            Raycastable.enabled() { interaction_priority(120.0) }
        }
    }
    let thumb = T.scale(THUMB_RADIUS, 0.14, 0.14) {
        R.sphere() {
            C.rgba(ACCENT_COLOR[0], ACCENT_COLOR[1], ACCENT_COLOR[2], ACCENT_COLOR[3])
            Raycastable.enabled() { interaction_priority(120.0) }
        }
    }
    let quality_slider = Slider.range(0.0, 6.0).step(1.0).value(state.quality_index).width(SLIDER_WIDTH)
        .track(track).thumb(thumb) { name = "graphics_mirrors_quality_slider" }
    let quality_readout = Text {
        name = "graphics_mirrors_quality_readout"
        quality_text(state)
    }
    let motion_toggle = setting_toggle(state.motion_enabled, "graphics_secondary_motion_toggle")
    let motion_control = toggle_control(motion_toggle, "Toggle", "graphics_secondary_motion_toggle_control")
    let motion_readout = Text {
        name = "graphics_secondary_motion_readout"
        enabled_text(state.motion_enabled)
    }
    let pause_toggle = setting_toggle(state.motion_paused, "graphics_secondary_motion_pause_toggle")
    let pause_control = toggle_control(pause_toggle, "Pause", "graphics_secondary_motion_pause_control")
    let pause_readout = Text {
        name = "graphics_secondary_motion_pause_readout"
        enabled_text(state.motion_paused)
    }

    on(mirror_toggle, "ToggleChanged", fn(event) {
        GraphicsSettings.mirrors.toggle(event.value)
        state.enabled = GraphicsSettings.mirrors.enabled()
        toggle_readout.set_text(enabled_text(state.enabled))
    })
    on(quality_slider, "SliderChanged", fn(event) {
        GraphicsSettings.mirrors.quality(QUALITY_STEPS[event.value])
        read_settings(state)
        quality_readout.set_text(quality_text(state))
    })
    on(motion_toggle, "ToggleChanged", fn(event) {
        GraphicsSettings.secondary_motion.toggle(event.value)
        state.motion_enabled = GraphicsSettings.secondary_motion.enabled()
        motion_readout.set_text(enabled_text(state.motion_enabled))
    })
    on(pause_toggle, "ToggleChanged", fn(event) {
        GraphicsSettings.secondary_motion.pause(event.value)
        state.motion_paused = GraphicsSettings.secondary_motion.paused()
        pause_readout.set_text(enabled_text(state.motion_paused))
    })

    return T {
        name = "graphics_settings_content"
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.25) }
        section_heading("Bloom", "graphics_bloom_heading")
        setting_row("Toggle bloom", bloom_control, bloom_readout)
        setting_row("Pyramid levels", bloom_layers, bloom_layers_readout)
        section_heading("Mirrors", "graphics_mirrors_heading")
        setting_row("Toggle mirrors", mirror_control, toggle_readout)
        setting_row("Mirror resolution", quality_slider, quality_readout)
        section_heading("Secondary motion", "graphics_secondary_motion_heading")
        setting_row("Toggle secondary motion", motion_control, motion_readout)
        setting_row("Pause (keep pose)", pause_control, pause_readout)
    }
}

// Required: root_name. Optional: title (string or complete title-bar component).
// The caller owns world placement; custom title bars own layout/movement markers.
export fn graphics_settings_panel(options) {
    let title = "Graphics settings"
    if options["title"] != null { title = options.title }
    let state = { bloom = true bloom_layers = 1 enabled = true motion_enabled = true motion_paused = false quality = null quality_index = 3 }
    let panel = info_panel({
        root_name = options.root_name
        width_gu = 32.0
        unit_scale = UNIT_SCALE
        title = title
        title_background_color = [0.93, 0.87, 0.71, 1.0]
        title_text_color = TEXT_COLOR
        toggle_background_color = [0.88, 0.81, 0.66, 1.0]
        toggle_icon_color = TEXT_COLOR
        toggle_icon_glow_intensity = 0.0
        body_background_color = BODY_COLOR
        body_text_color = TEXT_COLOR
        content = settings_content(state)
    })
    // Grip grabbing reparents the inner panel root away from its layout slot.
    // Keep selection exclusion and restore delivery on that moving root.
    let panel_root = panel.query("#" + options.root_name)
    let panel_material = ui_unlit_material()
    panel_root.attach(panel_material)
    let selection_guard = Selectable.off()
    panel_root.attach(selection_guard)
    let body_mount = panel.query("#accordion_body_mount")
    on(panel_root, "DataEvent", fn(event) {
        if event == "AccordionRestoreRequested" {
            body_mount.attach(info_panel_body({
                content = settings_content(state)
                body_background_color = BODY_COLOR
                body_text_color = TEXT_COLOR
            }))
        }
    })
    // The subscription captures the stable panel name, then resolves current
    // controls so minimized/deleted bodies never leave stale control handles.
    on_global("DataEvent", fn(event) {
        if event == "GraphicsSettingsChanged" {
            read_settings(state)
            let current_panel = query("#" + options.root_name)
            if current_panel {
                let bloom_toggle = current_panel.query("#graphics_bloom_toggle")
                let bloom_readout = current_panel.query("#graphics_bloom_readout")
                let bloom_layers = current_panel.query("#graphics_bloom_layers_slider")
                let bloom_layers_readout = current_panel.query("#graphics_bloom_layers_readout")
                if bloom_toggle { bloom_toggle.sync_value(state.bloom) }
                if bloom_readout { bloom_readout.set_text(enabled_text(state.bloom)) }
                if bloom_layers { bloom_layers.sync_value(state.bloom_layers) }
                if bloom_layers_readout { bloom_layers_readout.set_text("" + state.bloom_layers) }
                let toggle = current_panel.query("#graphics_mirrors_toggle")
                let readout = current_panel.query("#graphics_mirrors_enabled_readout")
                if toggle { toggle.sync_value(state.enabled) }
                if readout { readout.set_text(enabled_text(state.enabled)) }
                let slider = current_panel.query("#graphics_mirrors_quality_slider")
                let quality_readout = current_panel.query("#graphics_mirrors_quality_readout")
                if slider { slider.sync_value(state.quality_index) }
                if quality_readout { quality_readout.set_text(quality_text(state)) }
                let motion_toggle = current_panel.query("#graphics_secondary_motion_toggle")
                let motion_readout = current_panel.query("#graphics_secondary_motion_readout")
                if motion_toggle { motion_toggle.sync_value(state.motion_enabled) }
                if motion_readout { motion_readout.set_text(enabled_text(state.motion_enabled)) }
                let pause_toggle = current_panel.query("#graphics_secondary_motion_pause_toggle")
                let pause_readout = current_panel.query("#graphics_secondary_motion_pause_readout")
                if pause_toggle { pause_toggle.sync_value(state.motion_paused) }
                if pause_readout { pause_readout.set_text(enabled_text(state.motion_paused)) }
            }
        }
    })
    return panel
}
