import { info_panel } from "./info_panel.mms"

let TRACK = [0.012, 0.014, 0.020, 1.0]
let RED = [0.74, 0.10, 0.15, 1.0]
let ORANGE = [0.93, 0.33, 0.075, 1.0]
let YELLOW = [0.95, 0.74, 0.12, 1.0]
let GREEN = [0.20, 0.68, 0.33, 1.0]
let INK = [0.13, 0.11, 0.19, 1.0]
let MUTED = [0.36, 0.32, 0.42, 1.0]
let BAR_WIDTH = 7.6
let BAR_HEIGHT = 0.74

// These describe the owning asset's placeholder needs. A GLB and a texture
// that belong to the same vehicle can both carry "large".
let KIND_LABELS = {
    skinned_humanoid = "Skinned humanoid"
    humanoid_size = "Humanoid size"
    small = "Small object"
    large = "Large object"
    script = "Script"
}

fn file_details(file) {
    let label = KIND_LABELS[file.asset_kind]
    if label == null { label = "Asset" }
    if file.size_bytes >= 1048576 {
        return label + " · " + Math.round(file.size_bytes / 104857.6) / 10.0 + " MiB"
    }
    if file.size_bytes >= 1024 { return label + " · " + Math.round(file.size_bytes / 1024) + " KiB" }
    return label + " · " + file.size_bytes + " B"
}

fn fraction(row) {
    if row.phase == "complete" || row.phase == "verifying" { return 1.0 }
    if row.phase == "skipped" || row.file.size_bytes == 0 { return 0.0 }
    return Math.clamp(row.bytes_done / row.file.size_bytes, 0.0, 1.0)
}

fn track_color(row, red) {
    if row.phase == "failed" || (row.phase == "transferring" && row.bytes_done == 0 && red) { return RED }
    return TRACK
}

fn progress_color(row) {
    if row.phase == "complete" { return GREEN }
    if row.phase == "failed" { return RED }
    if row.phase == "verifying" { return YELLOW }
    let t = fraction(row)
    return [ORANGE[0] + (YELLOW[0] - ORANGE[0]) * t,
        ORANGE[1] + (YELLOW[1] - ORANGE[1]) * t,
        ORANGE[2] + (YELLOW[2] - ORANGE[2]) * t, 1.0]
}

fn progress_text(row) {
    if row.phase == "queued" { return "Queued" }
    if row.phase == "verifying" { return "Verifying" }
    if row.phase == "complete" { return "100% · Done" }
    if row.phase == "failed" { return "Failed" }
    if row.phase == "skipped" { return "Skipped" }
    if row.bytes_done == 0 { return "Starting" }
    return Math.floor(fraction(row) * 100.0) + "%"
}

fn status_text(phase) {
    if phase == "waiting_for_peer" { return "Waiting for peer approval" }
    if phase == "transferring" { return "Transferring files" }
    if phase == "complete" { return "Transfer complete" }
    if phase == "declined" { return "Transfer declined" }
    if phase == "failed" { return "Transfer failed" }
    return "All listed files are included"
}

fn label(text, size, text_color, wrap) {
    return T {
        Style {
            display("block") width(100%) font_size(size) color(text_color)
            if wrap { word_wrap("break_all") }
        }
        Text { text }
    }
}

fn included_switch(unit) {
    // V1 presents the selected state. The offer is accepted as one unit.
    return T {
        name = "asset_transfer_file_included"
        Raycastable.disabled()
        Style {
            display("block") width(2.4) height(1.3)
            background_color([0.31, 0.27, 0.45, 1.0])
        }
        T.position(1.72 * unit, -0.65 * unit, 0.025).scale(0.82 * unit, 0.82 * unit, 1.0) {
            R.circle_2d() { C.rgba(0.95, 0.92, 0.99, 1.0) }
        }
    }
}

