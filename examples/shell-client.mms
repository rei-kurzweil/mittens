import { shell_garden } from "../assets/components/mitten_droid/garden.mms"
// PC shell in MMS; directory enumeration and the 5 x 4 browser grid use Rust.
import { shell, caption, action } from "../assets/components/mitten_droid/ui.mms"
import { sakura_sky } from "../assets/components/backgrounds/sakura_sky.mms"
import { ui_unlit_material } from "../assets/components/materials/ui_unlit.mms"
import { envelope, decode } from "../assets/components/mitten_droid/protocol.mms"

RendererSettings { window_size(1440, 960) }
BGC.rgba(1.0, 0.80, 0.86, 1.0)
sakura_sky(false)
shell_garden(3.465,-0.2,0,4.8)
AL.rgb(0.38,0.31,0.39)
T.position(-0.65,0.47,-1.0) { DL.color(1.0,0.88,0.80).intensity(1.4) }
I.speed(2.0) {
    InputTransformMode.forward_z() { roll_axis_y() fps_rotation() }
    T.position(0,1.4,4.8) {
        name = "shell_camera_rig"
        C3D { Pointer {} }
    }
}
// Up can reach examples/. The root bounds native filesystem access.
let browser = FileBrowser.local("examples", "mitten-droid")
let state = {
    selected = "", source = "", draft = "", address = "127.0.0.1:7000", remote_address = "",
    connected = false, confirmed = false, auto_connect = false, busy = false, elapsed = 0, connection = 0,
    retry = false, retry_elapsed = 0, paused = false, info_open = false,
    session = "", request = 0, sent_request = "", last_reply = "", action_ack = ""
}
let status = Text { name = "shell_status" "OFFLINE / enter headset IP and click Connect" }
let info = action("Info", "shell_connection_info")
let info_text = Text { name = "shell_connection_details" "" }
let connection_mode = Text { name = "shell_connection_mode" "offline" }
T.position(0,-100,0) { ui_unlit_material() connection_mode }
let selection = Text { name = "shell_selection" "Select a script" }
let source = Text { name = "shell_source" "Its source will appear here." }
let terminal_status = Text { name = "shell_terminal_status" "Connect to mirror the terminal draft over HTTP." }
let address = TextInput { name = "shell_address" "127.0.0.1:7000" }
let editor = TextInput { name = "shell_editor" "" }
let connect = action("Connect", "shell_connect")
let disconnect = action("Disconnect", "shell_disconnect")
let refresh = action("Refresh", "shell_refresh")

T.position(-3.465, 3.65, 0).scale(0.055, 0.055, 0.055) {
    shell("mitten-droid / desktop", "Browse PC scripts, open from the headset, prepare terminal commands", T {
        T {
            Raycastable.enabled()
            Style { display("inline-block") width(40) padding(0.5) color([1.0,0.75,0.25,1]) background_color([0.035, 0.029, 0.018, 1]) }
            address
        }
        connect disconnect refresh info
        T { Style { margin_top(0.7) } status }
        T { Style { height(1.5) font_size(0.85) } info_text }
    }, browser, T {
        Style { display("block") width(100%) }
        T { Style { height(2) margin_bottom(0.5) font_size(0.9) } terminal_status }
        T { Style { margin_bottom(0.5) } caption("TERMINAL INPUT / CLICK HERE TO TYPE") }
        T {
            name = "shell_editor_box"
            Raycastable.enabled()
            Style { height(12) padding(0.7) background_color([0.08, 0.06, 0.035, 1]) }
            editor
        }
        T {
            name = "shell_editor_focus_indicator"
            Style { display("block") width(0.9) height(3.5) margin_top(0.3) background_color([0.018, 0.016, 0.012, 1]) }
        }
        T { Style { height(2) margin_top(1) } selection }
        T {
            Style { height(20) margin_top(0.8) padding(0.8) font_size(1.0) background_color([0.018, 0.016, 0.012, 1]) }
            source
        }
    }, caption("Up returns to examples/. Grab a title bar to move a panel."), true)
}
let client = HttpClient.timeout_ms(2000) {}
client

