# MMS development over the Focus 3 LAN

Status: workflow design, revised 2026-10-31. The
[runnable MMS shell prototype](../../docs/how_to/mitten-droid-shell.md) implements
the paired UI, native browser selection/scrolling, directory listing exchange,
and UDP terminal text/edit input. Scene launch, project transfer, gameplay key
relay, and incremental Universe/evaluation APIs below remain proposed.
The workflow and lifetime requirements below define the intended scope; message
fields, API names, and transport mechanics remain proposed implementation details.

The intended workflow is:

```text
Mittens desktop client on Arch PC
  -> Ethernet -> travel router -> Wi-Fi 6
  -> Android Mittens host on Focus 3
  -> launch scripts, evaluate terminal commands, or deliver keyboard input
  -> return status, results, errors, and headset browser actions to the PC
```

Once a host APK with this facility is installed, launching Mittens Host on the
headset starts the LAN listener by default in the Android development build.
The PC connects to its address; no headset menu or per-launch listener toggle
is required. ADB is independent of this connection. MMS and transferred asset
changes do not require reinstalling the APK; native engine changes and recovery
from a crashed host still need a separate mechanism.

## Desktop client and headset workflow

The desktop client runs inside Mittens using its desktop runtime. It provides:

- A connection screen with headset IP address, port, Connect, and remembered
  addresses. After connecting, show connection state, running script, and
  Disconnect. Start with a manually entered address; discovery is deferred.
- A file browser rooted at a local project folder selected on the PC. Navigate
  folders, search script entries, and double-click an MMS script to launch it on
  the headset, with the familiar interaction of a desktop file explorer.
- A terminal attached to the headset's active MMS session. Edit commands on the
  PC, explicitly submit them, and display correlated results and errors. Script
  output belongs in this terminal once a session output sink is implemented.
- Restart and Stop actions for the active script, plus a separate mode for
  forwarding keyboard input to focused headset UI or scene handlers.

After connection, use a file browser beside the terminal, with headset and
running-script status above them:

```text
Headset: 192.168.8.50  Connected       Running: sakura-stage.mms
+----------------------+-----------------------------------------+
| Project files        | Headset terminal                        |
| examples/            | > let speed = 0.5                       |
|   sakura-stage.mms   | > speed * 2                             |
|   cubes.mms          | 1                                       |
|   text-input.mms     | >                                       |
+----------------------+-----------------------------------------+
                         Restart / Stop / Disconnect
```

The PC also sends project directory listings to the headset. The headset
displays an interactive file browser backed by the same listing and launch
operations as the desktop browser. The user can open folders and select a
script while wearing the headset. These actions travel back to the PC, which
resolves the selection, transfers the required files, and requests the launch.
The headset can initially show a waiting scene; its project browser becomes
available when the PC supplies a project. Disconnecting leaves an explicit
offline state rather than making cached entries appear launchable.

### Launching scripts and evaluating terminal commands

These are distinct operations with different lifetimes:

| Operation | Behavior |
| --- | --- |
| Launch | Send an immutable script/project snapshot and start a fresh scene and MMS session. End the previous script's callbacks and release its scene-owned systems/resources. |
| Restart | Start the active launch snapshot again in a fresh scene/session. Launch again from the browser to use changed PC files. |
| Stop | Close the active script and return to the host's waiting/browser state. |
| Terminal submission | Evaluate the submitted source in the active scene's retained session; preserve bindings, functions, tables, and scene references between submissions. |
| Remote input | Deliver ordered keyboard/text events through the normal engine input path. It does not submit terminal source. |
| Draft preview | Optionally mirror unsubmitted editor text on the headset without evaluating it. |

Launching a script must not accumulate several examples in one scene. The host
listener, connection, and browser belong to the application and survive Launch,
Restart, and Stop. The script scene has separate ownership: merely replacing
the retained evaluator does not establish that the old ECS scene, audio,
network components, and callback routes were removed. A fresh content Universe
or equivalent complete teardown needs an explicit implementation contract.