fn progress_bar(row, unit, red) {
    let bar_width = BAR_WIDTH * unit
    let bar_height = BAR_HEIGHT * unit
    let fill_width = Math.clamp(fraction(row) * bar_width, 0.00001, bar_width)
    let fill_color = progress_color(row)
    let background = track_color(row, red)
    return T {
        Raycastable.disabled()
        Style { display("flex") flex_direction("column") width(BAR_WIDTH) row_gap(0.19) }
        T {
            Style { display("block") width(BAR_WIDTH) height(BAR_HEIGHT) }
            T.position(bar_width / 2.0, -bar_height / 2.0, 0.014).scale(bar_width, bar_height, 1.0) {
                R.plane() { name = "asset_progress_track" C.rgba(background[0], background[1], background[2], background[3]) }
            }
            T.position(fill_width / 2.0, -bar_height / 2.0, 0.018).scale(fill_width, bar_height, 1.0) {
                name = "asset_progress_fill"
                R.plane() { name = "asset_progress_color" C.rgba(fill_color[0], fill_color[1], fill_color[2], fill_color[3]) }
            }
        }
        T {
            Style { display("block") width(BAR_WIDTH) font_size(0.53) color(MUTED) }
            Text { name = "asset_progress_text" progress_text(row) }
        }
    }
}

fn file_row(row, unit, red) {
    return T {
        name = row.name
        Style {
            display("flex") flex_direction("row") width(100%) align_items("center")
            gap(0.58) padding_xy(0.60, 0.54) background_color([0.91, 0.89, 0.94, 1.0]) background_z(-0.01)
        }
        included_switch(unit)
        T {
            Style { display("flex") flex_direction("column") width(0.0) flex_grow(1.0) row_gap(0.20) }
            label(row.file.name, 0.66, INK, true)
            label(file_details(row.file), 0.49, MUTED)
        }
        progress_bar(row, unit, red)
    }
}

fn decision_button(name, text, background, foreground, action) {
    let control = T {
        name = name
        Grabbable.off()
        Raycastable.click_only() { interaction_priority(160.0) }
        Style {
            display("flex") width(7.4) height(2.45) align_items("center") justify_content("center")
            background_color(background) background_z(0.010) color(foreground) font_size(0.69)
        }
        T.position(0.0, 0.0, 0.03) { Raycastable.disabled() Text { text } }
    }
    on(control, "Click", fn(event) { action() })
    return control
}

