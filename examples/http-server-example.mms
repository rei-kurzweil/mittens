import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"

RendererSettings {
    window_size(1280, 960)
}

BGC.with_occlusion_and_lighting() {
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

    T.position(0.0, 0.0, -0.1).scale(22.0, 12.6, 0.3) {
        R.cube() {
            C.rgba(0.08, 0.07, 0.16, 0.96)
            EM.on() {
                intensity(0.18)
            }
        }
    }

    T.position(0.0, 5.0, 0.1) {
        Text {
            "HTTP server demo"
            C.rgba(1.0, 0.92, 0.70, 1.0)
            TextureFiltering.linear()
            EM.on() {
                intensity(1.8)
            }
        }
    }

    T.position(0.0, 3.8, 0.1) {
        Text {
            "POST http://127.0.0.1:7000/"
            C.rgba(0.82, 0.93, 1.0, 1.0)
            TextureFiltering.linear()
            EM.on() {
                intensity(1.2)
            }
        }
    }

    T.position(0.0, 2.8, 0.1) {
        Text {
            "Try: curl -X POST http://127.0.0.1:7000/ -d 'hello'"
            C.rgba(0.72, 0.82, 1.0, 1.0)
            TextureFiltering.linear()
        }
    }

    T.position(0.0, 0.2, 0.1).scale(18.0, 5.8, 0.2) {
        R.cube() {
            C.rgba(0.13, 0.11, 0.24, 1.0)
        }
    }

    T.position(-7.8, 1.55, 0.2) {
        Text {
            "Last accepted request"
            C.rgba(1.0, 0.82, 0.48, 1.0)
            TextureFiltering.linear()
            EM.on() {
                intensity(1.0)
            }
        }
    }

    T.position(-7.8, 0.35, 0.2) {
        Text {
            name = "http_status_text"
            "waiting for POST /"
            C.rgba(0.95, 0.97, 1.0, 1.0)
            TextureFiltering.linear()
        }
    }

    T.position(-7.8, -2.0, 0.2) {
        Text {
            "Non-root -> 404\nNon-POST -> 405"
            C.rgba(0.70, 0.76, 0.92, 1.0)
            TextureFiltering.linear()
        }
    }
}

scene_root

let server = HttpServer.bind("127.0.0.1:7000") {}
server

let status_text = query("#http_status_text")

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
