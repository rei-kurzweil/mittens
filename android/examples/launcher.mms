// Headset home screen for the Android launcher build (`--features launcher`).
// Browses the project folder pushed by android/packaging/sync-project.sh and
// opens the selected MMS file. Hold the left menu button in any scene to come
// back here; a scene that fails to load or render also returns here with its
// error. See docs/task/android-launcher.md.
import { shell, caption, action, shell_ui_material } from "../../assets/components/mitten_droid/ui.mms"
import { ui_unlit_material } from "../../assets/components/materials/ui_unlit.mms"
import { anime_surface } from "../../assets/components/materials/anime_surface.mms"
import { graphics_settings_panel } from "../../assets/components/panels/graphics_settings_panel.mms"
import { dock } from "../../assets/components/ui/dock.mms"

BGC.rgba(1.0, 0.80, 0.86, 1.0)
GraphicsSettings.foveation.quality("off")
AL.rgb(0.38, 0.31, 0.39)
T.position(23, 24, -50) { DL.color(1.0, 0.88, 0.80).intensity(1.4) }
// The Android host uses eye-level LOCAL space: the floor sits below the eyes.
T.position(0.0, -1.65, -1.5).scale(14.0, 0.1, 14.0) {
    R.cube() { anime_surface() C.rgba(0.97, 0.91, 0.87, 1.0) }
}

let pointer_material = ui_unlit_material()
fn controller_pointer(label) {
    // Stay out of view until the host supplies a valid controller Aim pose.
    return T.position(0, -20, 0) {
        Pointer { name = label }
        T.position(0, 0, -1.3).scale(0.0015, 0.0015, 2.6) { R.cube() { pointer_material C.rgba(1.0, 0.76, 0.25, 1.0) } }
        T.position(0, 0, -2.6).scale(0.012, 0.012, 0.012) { R.cube() { pointer_material C.rgba(1.0, 0.88, 0.52, 1.0) } }
    }
}
T {
    name = "launcher_xr_locomotion_root"
    // Grab reparents objects into this rig; give them an explicit custom default.
    anime_surface()
    InputXR.on() {
        InputXRGamepad { hand("left") locomotion() speed(1.4) }
        T { CXR {} }
        XRHand.new(true, "Left", "Aim") { controller_pointer("launcher_left_pointer") }
        XRHand.new(true, "Right", "Aim") { controller_pointer("launcher_right_pointer") }
    }
}

// Paths are relative to the project root, which is the host's working directory.
fn has_entry(directory, name) {
    for entry in File.list_dir(directory) {
        if entry.name == name { return true }
    }
    return false
}
let pushed = has_entry(".", "examples")
let start = "android/examples"
if pushed { start = "examples" }

let state = { selected = "", source = "", browser = null, selection_view = null, source_view = null, scroll_right = 0, scroll_time = 0 }
let status = Text { name = "launcher_status" "Select an MMS file, then press Launch." }
if !pushed { status.set_text("Only the bundled launcher is here. Push a project with android/packaging/sync-project.sh.") }
let previous_error = App.last_error()
if previous_error != "" { status.set_text("Returned home: " + previous_error) }
let launch = action("Launch", "launcher_launch")
let refresh = action("Refresh", "launcher_refresh")

fn show_selection() {
    let selected = FileBrowser.selection(state.browser)
    state.selected = selected.path
    state.source = selected.source
    if state.selection_view != null { state.selection_view.set_text(selected.path) }
    if state.source_view != null { state.source_view.set_text(selected.source) }
}
fn build_browser() {
    let browser = FileBrowser.local(".", start)
    state.browser = browser
    on(browser, "DataEvent", fn(event) { show_selection() })
    return browser
}
fn close_browser() { state.browser = null }
fn build_preview() {
    let selection = Text { name = "launcher_selection" state.selected }
    let source = Text { name = "launcher_source" state.source }
    state.selection_view = selection
    state.source_view = source
    return T {
        Style { display("block") width(100%) }
        T { Style { margin_bottom(0.5) } caption("SELECTED SCRIPT") }
        T { Style { height(2) margin_top(0.4) } selection }
        T { Style { height(20) margin_top(0.8) padding(0.8) font_size(1.0) background_color([0.018, 0.016, 0.012, 1]) } source }
        T { Style { margin_top(0.8) font_size(0.9) } caption("Scenes that need unsupported rendering features return here with the reason.") }
    }
}
fn close_preview() { state.selection_view = null state.source_view = null }

T.position(-1.89, 1.0, -2.6).scale(0.030, 0.030, 0.030) {
    shell("mittens / launcher", "Open a scene from this headset", T {
        launch refresh
        T { Style { margin_top(0.7) height(3) font_size(0.85) } status }
    }, { build = build_browser, close = close_browser }, { build = build_preview, close = close_preview },
    caption("Left stick moves. Right stick scrolls. Hold the left menu button in any scene to return here."), true, true)
}

on(launch, "Click", fn(event) {
    if state.selected == "" {
        status.set_text("Select an .mms file first.")
        return
    }
    status.set_text("Loading " + state.selected)
    App.launch(state.selected)
})
on(refresh, "Click", fn(event) {
    if state.browser != null { FileBrowser.refresh(state.browser) }
})

// Settings panels open from the dock, beside the launcher window.
let dock_panels = T { name = "launcher_dock_panels" shell_ui_material() }
dock_panels
T.position(1.5, 0.6, -1.9).rotation(0.0, -0.55, 0.0) {
    dock({
        root_name = "launcher_dock"
        mount = dock_panels
        material = shell_ui_material()
        groups = [[
            { id = "graphics" label = "Graphics" root_name = "launcher_graphics_settings"
                position = [1.2, 1.0, -2.4] rotation = [0.0, -0.45, 0.0]
                build = fn() {
                    return T.scale(0.65, 0.65, 0.65) {
                        graphics_settings_panel({ root_name = "launcher_graphics_settings" material = shell_ui_material() foveation_only = true })
                    }
                } },
        ]]
    })
}

on_global("XrAxisChanged", fn(event) {
    if event.control == "RightStick" { state.scroll_right = event.value[1] }
})
on_global("FrameTick", fn(event) {
    state.scroll_time = state.scroll_time + event.dt_sec
    if state.scroll_time >= 0.16 {
        state.scroll_time = 0
        if state.browser != null {
            if state.scroll_right < -0.55 { FileBrowser.scroll(state.browser, 1) }
            if state.scroll_right > 0.55 { FileBrowser.scroll(state.browser, -1) }
        }
    }
})