// transfer: { uploads, downloads, accept(), decline() }. Each file has a
// direction-local string id, name, size_bytes, and asset_kind. Optional
// bytes_done and phase fields can populate a panel opened mid-transfer.
// options: root_name, title, width_gu, unit_scale, initial_state.
// set_progress(direction, file_id, bytes_done, phase) uses "upload" or "download".
export fn asset_transfer_panel(transfer, options) {
    if options == null { options = {} }
    let root_name = options["root_name"]
    if root_name == null { root_name = "asset_transfer_panel" }
    let title = options["title"]
    if title == null { title = "Asset transfer" }
    let panel_width = options["width_gu"]
    if panel_width == null { panel_width = 55.0 }
    let unit = options["unit_scale"]
    if unit == null { unit = 0.055 }
    let phase = options["initial_state"]
    if phase == null { phase = "offered" }
    let state = { root = null phase = phase red = false }
    let files = { upload = transfer.uploads download = transfer.downloads }
    let rows = {}

    fn key(direction, id) { return direction + "/" + id }
    for direction in ["upload", "download"] {
        let list = files[direction]
        for index in range(len(list)) {
            let file = list[index]
            let bytes_done = file["bytes_done"]
            if bytes_done == null { bytes_done = 0 }
            let row_phase = file["phase"]
            if row_phase == null { row_phase = "queued" }
            rows[key(direction, file.id)] = {
                name = "asset_transfer_" + direction + "_" + index
                file = file bytes_done = bytes_done phase = row_phase
            }
        }
    }

    fn refresh_track(view, row) {
        view.query("#asset_progress_track").set_color(track_color(row, state.red))
    }

    fn set_progress(direction, id, bytes_done, row_phase) {
        let row = rows[key(direction, id)]
        if !row { return }
        row.bytes_done = Math.clamp(bytes_done, 0, row.file.size_bytes)
        row.phase = row_phase
        let view = state.root.query("#" + row.name)
        if !view { return }
        let bar_width = BAR_WIDTH * unit
        let fill_width = Math.clamp(fraction(row) * bar_width, 0.00001, bar_width)
        let bar_height = BAR_HEIGHT * unit
        view.query("#asset_progress_fill").update_transform(
            [fill_width / 2.0, -bar_height / 2.0, 0.018], [0.0, 0.0, 0.0], [fill_width, bar_height, 1.0])
        view.query("#asset_progress_color").set_color(progress_color(row))
        view.query("#asset_progress_text").set_text(progress_text(row))
        refresh_track(view, row)
    }

    fn set_state(value) {
        if state.phase == value { return }
        state.phase = value
        let status = state.root.query("#asset_transfer_status")
        if status { status.set_text(status_text(value)) }
        let actions = state.root.query("#asset_transfer_actions")
        if actions && value != "offered" { actions.remove_subtree() }
        if value == "declined" {
            for direction in ["upload", "download"] {
                for file in files[direction] {
                    if rows[key(direction, file.id)].phase == "queued" {
                        set_progress(direction, file.id, 0, "skipped")
                    }
                }
            }
        }
    }

    fn accept() {
        if state.phase != "offered" { return }
        set_state("waiting_for_peer")
        transfer.accept()
    }

    fn decline() {
        if state.phase != "offered" { return }
        set_state("declined")
        transfer.decline()
    }

    fn flash(red) {
        state.red = red
        if state.phase != "transferring" { return }
        for direction in ["upload", "download"] {
            for file in files[direction] {
                let row = rows[key(direction, file.id)]
                if row.phase == "transferring" && row.bytes_done == 0 {
                    let view = state.root.query("#" + row.name)
                    if view { refresh_track(view, row) }
                }
            }
        }
    }

    fn column(direction, title, empty) {
        let list = files[direction]
        return T {
            name = "asset_transfer_" + direction + "s"
            Style {
                display("flex") flex_direction("column") width(0.0) flex_grow(1.0)
                row_gap(0.42)
            }
            label(title, 0.88, INK)
            if len(list) == 0 { label(empty, 0.59, MUTED) }
            T {
                Style {
                    display("flex") flex_direction("column") width(100%) row_gap(0.42)
                    if len(list) > 6 { height(23.0) overflow("scroll") }
                }
                for file in list { file_row(rows[key(direction, file.id)], unit, state.red) }
            }
        }
    }

    fn content() {
        return T {
            Style { display("flex") flex_direction("column") width(100%) row_gap(0.80) }
            T {
                Style { display("flex") flex_direction("row") width(100%) align_items("start") gap(1.20) }
                column("upload", "Uploads", "No files to upload")
                column("download", "Downloads", "No files to download")
            }
            T {
                Style { display("flex") flex_direction("row") width(100%) align_items("center") height(2.45) }
                T {
                    Style { display("block") width(0.0) flex_grow(1.0) font_size(0.59) color(MUTED) }
                    Text { name = "asset_transfer_status" status_text(state.phase) }
                }
                if state.phase == "offered" {
                    T {
                        name = "asset_transfer_actions"
                        Style { display("flex") flex_direction("row") gap(0.50) }
                        decision_button("asset_transfer_decline", "Decline", [0.86, 0.83, 0.89, 1.0], INK, decline)
                        decision_button("asset_transfer_accept", "Accept all", [0.36, 0.30, 0.53, 1.0], [1.0, 1.0, 1.0, 1.0], accept)
                    }
                }
            }
        }
    }

    let root = info_panel({
        root_name = root_name title = title width_gu = panel_width unit_scale = unit
        title_background_color = [0.25, 0.20, 0.38, 1.0]
        title_text_color = [1.0, 1.0, 1.0, 1.0]
        toggle_background_color = [0.33, 0.27, 0.49, 1.0]
        toggle_icon_color = [0.88, 0.84, 0.98, 1.0]
        body_background_color = [0.97, 0.95, 0.98, 1.0]
        body_text_color = INK body_padding = 0.95 content_factory = content
    })
    state.root = root
    let pulse = Animation.looping().length(2.0) {
        Keyframe.at(0.0) { flash(true) }
        Keyframe.at(1.0) { flash(false) }
    }
    root.attach(pulse)
    return { root = root set_progress = set_progress set_state = set_state }
}
