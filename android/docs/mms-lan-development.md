# MMS development over the Focus 3 LAN

Status: design proposal, 2026-10-31. The endpoint, UDP component, and incremental
Universe API below are proposed; this document does not enable a listener.

The intended workflow is:

```text
Mittens example on Arch PC
  -> Ethernet -> travel router -> Wi-Fi 6
  -> Android Mittens host on Focus 3
  -> evaluate MMS against the running headset scene
  -> return result or error to the PC example
```

Once a host APK with this facility is installed, launching it from the headset
should be enough to reconnect. ADB is independent of this connection. This
provides live MMS editing while the app runs; installing a new native build or
recovering a crashed host still needs a separate mechanism.

## Separate the socket from remote evaluation

Add a general `UdpSocket` component alongside `HttpClient` and `HttpServer`.
Receiving a datagram produces an MMS event; interpreting it as code is an
explicit application decision.

Use HTTP for the first remote evaluation endpoint. The PC already has
`HttpClient.post(url, body_text)` and `HttpResponse`/`HttpError` events. UDP is
[unordered and unreliable](https://doc.rust-lang.org/std/net/struct.UdpSocket.html):
retrying a command whose result was lost can repeat its scene changes. Remote
evaluation over UDP would need application request IDs, duplicate suppression,
timeouts, and payload limits from the start. HTTP still needs explicit retry
semantics when a connection fails after execution.

Both transports can eventually feed the same evaluation queue. UDP is also
useful independently for telemetry and frequently updated state where a newer
sample supersedes a lost one, including the live editor draft described below.

## Desktop editor and live headset preview

The proposed example names follow the network roles:

| File | Responsibility |
| --- | --- |
| `examples/shell-client.mms` | Winit-hosted desktop editor: connection settings, multiline input, Run button, result/error display. |
| `android/examples/shell-server.mms` | Headset endpoint and scene: display the incoming draft without evaluating it, and execute an explicitly submitted snippet. |

These files do not exist yet. The headset has a read-only draft/result display;
the PC owns editing. Preserve the desktop draft while a previous submission
executes, so the user can keep typing.

Treat preview and execution as separate operations:

```text
desktop TextInputChanged -> latest draft snapshot -> headset Text.set_text
desktop Run              -> exact source snapshot -> MMS evaluation -> result
```

Send the edited text rather than replaying individual physical key events. This
handles insertion, deletion, paste, and committed input-method text consistently,
and a later snapshot repairs a missed update. The transport message can include
the caret too. `TextInputChanged` currently includes text and caret after an edit;
mirroring cursor-only movement would need its own notification.

A draft request to `POST /mms/draft` would add these fields to the authenticated,
versioned session envelope:

```json
{
  "editor": "pc-editor-instance-001",
  "revision": 42,
  "source": "let cube = T.pos",
  "caret": 16
}
```

The server associates the active editor instance with the authenticated client
and accepts only newer revisions from it. Switching editor instances is an
explicit connection/handshake operation, so a delayed packet from an old editor
cannot replace the new draft. Incomplete or syntactically invalid MMS is normal
in a draft: displaying it must never invoke the parser/evaluator.

For an HTTP first version, send on edits with at most one preview request in
flight, coalescing intervening edits into the latest pending snapshot. A short
update cadence can additionally cap preview work; choose it after measuring
typing-to-display latency on the actual LAN. Never build an ever-growing queue
of old text. Acknowledge the accepted revision, and resend the current snapshot
on reconnect. Use an independent execution request so a Run response does not
hold up the preview queue.

UDP is a reasonable optional transport for these disposable preview snapshots.
Discard old revisions, periodically resend the latest draft or acknowledge and
retry that revision so losing the final edit cannot leave the display stale.
Apply the same authentication, editor identity, and generation checks. If the
full envelope exceeds the datagram limit, use HTTP for that snapshot; do not
assume an entire editor buffer fits in one packet. Start with full snapshots;
incremental edit protocols would require explicit resynchronization.

Run remains an HTTP request carrying its own `source`, editor instance, revision,
and unique execution ID. Execute that immutable source, regardless of whether
its preview has arrived. Never implement Run as "execute whatever text the
server most recently received." Return the executed revision with the result;
an old completion must not overwrite the newer draft currently being typed.

The repository already provides useful implementation pieces:

- [TextInput](../../examples/text-editor.mms) supplies a multiline editor, and
  [the table-state example](../../examples/table-field-reassign.mms) handles
  `TextInputChanged` with `event.text`.
- [TextInputSystem](../../src/engine/ecs/system/text_input_system.rs) emits the
  full edited text and caret. The [keyboard signal system](../../src/engine/ecs/system/keyboard_input_system.rs)
  suppresses gameplay key events while text input is focused, so a global
  `KeyDown` handler will not implement Ctrl+Enter inside the editor. Use a Run
  button first; add a focused input submit action before promising that shortcut.
- [Android's demo asset bundle](../src/demo_assets.rs) already installs the PNG
  font atlas, and the [custom UI material](../../assets/components/materials/ui_unlit.mms)
  can supply the generic custom-material path for the read-only preview. Avoid
  introducing legacy text materials, bloom/emission, or stencil clipping for
  this example. Validate changing text on the headset before calling it complete.

The Android example should compose an ordinary `HttpServer` with a queued MMS
evaluation facility. The missing evaluation facility must submit work after the
current callback returns; it is not a synchronous `eval()` inside that callback.
Keep these transport, draft-display, and evaluation responsibilities reusable
rather than hiding the whole workflow inside an Android-only shell implementation.

## First-class UDP surface

UDP uses one bidirectional socket, so start with one component rather than
separate client/server types. The following names and signatures are proposed:

| Surface | Contract |
| --- | --- |
| `UdpSocket.bind(address) {}` | Own one socket until component removal; port `0` asks the OS for a port. |
| `socket.send_text(remote_addr, text)` | Queue one UTF-8 datagram. |
| `socket.send_to(remote_addr, payload)` | Queue one binary datagram; `payload` is an array of byte values. |
| `UdpReady` | `{ local_addr }`, emitted after binding succeeds. |
| `UdpDatagram` | `{ remote_addr, payload, body_text, size_bytes }`; `body_text` is null for invalid UTF-8. |
| `UdpError` | `{ phase, message, remote_addr }`; phases include bind, receive, send, and size validation. |

Example headset echo service, using the proposed component:

```mms
let socket = UdpSocket.bind("0.0.0.0:7001") {}
socket

on(socket, "UdpDatagram", fn(packet) {
    socket.send_to(packet.remote_addr, packet.payload)
})
```

The PC binds `0.0.0.0:0`, waits for `UdpReady`, and sends to the headset's
LAN address. Replies return to `packet.remote_addr`, including the PC's actual
source port. `0.0.0.0` is a local bind address, not a destination address.

Implementation belongs in the portable engine, matching the HTTP flow:

```text
UdpSocket component -> registration/send intents -> UdpSystem
OS socket -> bounded incoming queue -> UdpDatagram -> MMS callback
```

Keep socket I/O and name resolution off the XR thread, reuse the socket and
receive storage, and bound queue size and events serviced per update. Do not
create a thread per datagram. Reject outgoing packets above the configured
limit and receive into enough storage to detect oversized packets before
discarding them; never deliver a truncated packet as valid data. A conservative
initial application payload limit is 1,200 bytes, without claiming that every
network path will avoid fragmentation at that size. No automatic retries,
ordering, or evaluation belongs in this component.

Start with unicast and numeric socket addresses. Broadcast, multicast, discovery,
and hostname resolution can be deliberate additions. Expose queue drops through
bounded counters/diagnostics, not an unlimited stream of error events.

## PC-authored remote MMS

The first endpoint would be `POST http://HEADSET_IP:7000/mms/eval`.
This PC-side fragment uses existing MMS HTTP and JSON APIs; its destination
endpoint does not exist yet. Integrate the result/error state with an example's
UI as desired. Addresses and token are placeholders.

```mms
let client = HttpClient {}
client
let state = { result = null, error = null }

on(client, "HttpResponse", fn(response) {
    state.result = JSON.parse(response.body_text)
})
on(client, "HttpError", fn(error) {
    state.error = error.message
})

client.post("http://192.168.8.50:7000/mms/eval", JSON.stringify({
    version = 1,
    session = "HOST_SCENE_GENERATION",
    id = "pc-run-001",
    token = "PAIRED_DEV_TOKEN",
    source = "query(\"#sakura_mobile_player\").translation()"
}))
```

The code in `source` runs on the Focus 3. The rest runs on the PC. A subsequent
command could create a named transform on the headset:

```mms
source = "T.position(0.0, 1.0, -2.0) { name = \"remote_probe\" }"
```

A later request can query that node. Remote snippets should retain bindings and
tables across requests too, e.g. `let adjustment = 0.25`, then
`adjustment = adjustment + 0.1`. This is incremental evaluation in the active
scene session, not loading a replacement scene on every request.

Use an inspection command first so the initial check does not depend on a new
rendering feature. A visible smoke test should update or attach geometry using
the headset scene's existing custom material definitions. Sending a component
with only legacy color/material conventions does not establish Android renderer
compatibility.

## Endpoint and result contract

An authenticated `GET /mms/info` handshake supplies protocol version, a fresh
scene/session generation, and advertised limits. Until MMS supports HTTP request
headers, a `POST /mms/info` variant can accept the token in a JSON body; the
current two-argument `HttpClient.post` cannot set an Authorization header.
Never place the token in the URL. A stale generation must be rejected, so a
retry cannot accidentally execute in a replacement scene or restarted app.

The evaluation request is the JSON envelope shown above. `id` is a caller-created
string unique within that scene generation. Successful HTTP delivery is separate
from successful evaluation. A completed response could be:

```json
{
  "version": 1,
  "session": "HOST_SCENE_GENERATION",
  "id": "pc-run-001",
  "ok": true,
  "value_text": "[0, 1, -2]",
  "error": null
}
```

The coordinates above are illustrative. `value_text` is formatted on the
headset; live ECS handles, closures, and mutable tables do not cross the network.
Return formatted values and evaluation errors first. Capturing console output
requires a separate session output sink; redirecting process-wide stdout would
mix unrelated engine logs into replies.

Authenticate before accepting work and bound body size, queued work, response
size, and retained results. While the host stays alive, suppress duplicate
`(session, id)` submissions: return the cached result, or pending status for a
command already running. Reusing an ID with different source returns a conflict.
Keep the retry cache bounded but retain a high-water mark or expired-ID record
so an evicted result cannot cause the same command to run again; a monotonic
per-client sequence is another option. Retries after restart are rejected by
the generation check. This is not a promise of exactly-once execution through
crashes.

An evaluation error can follow earlier successful effects in the same snippet;
do not promise rollback. A response marked complete means evaluation finished
and its immediate intents were processed. It does not mean the GPU displayed
the change or asynchronous asset loading finished. If the app is suspended,
reject or expire queued work with a clear unavailable status rather than holding
HTTP requests indefinitely. A client timeout does not cancel executing code.

## Headset integration

The reusable missing capability is an incremental evaluation facade such as
`Universe::evaluate_mms_snippet(...)`, returning a formatted result/error and
applying the resulting intents. It should use the retained RuntimeSpec session:

- [SceneRuntime](../src/scene_runtime.rs) already owns a `Universe`, loads MMS,
  and calls `Universe::update` from the XR frame path.
- [Universe](../../src/engine/universe.rs) retains `runtime_spec_session`, but
  its public `load_mms_source_at_path` starts a replacement session and resets
  settings. It is not the submission API.
- [RuntimeSpecSession](../../src/scripting/runner.rs) already has internal
  `evaluate_with_host`, preserving session state with a temporary `MittensHost`
  lease. It currently discards the evaluation value; expose an appropriate
  formatted result through the facade.
- [The current MMS REPL](../../src/scripting/repl/backend.rs) owns stdin and
  uses the legacy evaluator. Do not build a second remote evaluator around it.
  The [crate REPL migration](../../docs/task/mms-repl-navigation-and-cat-unification.md)
  is the path for later `ls`/`cd`/`cat` support. The first endpoint accepts MMS
  source, without promising those navigation commands.

The Android listener is an explicit development host facility. A socket worker
validates the transport envelope and enqueues an owned request; the engine
services it at a defined update boundary, lends the active session a host,
processes immediate intents, then sends a response. If authored HTTP/UDP
handlers later submit evaluations, queue them until the current callback has
returned; do not recursively enter a borrowed session.

There is currently no execution budget/cancellation facility in the crate
evaluator. Bounding queue drains does not bound an individual evaluation:
an infinite loop or expensive host operation can stall XR. The first trusted
developer prototype must state that limitation. Responsive arbitrary remote
code needs interpreter fuel/cooperative cancellation in `meow-meow-script` and
limits on expensive host calls, or a separately designed evaluator worker with
host calls serviced at update boundaries. A socket timeout cannot solve this.

Keep the listener lifetime at the app/Universe boundary so graphics recreation
does not create competing sockets. Invalidate pending jobs and the generation
when the scene closes; release sockets on shutdown. Define suspended behavior
explicitly rather than relying on an XR draw to service requests.

Several integration files are already oversized, including `universe.rs`,
`runner.rs`, the runtime/component registries, `host.rs`, and `system_world.rs`.
Extract the affected responsibility into focused modules before expanding it,
as required by `AGENTS.md`; do not add a large remote-service match arm to each.

## LAN and Android setup

Put the PC Ethernet port and headset Wi-Fi on reachable router networks. Check
guest/client isolation, router firewall rules, and the PC firewall if discovery
or replies fail. Use a DHCP reservation for the first version; discovery is not
required. Binding the headset server to `127.0.0.1` will not expose it to the PC.

The current [Android manifest metadata](../Cargo.toml) has OpenXR permissions
but no `android.permission.INTERNET`. Add that permission for native sockets;
Android documents it as an install-time normal permission in its
[network setup guide](https://developer.android.com/develop/connectivity/network-ops/connecting).
It currently targets SDK 32. Revisit
[local network permissions](https://developer.android.com/privacy-and-security/local-network-permission)
when changing Android/target SDK levels, since those restrictions also apply
to native TCP and UDP sockets.

Expose evaluation only when the development facility is explicitly enabled,
with a per-install pairing token or equivalent. A token on plaintext HTTP/UDP
does not protect traffic from LAN observers; this initial mode assumes a trusted
private development LAN. Use authenticated encryption for other networks.

## Implementation slices and evidence

1. Extract the retained-session driver and add incremental evaluation with
   structured completion. Verify shared bindings, errors followed by recovery,
   preserved scene callbacks/settings, and the stated partial-effect behavior.
2. Add the desktop `shell-client.mms` editor and Android `shell-server.mms` scene
   with bounded HTTP draft and execution endpoints. Verify preview changes before
   Run, rapid edits/coalescing, paste/deletion, exact submitted revision, actual
   headset execution, result correlation, duplicate requests, stale generations,
   and suspend/resume/shutdown behavior.
3. Add portable `UdpSocket` component/system/runtime bindings. Verify loopback
   echo, binary/UTF-8 behavior, port `0`, limits/drops, and component removal;
   then test PC-to-headset traffic through the travel router.
4. If UDP preview is useful after measuring the HTTP version, adapt the draft
   envelope with sequence checking, final-edit recovery, and packet size limits.
   Keep execution and larger drafts on HTTP rather than inventing a fragment/
   reassembly protocol.

No renderer changes are required by this proposal. Measure frame/update costs
with the listener idle and under bounded request load on Focus 3; PC loopback
tests cannot establish Wi-Fi reachability or sustained XR behavior.
