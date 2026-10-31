// Headset shell in MMS, with a native grid backed by the PC's directory listing.
import { shell, caption } from "../../assets/components/mitten_droid/ui.mms"
import { ui_unlit_material } from "../../assets/components/materials/ui_unlit.mms"
import { envelope, decode } from "../../assets/components/mitten_droid/protocol.mms"

BGC.rgba(0.015, 0.025, 0.045, 1.0)
let pointer_material = ui_unlit_material()
fn controller_pointer(label) {
    // Stay out of view until the host supplies a valid controller Aim pose.
    return T.position(0,-20,0) {
        Pointer { name = label }
        T.position(0,0,-1.3).scale(0.0015,0.0015,2.6) { R.cube() { pointer_material C.rgba(1.0,0.76,0.25,1.0) } }
        T.position(0,0,-2.6).scale(0.012,0.012,0.012) { R.cube() { pointer_material C.rgba(1.0,0.88,0.52,1.0) } }
    }
}
InputXR.on() {
    T { CXR {} }
    XRHand.new(true, "Left", "Aim") { controller_pointer("shell_left_pointer") }
    XRHand.new(true, "Right", "Aim") { controller_pointer("shell_right_pointer") }
}
let browser = FileBrowser.demo()
let state = {
    scroll_left = 0, scroll_right = 0, scroll_time = 0, demo = true, session = "", selected = "", source = "", draft = "", idle = 10,
    action_id = "", entry_id = "", action_kind = "", revision = ""
}
let status = Text { name = "shell_status" "DESIGN PREVIEW / dummy data / desktop connection optional" }
let selection = Text { name = "shell_selection" "mitten-droid / MMS console" }
let source = Text { name = "shell_source" "Mittens / remote console\n\nHeadset     Focus 3\nWorkspace   examples/mitten-droid\nMode        UI preview\n\n> let greeting = \"hello, headset\"\nhello, headset\n\n> 6 * 7\n42\n\nReady for your next idea.\n_" }
let draft = TextInput { name = "shell_terminal_input" "" }
// The Android host uses eye-level LOCAL space, with no floor-height offset.
T.position(-1.89, 1.0, -2.6).scale(0.030, 0.030, 0.030) {
    shell("mitten-droid / headset", "Browse your PC scripts from here", T { status }, browser, T {
        Style { display("block") width(100%) }
        T { Style { height(2) } selection }
        T {
            Style { height(23) margin_top(0.8) padding(0.8) font_size(1.0) background_color([0.018, 0.016, 0.012, 1]) }
            source
        }
        T { Style { margin_top(1.1) margin_bottom(0.5) } caption("UDP TERMINAL / PORT 7001") }
        T {
            Style { height(9) padding(0.7) background_color([0.035, 0.029, 0.018, 1]) }
            draft
        }
        T { Style { margin_top(0.6) font_size(0.9) } caption("PC keyboard edits arrive over UDP. Enter inserts a newline; code is not executed.") }
    }, caption("Trigger selects. Thumbstick or pointer drag scrolls. No downloads or script execution."))
}
let server = HttpServer.bind("0.0.0.0:7000") {}
server
on(browser, "DataEvent", fn(event) {
    if state.demo {
        let selected = FileBrowser.selection(browser)
        if selected.name != "" { selection.set_text("Selected: " + selected.name) }
        return
    }
    let action = FileBrowser.action(browser)
    if state.idle < 3 && action != null {
        state.action_id = action.action_id
        state.entry_id = action.entry_id
        state.action_kind = action.kind
        state.revision = action.revision
    }
})
on(server, "HttpRequest", fn(request) {
    if request.method != "POST" { server.reply_text(request, 405, "POST required") return }
    if request.path != "/mitten-droid/ui" { server.reply_text(request, 404, "Unknown route") return }
    let packet = decode(request.body_text, "ui.snapshot")
    if packet == null { server.reply_text(request, 400, "Invalid mitten-droid UI envelope") return }
    if !is_string(packet.source) || !is_string(packet.draft) || !is_string(packet.selected) {
        server.reply_text(request, 400, "Invalid snapshot fields") return
    }
    if len(packet.source) > 1200 || len(packet.draft) > 320 {
        server.reply_text(request, 413, "Preview limit exceeded") return
    }
    if packet.session != state.session {
        FileBrowser.clear(browser)
        state.action_id = ""
    }
    if !FileBrowser.set_listing(browser, packet.listing) {
        server.reply_text(request, 400, "Invalid directory listing") return
    }
    state.demo = false
    state.session = packet.session
    state.idle = 0
    if packet.action_ack == state.action_id { state.action_id = "" }
    if state.selected != packet.selected {
        state.selected = packet.selected
        selection.set_text(packet.selected + " / source preview")
    }
    if state.source != packet.source { state.source = packet.source source.set_text(packet.source) }

    status.set_text("Desktop connected / choose a folder or script")
    let reply = envelope("ui.reply", state.session, packet.request_id)
    reply.revision = state.revision
    reply.action_id = state.action_id
    reply.action_kind = state.action_kind
    reply.entry_id = state.entry_id
    server.reply_text(request, 200, JSON.stringify(reply))
})
on(server, "HttpError", fn(error) { status.set_text("Listener error / " + error.message) })
on_global("XrAxisChanged", fn(event) {
    if event.control == "LeftStick" { state.scroll_left = event.value[1] }
    if event.control == "RightStick" { state.scroll_right = event.value[1] }
})
on_global("FrameTick", fn(event) {
    state.scroll_time = state.scroll_time + event.dt_sec
    if state.scroll_time >= 0.16 {
        state.scroll_time = 0
        let scroll_y = state.scroll_left
        if Math.abs(state.scroll_right) > Math.abs(scroll_y) { scroll_y = state.scroll_right }
        if scroll_y < -0.55 { FileBrowser.scroll(browser,1) }
        if scroll_y > 0.55 { FileBrowser.scroll(browser,-1) }
    }
    let was_online = state.idle < 3
    state.idle = state.idle + event.dt_sec
    if was_online && state.idle >= 3 {
        state.action_id = ""
        FileBrowser.clear(browser)
        status.set_text("Desktop offline / waiting for connection")
    }
})
