// UI preview protocol, independent of HTTP. Execution/input are future kinds.
export fn envelope(kind, session, request_id) {
    return {
        protocol = "mitten-droid", version = 1, kind = kind,
        session = session, request_id = request_id
    }
}

export fn decode(text, kind) {
    if len(text) > 1048576 { return null }
    let packet = JSON.try_parse_object(text)
    if packet == null { return null }
    if packet.protocol != "mitten-droid" || packet.version != 1 || packet.kind != kind { return null }
    if !is_string(packet.session) || !is_string(packet.request_id) { return null }
    if len(packet.session) > 128 || len(packet.request_id) > 128 { return null }
    return packet
}
