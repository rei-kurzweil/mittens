// cargo run --release -- load assets/components/ui/asset_transfer_panel.preview.mms
import { asset_transfer_panel } from "./asset_transfer_panel.mms"

RendererSettings { window_size(1240, 760) }
BGC.rgba(0.095, 0.080, 0.13, 1.0)
Clock.bpm(120.0)
T.position(0.0, 1.46, 3.2) { C3D { fov(53.0) Pointer {} } }

let uploads = [
    { id = "pilot" name = "pilot.glb" size_bytes = 26843546 asset_kind = "skinned_humanoid" },
    { id = "rules" name = "lighting_rules.mms" size_bytes = 9421 asset_kind = "script" },
    { id = "tool" name = "hand_tool.glb" size_bytes = 461200 asset_kind = "small" },
]
let downloads = [
    { id = "interceptor" name = "interceptor.glb" size_bytes = 3846000 asset_kind = "large" },
    { id = "stand" name = "display_mannequin.glb" size_bytes = 851200 asset_kind = "humanoid_size" },
]
let playback = { running = false index = 0 seconds = 0.0 }
let transfer = {
    uploads = uploads downloads = downloads
    accept = fn() { playback.running = true }
    decline = fn() { playback.running = false }
}
let panel = asset_transfer_panel(transfer, { title = "Asset transfer · Preview" })
T.position(-1.5125, 2.09, 0.0) { panel.root }

// Exercise the panel's normal progress interface with a sequential mock transfer.
on_global("FrameTick", fn(event) {
    if !playback.running { return }
    if playback.index == 0 && playback.seconds == 0.0 { panel.set_state("transferring") }
    let direction = "upload"
    let file = uploads[playback.index]
    if playback.index >= len(uploads) {
        direction = "download"
        file = downloads[playback.index - len(uploads)]
    }
    playback.seconds = playback.seconds + event.dt_sec
    if playback.seconds < 0.9 {
        panel.set_progress(direction, file.id, 0, "transferring")
    } else if playback.seconds < 2.6 {
        let fraction = (playback.seconds - 0.9) / 1.7
        panel.set_progress(direction, file.id, Math.floor(fraction * file.size_bytes), "transferring")
    } else if playback.seconds < 2.9 {
        panel.set_progress(direction, file.id, file.size_bytes, "verifying")
    } else {
        panel.set_progress(direction, file.id, file.size_bytes, "complete")
        playback.seconds = 0.0
        playback.index = playback.index + 1
        if playback.index == len(uploads) + len(downloads) {
            playback.running = false
            panel.set_state("complete")
        }
    }
})