Terminal submissions target the active scene generation. Launch, Restart, and
Stop invalidate the previous generation and any pending commands/input for it.
The terminal preserves PC-side history but marks the session change; old
responses cannot become results for the new scene. If no script is running,
return a clear no-active-scene response. The host/browser remains available
after a load failure; an atomic rollback to the previous scene is not promised.
Stop and Restart run at engine boundaries; they cannot interrupt a stalled
evaluation with the current interpreter's cancellation limitations below.

### Directory listings and project transfer

The PC owns project filesystem access. A listing contains project identity, a
project/browser generation, request ID, directory ID, listing revision, and
entries with opaque IDs, display names, and file/folder kinds. Include a parent
directory ID for navigation within the selected project. MMS file entries can
be launched; ordinary assets remain browseable. File type alone does not
establish compatibility with the Android renderer.

Select a project root containing the script's dependencies: for repository
examples importing `../assets`, use the repository root rather than just its
`examples` folder. Relative parent paths are valid when they stay within that
project root.

Headset navigation and launch actions carry the displayed project generation,
listing revision, and entry ID. The PC rejects obsolete selections and sends a
fresh listing if the folder changed or the entry disappeared. Opening a new
project or reconnecting rotates browser identity so delayed listings and actions
cannot apply to the wrong project. Scene replacement does not invalidate the
project browser; project and scene generations have different lifetimes.

Both views use the same data model and operations; they can keep independent
current folders. Bound listing size and paginate large directories. Bound and
correlate navigation requests so late responses cannot overwrite a more recent
folder selection. Refresh/resynchronize on reconnect. Resolve IDs on the PC
within the selected project root, including symlink handling; a headset action
does not grant arbitrary access to PC paths. A portable directory-listing host
API or a narrow PC host adapter is needed; the proposed MMS file browser is not
already implemented by the existing `File.read_text` API.

Launch transfers an entry script together with its required imports and assets.
Use a versioned project manifest, relative paths, file sizes, and content
digests, then send missing content into an app-private headset cache. Preserve
project-relative layout for imports and asset references. Check transfer limits,
digests, and path containment before activating the snapshot; never resolve a
PC absolute path as a headset file. Retain the launch snapshot for Restart and
bound cache storage, retaining files referenced by the active snapshot.

Dependency collection must cover imported MMS, shader files, textures, fonts,
models, and other referenced files. Dynamically constructed paths need explicit
manifest entries or a project include list; static import scanning alone cannot
promise a complete bundle. An early packaged-assets prototype may be useful,
but general file-browser launch is complete only when a script with transferred
imports/assets runs without those files having been packaged into the APK.

### Remote keyboard and text input

Keep terminal editing, draft mirroring, and remote input separate. Typing in the
terminal changes its PC buffer until explicit submission. A visible forwarding
mode sends committed text and key presses/releases, including modifiers,
Backspace, arrows, and Enter, to the headset's normal keyboard/TextInput path.
Text commits support paste and input methods; define the event format so a key
and its committed text cannot insert the same character twice.

Use bounded, ordered input batches with a controller identity, scene generation,
and increasing sequence. Suppress duplicate batches, detect gaps, and release
held keys when ordering cannot be recovered. Release held state on disconnect,
controller timeout, forwarding-mode exit, suspend, and scene replacement; a
reconnection starts neutral. Reject or expire input while suspended instead of
replaying it on resume. Desktop browser/terminal shortcuts must not also be
forwarded unintentionally.

## Separate the socket from remote evaluation

Keep transport separate from browsing, input, scene launch, and evaluation. The
[UDP and LAN keyboard proposal](udp-lan-keyboard.md) specifies a portable
`UdpSocket` component alongside `HttpClient` and `HttpServer`, plus a
`KeyboardRelay` component and reusable Rust/MMS examples. Build those examples
early to establish real LAN typing before completing the dashboard. Receiving
a general datagram produces an MMS event; interpreting it as code is an explicit
application decision.

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

