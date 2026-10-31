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
T.position(-0.5,1.0,0.7) { DL.color(1.0,0.88,0.80).intensity(1.4) }
let zoom = { distance = 4.8 }
let zoom_in = action("+", "shell_zoom_in")
let zoom_out = action("-", "shell_zoom_out")
let camera = T.position(0,1.4,4.8) {
    name = "shell_camera_rig"
    C3D { Pointer {} }
    // Camera-relative controls remain reachable when the main UI is enlarged.
    T.position(0.30,-0.235,-0.5).scale(0.014,0.014,0.014) {
        name = "shell_zoom_controls"
        ui_unlit_material()
        LayoutRoot {
            available_width(9) available_height(4)
            T { Style { display("flex") flex_direction("row") column_gap(0.4) } zoom_out zoom_in }
        }
    }
}
camera
on(zoom_in,"Click",fn(event) {
    zoom.distance = zoom.distance-0.5
    if zoom.distance < 2.0 { zoom.distance = 2.0 }
    camera.update_transform([0,1.4,zoom.distance])
})
on(zoom_out,"Click",fn(event) {
    zoom.distance = zoom.distance+0.5
    if zoom.distance > 12.0 { zoom.distance = 12.0 }
    camera.update_transform([0,1.4,zoom.distance])
})
// Up can reach examples/. The root bounds native filesystem access.
let browser = FileBrowser.local("examples", "mitten-droid")
let state = {
    selected = "", source = "", draft = "", address = "127.0.0.1:7000", remote_address = "",
    connected = false, busy = false, elapsed = 0, connection = 0,
    session = "", request = 0, sent_request = "", action_ack = ""
}
let status = Text { name = "shell_status" "Offline / local browser ready" }
let selection = Text { name = "shell_selection" "Select a script" }
let source = Text { name = "shell_source" "Its source will appear here." }
let terminal_status = Text { name = "shell_terminal_status" "Click this input to send keyboard edits over UDP to headset port 7001." }
let address = TextInput { name = "shell_address" "127.0.0.1:7000" }
let editor = TextInput { name = "shell_editor" "" }
let connect = action("Connect", "shell_connect")
let disconnect = action("Disconnect", "shell_disconnect")
let refresh = action("Refresh", "shell_refresh")

T.position(-3.465, 3.65, 0).scale(0.055, 0.055, 0.055) {
    shell("mitten-droid / desktop", "Browse PC scripts, open from the headset, prepare terminal commands", T {
        T {
            Style { display("inline-block") width(40) padding(0.5) color([1.0,0.75,0.25,1]) background_color([0.035, 0.029, 0.018, 1]) }
            address
        }
        connect disconnect refresh
        T { Style { margin_top(0.7) } status }
    }, browser, T {
        Style { display("block") width(100%) }
        T { Style { height(2) } selection }
        T {
            Style { height(23) margin_top(0.8) padding(0.8) font_size(1.0) background_color([0.018, 0.016, 0.012, 1]) }
            source
        }
        T { Style { margin_top(1.1) margin_bottom(0.5) } caption("HEADSET TERMINAL / UDP KEYBOARD") }
        T {
            Style { height(9) padding(0.7) overflow("scroll") background_color([0.035, 0.029, 0.018, 1]) }
            editor
        }
        T { Style { margin_top(0.6) font_size(0.9) } terminal_status }
    }, caption("Up returns to examples/. Select a file to preview. Launch / RPC / keyboard relay are pending."))
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
    if len(packet.draft) > 320 { packet.draft = "Draft exceeds the 320 character UI preview limit." }
    packet.action_ack = state.action_ack
    state.sent_request = packet.request_id
    state.request = state.request + 1
    let body = JSON.stringify(packet)
    if len(body) > 1000000 { status.set_text("Directory listing exceeds the UI transfer limit.") return }
    state.busy = true
    client.post("http://" + state.remote_address + "/mitten-droid/ui", body)
}
on(browser, "DataEvent", fn(event) { show_selection() })
on(connect, "Click", fn(event) {
    if state.busy { return }
    state.connection = state.connection + 1
    state.session = "desktop-" + state.connection
    state.action_ack = ""
    state.remote_address = state.address
    state.connected = true
    status.set_text("Connecting to " + state.address)
    exchange()
})
on(disconnect, "Click", fn(event) {
    state.connected = false
    status.set_text("Offline / local browser ready")
})
on(address, "TextInputChanged", fn(event) { state.address = event.text })
on(editor, "TextInputChanged", fn(event) { state.draft = event.text })
on(refresh, "Click", fn(event) { FileBrowser.refresh(browser) })
on_global("FrameTick", fn(event) {
    state.elapsed = state.elapsed + event.dt_sec
    if state.elapsed >= 0.3 { state.elapsed = 0 exchange() }
})
on(client, "HttpResponse", fn(response) {
    state.busy = false
    if !state.connected { return }
    let packet = decode(response.body_text, "ui.reply")
    if response.status != 200 || packet == null {
        state.connected = false
        status.set_text("Connection rejected / HTTP " + response.status)
        return
    }
    if packet.session != state.session || packet.request_id != state.sent_request { return }
    status.set_text("Connected to " + state.remote_address + " / preview session")
    if packet.action_id != "" && packet.action_id != state.action_ack {
        FileBrowser.activate(browser, packet.revision, packet.action_kind, packet.entry_id)
        state.action_ack = packet.action_id
    }
})
on(client, "HttpError", fn(error) {
    state.busy = false
    state.connected = false
    status.set_text("Offline / " + error.message)
})
