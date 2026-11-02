import { shell_garden } from "../../assets/components/mitten_droid/garden.mms"
// Headset shell in MMS, with a native grid backed by the PC's directory listing.
import { sakura_sky } from "../../assets/components/backgrounds/sakura_sky.mms"
import { shell, caption, action } from "../../assets/components/mitten_droid/ui.mms"
import { ui_unlit_material } from "../../assets/components/materials/ui_unlit.mms"
import { envelope, decode } from "../../assets/components/mitten_droid/protocol.mms"

BGC.rgba(1.0, 0.80, 0.86, 1.0)
sakura_sky(true)
shell_garden(1.89,-1.6,-2.6,0)
AL.rgb(0.38,0.31,0.39)
T.position(23,24,-50) { DL.color(1.0,0.88,0.80).intensity(1.4) }
let pointer_material = ui_unlit_material()
fn controller_pointer(label) {
    // Stay out of view until the host supplies a valid controller Aim pose.
    return T.position(0,-20,0) {
        Pointer { name = label }
        T.position(0,0,-1.3).scale(0.0015,0.0015,2.6) { R.cube() { pointer_material C.rgba(1.0,0.76,0.25,1.0) } }
        T.position(0,0,-2.6).scale(0.012,0.012,0.012) { R.cube() { pointer_material C.rgba(1.0,0.88,0.52,1.0) } }
    }
}
T {
    name = "shell_xr_locomotion_root"
    InputXR.on() {
        InputXRGamepad { hand("left") locomotion() speed(1.4) }
        T { CXR {} }
        XRHand.new(true, "Left", "Aim") { controller_pointer("shell_left_pointer") }
        XRHand.new(true, "Right", "Aim") { controller_pointer("shell_right_pointer") }
    }
}
let browser = FileBrowser.remote()
let state = {
    scroll_right = 0, scroll_time = 0, session = "", selected = "", source = "", draft = "", idle = 10,
    action_id = "", entry_id = "", action_kind = "", revision = "", last_request = "", info_open = false
}
let status = Text { name = "shell_status" "WAITING FOR PC / port 7000" }
let info = action("Info", "shell_connection_info")
let info_text = Text { name = "shell_connection_details" "" }
let selection = Text { name = "shell_selection" "mitten-droid / MMS console" }
let source = Text { name = "shell_source" "No script selected." }
let draft_input = TextInput { name = "shell_terminal_input" "" }
let draft = Text { name = "shell_terminal_draft" "" }
// UDP remains an independent editing mode; its input is displayed through draft.
T.position(0,-100,0) { ui_unlit_material() draft_input }
// The Android host uses eye-level LOCAL space, with no floor-height offset.
T.position(-1.89, 1.0, -2.6).scale(0.030, 0.030, 0.030) {
    shell("mitten-droid / headset", "Browse your PC scripts from here", T {
        info status
        T { Style { height(1.5) font_size(0.85) } info_text }
    }, browser, T {
        Style { display("block") width(100%) }
        T { Style { margin_bottom(0.5) } caption("PC TERMINAL DRAFT") }
        T {
            Style { height(12) padding(0.7) background_color([0.08, 0.06, 0.035, 1]) }
            draft
        }
        T { Style { margin_top(0.6) font_size(0.9) } caption("Enter inserts a newline; code is not executed.") }
        T { Style { height(2) margin_top(1) } selection }
        T {
            Style { height(20) margin_top(0.8) padding(0.8) font_size(1.0) background_color([0.018, 0.016, 0.012, 1]) }
            source
        }
    }, caption("Left stick moves. Right stick or pointer drag scrolls."), false)
}
let server = HttpServer.bind("0.0.0.0:7000") {}
server
on(info, "Click", fn(event) {
    state.info_open = !state.info_open
    if state.info_open { info_text.set_text("Last PC snapshot: " + state.last_request + " / port 7000") }
    else { info_text.set_text("") }
})
on(draft_input, "TextInputChanged", fn(event) {
    if state.idle >= 3 { state.draft = event.text draft.set_text(event.text) }
})
on(browser, "DataEvent", fn(event) {
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
    if len(packet.source) > 1200 || byte_len(packet.draft) > 16384 {
        server.reply_text(request, 413, "Preview limit exceeded") return
    }
    if packet.session != state.session {
        FileBrowser.clear(browser)
        state.action_id = ""
    }
    if !FileBrowser.set_listing(browser, packet.listing) {
        server.reply_text(request, 400, "Invalid directory listing") return
    }
    let was_online = state.idle < 3
    state.session = packet.session
    state.idle = 0
    state.last_request = packet.request_id
    if packet.action_ack == state.action_id { state.action_id = "" }
    if state.selected != packet.selected {
        state.selected = packet.selected
        selection.set_text(packet.selected + " / source preview")
    }
    if state.source != packet.source { state.source = packet.source source.set_text(packet.source) }
    if state.draft != packet.draft { state.draft = packet.draft draft.set_text(packet.draft) }

    if !was_online { status.set_text("CONNECTED / PC") }
    let reply = envelope("ui.reply", state.session, packet.request_id)
    reply.revision = state.revision
    reply.action_id = state.action_id
    reply.action_kind = state.action_kind
    reply.entry_id = state.entry_id
    server.reply_text(request, 200, JSON.stringify(reply))
})
on(server, "HttpError", fn(error) { status.set_text("Listener error / " + error.message) })
on_global("XrAxisChanged", fn(event) {
    if event.control == "RightStick" { state.scroll_right = event.value[1] }
})
on_global("FrameTick", fn(event) {
    state.scroll_time = state.scroll_time + event.dt_sec
    if state.scroll_time >= 0.16 {
        state.scroll_time = 0
        let scroll_y = state.scroll_right
        if scroll_y < -0.55 { FileBrowser.scroll(browser,1) }
        if scroll_y > 0.55 { FileBrowser.scroll(browser,-1) }
    }
    let was_online = state.idle < 3
    state.idle = state.idle + event.dt_sec
    if was_online && state.idle >= 3 {
        state.action_id = ""
        FileBrowser.clear(browser)
        status.set_text("PC OFFLINE / waiting on port 7000")
    }
})