Use HTTP for listings, launch/control, terminal submissions, and relay setup.
Use the proposed UDP keyboard relay for remote input once its ordering,
acknowledgments, bounded retries, and release behavior are implemented. HTTP
input remains useful as an initial comparison; raw UDP alone does not satisfy
the input contract. Headset browser actions need a return path even though the
PC initiates the connection: let the PC poll a bounded host action queue and
post listing updates or launch requests back. Actions have IDs and remain
available until acknowledged or explicitly expired; duplicate delivery must not
launch a script twice. Acknowledge after accepting the action, and correlate
any later transfer/launch failure to it. Expire actions on disconnect or
project-generation change. Do not open a second inbound server on the PC as an
implicit requirement. A persistent bidirectional transport can replace this
polling if measured interaction latency or traffic warrants it.

## Example composition and optional live draft preview

The proposed example names follow the network roles:

| File | Responsibility |
| --- | --- |
| `examples/shell-client.mms` | Desktop Mittens UI: connection screen, local project browser, headset terminal, launch controls, and remote-input mode. A host adapter/launcher supplies filesystem services as needed. |
| `android/examples/shell-server.mms` | Headset waiting/browser UI, listing display and selection actions, status, and optional draft preview. The app-owned listener and scene management survive replacement of a launched script. |

The initial UI examples now exist, with a native browser and UDP text/edit
input. They do not yet implement scene launch, transfer, or remote evaluation.
The full workflow below remains the intended contract. A
mirrored terminal/editor draft can separately be read-only, with the PC owning
that editing buffer. Draft mirroring is an optional extension; the browser and
terminal workflow does not depend on it. Preserve the desktop draft while a
previous submission executes, so the user can keep typing.

Treat preview and execution as separate operations:

```text
desktop TextInputChanged -> latest draft snapshot -> headset Text.set_text
desktop Run              -> exact source snapshot -> MMS evaluation -> result
```

For draft mirroring, send the edited text as a snapshot. This handles insertion,
deletion, paste, and committed input-method text consistently, and a later
snapshot repairs a missed update. Remote input to headset UI remains the
separate event stream described above. The draft message can include the caret
too. `TextInputChanged` currently includes text and caret after an edit;
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

The examples should consume reusable transport, project-browser, input, and
queued execution facilities. An ordinary `HttpServer` can serve authored routes,
but the default Android listener must outlive launched script scenes. The
missing evaluation facility must submit work after the current callback returns;
it is not a synchronous `eval()` inside that callback. Keep these responsibilities
reusable rather than hiding the whole workflow inside an Android-only shell
implementation.

## Portable UDP and keyboard relay

The [focused proposal](udp-lan-keyboard.md) is the source of truth for UDP
component configuration, Rust intents/events, MMS bindings, socket lifecycle,
and keyboard relay messages. It separates generic datagrams from reliable
ordered keyboard/text operations and defines capture, focus ownership, ACKs,
timeout releases, queue/traffic limits, and the remote HID provider boundary.

Its first acceptance slices build Rust and MMS UDP echo examples, then desktop
keyboard sender/headset receiver examples. The generic socket has no automatic
retry or evaluation behavior; the relay supplies the keyboard-specific protocol.
Live draft snapshots remain an optional use of UDP after the typing path works.

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

An authenticated `GET /mms/info` handshake supplies protocol version, host
identity/generation, active scene/session generation (or no active scene),
running-script and suspended state, and advertised limits. Until MMS supports
HTTP request headers, a `POST /mms/info` variant can accept the token in a JSON
body; the current two-argument `HttpClient.post` cannot set an Authorization
header. Never place the token in the URL. A stale generation must be rejected,
so a retry cannot accidentally execute in a replacement scene or restarted app.

Host/controller identity, project/browser identity, and scene identity are
separate. Bind the selected project and its browser actions to the authenticated
PC controller. The host protocol also needs operations for publishing listings,
polling/acknowledging browser actions, transferring manifests/files, Launch,
Restart, Stop, and input batches. Their exact route names are not settled here.
Scene-changing operations include the expected current scene state and return
the new generation. Deduplicate launch/control requests in the host/controller
scope, retaining their completion across scene changes: retrying a successful
Launch must not restart the script merely because Launch changed the scene
generation. Reject a competing operation based on obsolete scene state.

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