fn show_selection() {
    let selected = FileBrowser.selection(browser)
    state.selected = selected.name
    state.source = selected.source
    selection.set_text(selected.name + " / source preview")
    source.set_text(selected.source)
}
fn exchange() {
    if !state.connected || state.busy { return }
    let packet = envelope("ui.snapshot", state.session, "" + state.request)
    packet.listing = FileBrowser.snapshot(browser)
    packet.selected = state.selected
    packet.source = state.source
    packet.draft = state.draft
    if byte_len(packet.draft) > 16384 {
        if !state.paused {
            state.paused = true
            connection_mode.set_text("paused")
            status.set_text("SYNC PAUSED / draft exceeds 16 KiB")
        }
        return
    }
    if state.paused {
        state.paused = false
        if state.confirmed { connection_mode.set_text("http") status.set_text("CONNECTED / " + state.remote_address) }
    }
    packet.action_ack = state.action_ack
    state.sent_request = packet.request_id
    state.request = state.request + 1
    let body = JSON.stringify(packet)
    if len(body) > 1000000 { status.set_text("Directory listing exceeds the UI transfer limit.") return }
    state.busy = true
    if !state.confirmed { status.set_text("CONNECTING / " + state.remote_address + " / waiting for reply") }
    client.post("http://" + state.remote_address + "/mitten-droid/ui", body)
}
on(browser, "DataEvent", fn(event) { show_selection() })
fn start_connection() {
    if state.busy { return }
    state.retry = false
    state.retry_elapsed = 0
    state.paused = false
    state.connection = state.connection + 1
    state.session = "desktop-" + state.connection
    state.action_ack = ""
    state.remote_address = state.address
    state.connected = true
    state.confirmed = false
    connection_mode.set_text("connecting")
    status.set_text("CONNECTING / " + state.address + " / waiting for reply")
    exchange()
}
on(connect, "Click", fn(event) { start_connection() })
on(info, "Click", fn(event) {
    state.info_open = !state.info_open
    if state.info_open { info_text.set_text("Last reply: " + state.last_reply + " / session: " + state.session) }
    else { info_text.set_text("") }
})
on(disconnect, "Click", fn(event) {
    state.connected = false
    state.confirmed = false
    state.retry = false
    connection_mode.set_text("offline")
    status.set_text("OFFLINE / click Connect to retry")
})
on(address, "TextInputChanged", fn(event) { state.address = event.text })
on(editor, "TextInputChanged", fn(event) { state.draft = event.text })
on(refresh, "Click", fn(event) { FileBrowser.refresh(browser) })
on_global("FrameTick", fn(event) {
    if state.auto_connect { state.auto_connect = false start_connection() }
    if state.retry {
        state.retry_elapsed = state.retry_elapsed + event.dt_sec
        if state.retry_elapsed >= 5 { start_connection() }
    }
    state.elapsed = state.elapsed + event.dt_sec
    if state.elapsed >= 0.3 { state.elapsed = 0 exchange() }
})
on(client, "HttpResponse", fn(response) {
    state.busy = false
    if !state.connected { return }
    let packet = decode(response.body_text, "ui.reply")
    if response.status != 200 || packet == null {
        state.connected = false
        state.confirmed = false
        connection_mode.set_text("offline")
        status.set_text("Connection rejected / HTTP " + response.status)
        return
    }
    if packet.session != state.session || packet.request_id != state.sent_request { return }
    state.last_reply = packet.request_id
    if !state.confirmed {
        state.confirmed = true
        connection_mode.set_text("http")
        status.set_text("CONNECTED / " + state.remote_address)
    }
    if packet.action_id != "" && packet.action_id != state.action_ack {
        FileBrowser.activate(browser, packet.revision, packet.action_kind, packet.entry_id)
        state.action_ack = packet.action_id
    }
})
on(client, "HttpError", fn(error) {
    state.busy = false
    if !state.connected { return }
    state.connected = false
    state.confirmed = false
    state.retry = true
    state.retry_elapsed = 0
    connection_mode.set_text("retrying")
    status.set_text("NO REPLY / " + state.remote_address + " / retrying in 5s")
})
