# HTTP / Router / MCP Roadmap

Status split:

- Implemented now: Phase 0 example only
- Draft next: Phases 1 and 2
- Later public API work: Phases 3 and 4

## Phase 0: implemented now

`examples/http-server-example.mms` and `examples/http-server-example.rs` demonstrate the
current authored `HttpServer` surface on a star-kawaii scene.

The implemented contract is deliberately small:

- bind `127.0.0.1:7000`
- receive `HttpRequest`
- accept `POST /`
- update visible scene text with the latest method, path, and body text
- reply `200` on success
- reply `404` for non-root paths
- reply `405` for non-`POST` methods

This is a proof that MMS-authored HTTP handling works on the existing event/reply path.
It is not the final network API shape.

## Phase 1: router draft

Next, define modular HTTP routers before implementing a generalized runtime.

Goals:

- model routers as authored HTTP subtrees
- support method match, exact path, path-prefix mount, and nesting
- attach routers under an `HttpServer`
- preserve the current `HttpRequest` plus `reply_text` execution model

Non-goals for the first router draft:

- middleware
- auth
- path params
- streaming

See `docs/draft/http-router-draft.md`.

## Phase 2: HTTP-to-signal bridge

After the raw example exists, add a second explicit transport for engine messages rather
than overloading every `POST /`.

First envelope direction:

```json
{
  "kind": "event",
  "name": "DataEvent",
  "target": "some-explicit-scope",
  "payload": { "...": "..." }
}
```

Initial categories:

- `event`
- `intent`

Safety rules for the first bridge:

- keep the plain `HttpRequest` example unchanged as the baseline
- worker thread only decodes transport text
- main thread validates and decides whether enqueue is allowed
- network payloads do not get arbitrary `IntentValue` construction by default
- start with a curated whitelist such as `DataEvent`
- delivery target must be explicit, not implicit global broadcast

Processing model:

1. HTTP worker receives request.
2. Worker decodes a UTF-8 JSON envelope.
3. Worker sends a distilled message to the main thread.
4. Main thread validates and converts into `Signal` or `IntentSignal`.
5. Normal `RxWorld` / executor flow handles it.
6. HTTP response returns after enqueue acceptance, not after downstream completion.

## Phase 3: typed engine HTTP API

Once the bridge exists, define a stable external HTTP API above it.

Purpose:

- expose deliberate engine capabilities instead of raw internal enums
- version the public HTTP API independently from internal engine signal shapes

Likely capability split:

- emit event
- request safe intent
- query world state
- reserve streaming/subscription for later

The core rule is to prefer stable named operations, not arbitrary serialized internal
enum transport.

## Phase 4: MCP server placement

MCP comes after the transport and typed HTTP API are understood.

Role:

- MCP is the LLM-facing tools layer
- it should expose curated cat-engine capabilities, not low-level transport details

Likely initial MCP surface:

- list roots / inspect subtree / read text or status
- emit named safe `DataEvent`
- invoke curated authored actions
- possibly add constrained module load/run flows later

Placement rule:

- `HttpServer` remains engine/runtime infrastructure
- the typed engine HTTP API is one substrate
- MCP sits above that substrate as a semantic tool contract

This ordering avoids shaping the lowest transport layer around one client class and lets
MCP reuse the same stable API that human-operated tools could also target.