The Android listener is a development host facility, enabled by default in the
Android development build. A socket worker validates the transport envelope and
enqueues an owned request; the engine services it at a defined update boundary,
lends the active session a host, processes immediate intents, then sends a
response. If authored HTTP/UDP handlers later submit evaluations, queue them
until the current callback has returned; do not recursively enter a borrowed
session. Keep directory scanning, file transfer, and hashing off the XR thread;
queue only validated launch metadata and bounded input/evaluation work for it.

There is currently no execution budget/cancellation facility in the crate
evaluator. Bounding queue drains does not bound an individual evaluation:
an infinite loop or expensive host operation can stall XR. The first trusted
developer prototype must state that limitation. Responsive arbitrary remote
code needs interpreter fuel/cooperative cancellation in `meow-meow-script` and
limits on expensive host calls, or a separately designed evaluator worker with
host calls serviced at update boundaries. A socket timeout cannot solve this.

Keep the listener and project browser at the application boundary so graphics
recreation or script scene replacement does not create competing sockets or
disconnect the PC. Closing a script invalidates its pending jobs, held input,
and scene generation while preserving host/controller and project state.
App shutdown releases sockets and invalidates all generations. Reject or expire
scene work while suspended, report that state to the PC, and resynchronize
browser and neutral input state on resume. Do not rely on an XR draw to service
network status or expire requests.

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

Enable the listener by default in the Android development build, with a
per-install pairing token or equivalent and a way to disable it in build/config.
Selecting that development build enables the facility; it must not require a
headset menu action each launch. Production distribution policy is separate.
A token on plaintext HTTP/UDP does not protect traffic from LAN observers; this
initial mode assumes a trusted private development LAN. Use authenticated
encryption for other networks.

## Implementation slices and evidence

1. Extract the retained-session driver and add incremental evaluation with
   structured completion. Verify shared bindings, errors followed by recovery,
   preserved scene callbacks/settings, and the stated partial-effect behavior.
2. Add the default Android development listener and desktop connection/terminal
   UI. Verify connection by IP, authentication, actual headset evaluation,
   correlated results, duplicate requests, stale generations, reconnect, and
   suspend/resume/shutdown. Add a session output sink for script output. Build
   the [portable UDP and keyboard relay examples](udp-lan-keyboard.md) alongside
   this slice. Verify remote text, paste, editing keys, modifiers, ordering,
   duplicate/gap handling, and held-key release through the normal input path.
3. Add the desktop file browser, selected project root, and reusable listing
   model/host adapter. Verify folders, script search, double-click selection,
   bounded/paged listings, changed/deleted entries, and root/path containment.
4. Implement manifest/cache transfer and Launch/Restart/Stop with explicit
   script scene ownership. Launch a script with imports and assets absent from
   the APK; verify missing/corrupt files, immutable launch snapshots, restart,
   callback/resource cleanup, settings reset, duplicate launch suppression, and
   failure recovery while the listener/browser remains available. Confirm each
   double-click starts one fresh scene and terminal commands target that scene.
5. Add the headset `shell-server.mms` browser panel using PC-supplied listings,
   with navigation/launch actions returned through the host queue. Verify folder
   selection and script launch while wearing the headset, out-of-order listing
   responses, action retries/expiry, stale entries, separate browser/scene
   generations, offline display, and reconnect resynchronization. Verify actual
   headset selection/input and readable changing text with custom UI materials.
6. Optionally add live draft preview, checking rapid edits/coalescing, paste and
   deletion, exact submitted source/revision, and independent completion display.
   If measurements justify using the UDP API for preview too, add draft revision
   checks and final-edit recovery. Keep execution and larger drafts on HTTP.

No new rendering features are part of this proposal. Build the browser/terminal
UI with the supported generic custom-material path; do not introduce legacy
pipelines or assume desktop examples are all Android-compatible. Retain direct
XR rendering and the current attachment lifetime/load/store policy without
extra intermediate targets for the launcher. Validate UI and controller
interaction on Focus 3. Measure frame/update costs with the listener idle and
under bounded request and transfer load; PC loopback tests cannot establish
Wi-Fi reachability or sustained XR behavior.
