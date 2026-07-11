# HTTP Router Draft

Status: draft only. This document defines the next authored API direction after the
implemented `HttpServer` example. It does not describe shipped runtime behavior yet.

## Implemented now

Current v1 HTTP authoring stays on the existing server event path:

- `HttpServer.bind("127.0.0.1:7000")`
- `on(server, "HttpRequest", fn(req) { ... })`
- `server.reply_text(req, status, body_text)`

`examples/http-server-example.mms` is the reference for that surface.

## Router intent

The next layer should let authored modules compose HTTP subtrees in a Hono-like way
without inventing a second execution model. Routers should decide how requests map into
engine events, while the engine signal system remains authoritative.

## Draft API direction

Introduce an authored `HttpRouter` concept:

```mms
let api = HttpRouter {
    get("/", fn(req) { ... })
    post("/emit", fn(req) { ... })
}

let admin = HttpRouter {
    post("/reload", fn(req) { ... })
}

let server = HttpServer.bind("127.0.0.1:7000") {}

server.mount("/", api)
server.mount("/admin", admin)
```

The draft scope for matching is intentionally small:

- method match
- exact path match
- path-prefix mount
- nested router composition
- explicit fallback / not-found handling

The first router phase should avoid:

- middleware stacks
- auth
- params extraction
- streaming responses

## Execution model rule

Routers must compile down to the same underlying path already used by the v1 server:

- HTTP worker receives a request
- runtime distills it into `HttpRequest`
- router matching happens against that request
- matched handler emits normal engine-side work
- reply still flows through `server.reply_text(...)`

There should not be a second callback transport or a special router-only worker model.

## Reference authored shape

The first implemented router doc/example should demonstrate:

```mms
let root = HttpRouter {
    get("/", fn(req) {
        server.reply_text(req, 200, "root\n")
    })

    mount("/api", HttpRouter {
        post("/emit", fn(req) {
            server.reply_text(req, 202, "queued\n")
        })
    })

    fallback(fn(req) {
        server.reply_text(req, 404, "not found\n")
    })
}
```

That gives one root server, one mounted sub-router, one exact route handler, and one
fallback path while still using the current reply mechanism underneath.
