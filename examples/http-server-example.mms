import { button } from "../assets/components/button.mms"
import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"

RendererSettings {
    window_size(1280, 960)
}

BGC {
    C.rgba(0.03, 0.02, 0.10, 1.0)
}
AL.rgb(0.18, 0.16, 0.30)

RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.05)
            half_res(true)
        }
    }
    Bloom {
        intensity(1.0)
        radius_ndc(0.05)
        emissive_scale(1.15)
        half_res(true)
    }
}

BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.84, 0.18, 1.0])
}

T.position(0.0, 3.4, 3.0) {
    PL {
        intensity(4.5)
        distance(180.0)
        color(1.0, 0.95, 1.0)
    }
}

T.position(-2.2, 1.1, -5.8) {
    PL {
        intensity(2.6)
        distance(90.0)
        color(0.45, 0.80, 1.0)
    }
}

I.speed(1.5) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }
    T.position(0.0, 0.25, 8.2) {
        C3D {
            Pointer {}
        }
    }
}

T.position(0.0, -2.8, 0.0).scale(10000.0, 0.1, 10000.0) {
    R.cube() {
        C.rgba(0.04, 0.02, 0.08, 1.0)
    }
}

let scene_root = T.position(-3.9, 2.5, -2.6).scale(0.08, 0.08, 0.08) {
    name = "http_server_demo"
    LayoutRoot {
        name = "http_server_layout"
        available_width(22.0)
        available_height(12.6)

        T {
            name = "http_panel_shell"
            Style {
                display("block")
                width(22.0)
                height(12.6)
                padding(1.0)
                background_color = [0.08, 0.07, 0.16, 0.96]
            }

            T {
                Style {
                    display("block")
                    margin_bottom(0.6)
                }
                Text {
                    "HTTP server demo"
                    C.rgba(1.0, 0.92, 0.70, 1.0)
                    TextureFiltering.linear()
                    EM.on() {
                        intensity(1.8)
                    }
                }
            }

            T {
                Style {
                    display("block")
                    margin_bottom(0.4)
                }
                Text {
                    "POST http://127.0.0.1:7000/"
                    C.rgba(0.82, 0.93, 1.0, 1.0)
                    TextureFiltering.linear()
                    EM.on() {
                        intensity(1.2)
                    }
                }
            }

            T {
                Style {
                    display("block")
                    margin_bottom(0.9)
                }
                Text {
                    "Try: curl -X POST http://127.0.0.1:7000/ -d 'hello'"
                    C.rgba(0.72, 0.82, 1.0, 1.0)
                    TextureFiltering.linear()
                }
            }

            T {
                name = "request_row"
                Style {
                    display("block")
                    margin_bottom(0.8)
                }

                T {
                    name = "status_panel"
                    Style {
                        display("inline-block")
                        width(14.8)
                        height(5.8)
                        margin_right(1.0)
                        padding(0.8)
                        background_color = [0.13, 0.11, 0.24, 1.0]
                    }

                    T {
                        Style {
                            display("block")
                            margin_bottom(0.6)
                        }
                        Text {
                            "Last accepted request"
                            C.rgba(1.0, 0.82, 0.48, 1.0)
                            TextureFiltering.linear()
                            EM.on() {
                                intensity(1.0)
                            }
                        }
                    }

                    T {
                        Style {
                            display("block")
                        }
                        Text {
                            name = "http_status_text"
                            "waiting for POST /"
                            C.rgba(0.95, 0.97, 1.0, 1.0)
                            TextureFiltering.linear()
                        }
                    }
                }

                T {
                    name = "send_button_mount"
                    Style {
                        display("inline-block")
                        vertical_align("middle")
                        margin_top(1.7)
                    }
                    button("send", {
                        background_color = [0.18, 0.48, 0.88, 1.0]
                        color = [0.98, 0.98, 1.0, 1.0]
                    })
                }
            }

            T {
                Style {
                    display("block")
                }
                Text {
                    "Non-root -> 404\nNon-POST -> 405"
                    C.rgba(0.70, 0.76, 0.92, 1.0)
                    TextureFiltering.linear()
                }
            }
        }
    }
}

scene_root

let server = HttpServer.bind("127.0.0.1:7000") {}
server
let client = HttpClient {}
client

let status_text = query("#http_status_text")
let send_button = scene_root.query("#send_button_mount").query("#button_root")

on(server, "HttpRequest", fn(req) {
    if req.method != "POST" {
        server.reply_text(req, 405, "method not allowed: POST only\n")
        return
    }

    if req.path != "/" {
        server.reply_text(req, 404, "not found\n")
        return
    }

    if status_text {
        status_text.set_text(
            "method: " + req.method + "\n"
            + "path: " + req.path + "\n"
            + "body: " + req.body_text
        )
    }

    server.reply_text(req, 200, "accepted POST /\n")
})

on(send_button, "Click", fn(event) {
    if status_text {
        status_text.set_text(
            "sending to http://127.0.0.1:7000/\n"
            + "body: mrrow mrrp nya?"
        )
    }
    client.post("http://127.0.0.1:7000/", "mrrow mrrp nya?")
})

on(client, "HttpResponse", fn(resp) {
    if status_text {
        status_text.set_text(
            "sent: mrrow mrrp nya?\n"
            + "response: " + resp.status + "\n"
            + "body: " + resp.body_text
        )
    }
})

on(client, "HttpError", fn(err) {
    if status_text {
        status_text.set_text(
            "send failed\n"
            + "phase: " + err.phase + "\n"
            + "message: " + err.message
        )
    }
})
