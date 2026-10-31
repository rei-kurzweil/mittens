# Mitten-droid shell UI

The headset example starts with dummy data: a dark terminal on the left and a
cream file browser on the right. Both windows have gold title bars, dark title
text, and gold side rails around inset content. The title finish is a small
custom shader with a highlight and sparse flecks. Terminal text uses the custom
unlit material, so it stays bright without lights. There is no bloom pass or
legacy emissive material. Both environments use a pink Sakura sky: the desktop
keeps the shader's sun, while the headset uses an unlit crescent authored with
`R.polygon`. The desktop has gold `-` / `+` buttons at the bottom right. They
move the camera outward/inward in 0.5 m steps, bounded to 2–12 m from the UI;
the controls travel with the camera so they remain reachable while zoomed in.

A floating ceramic terrace covers the spawn area and UI frontage. One raised
flower bed sits to the right and 0.8 m closer than the UI, with tiled edges,
soil, small blossoms, and the shared branching Sakura tree. A 0.62 m railing
surrounds the perimeter. Floor, planter, railings, branches, and blossoms use
retained combined meshes and explicit custom materials. The tree's mobile
variant samples/discards the flower mask in its fragment shader; it does not
select a legacy material or the unsupported TransparentCutout pipeline.

The file browser has a path bar, Up, twenty visible folded-page icons in five columns
and four rows, wrapped filenames, and row scroll buttons. Use a controller trigger to select dummy entries, or use Up to inspect the virtual `examples` directory. Dummy
source and terminal results are display data; they are never evaluated.

## Preview

From the repository root:

```sh
cargo run --example shell-server-preview
cargo run --example shell-client
```

The first command shows the actual headset MMS in a desktop window. The second
runs the PC shell, initially browsing `examples/mitten-droid`. Its Up button can
reach `examples`, which bounds this example's filesystem access. It displays all
regular files and folders, with MMS source previews. Symlinks are displayed but
not opened. Refresh rescans the current directory.

For the Focus 3 APK:

```sh
ANDROID_HOME=/home/rei/Android/Sdk android/packaging/build-apk.sh --features shell-demo
adb install --no-incremental -r target/debug/apk/mitten-droid.apk
adb shell am start -W -n org.mittens.engine.dev/android.app.NativeActivity
```

`shell-demo` selects this scene instead of the rooftop. No desktop connection is
needed to see the dummy data. The app owns OpenXR, and the scene uses its
external head/hand input. The windows are centered in eye-level `LOCAL` space,
without assuming a floor-height origin. The left and right Aim pointers can select items.

A GPU screenshot uses the same packaged scene and Android custom renderer:

```sh
cargo run --manifest-path android/validation/renderer-smoke/Cargo.toml -- \
  --shell-preview /tmp/mitten-droid-shell.ppm
```

This renders with 4x MSAA to a test image and writes the left eye. Readback is
confined to the validation harness; the headset renders into its XR images.

## Native grid, MMS shell

`assets/components/mitten_droid/ui.mms` owns both windows, their titles, colors,
status, and terminal composition. Rust in `src/scripting/file_browser/` owns
filesystem enumeration, listing validation, sorting, paging, the twenty cells,
icons, and their click handlers. Updating a listing reuses those cells and the
cached page/fold meshes. No MMS callback or component loop runs per file.

The widget APIs return an ordinary component that MMS can put in its tree:

| MMS API | Behavior |
| --- | --- |
| `FileBrowser.demo()` | Interactive dummy browser, without filesystem access. |
| `FileBrowser.local(root, relative)` | Native browser bounded by a canonical root. |
| `FileBrowser.remote()` | Initially empty browser populated from listings. |
| `FileBrowser.snapshot(browser)` | Path, revision, Up availability, opaque entry IDs/names/kinds. No PC absolute paths. |
| `FileBrowser.selection(browser)` | Selected filename and bounded source preview. |
| `FileBrowser.action(browser)` | Latest remote browser action, or null. |
| `FileBrowser.set_listing(browser, listing)` | Validate a remote listing; returns false for invalid data. |
| `FileBrowser.activate(browser, revision, kind, entry_id)` | Apply an `open` or `up` action if its revision is current. |
| `FileBrowser.refresh(browser)` | Rescan a local folder; report errors in the widget. |
| `FileBrowser.clear(browser)` | Show an empty offline browser. |

The widget emits `DataEvent` named `FileBrowserChanged`. Repeated entries are
native; MMS handles only the resulting browser or selection change. Listings
are limited to 4,096 entries. Long names wrap inside a fixed title area and are
ellipsized when they exceed three lines. The path bar shows the complete root-relative
path. Source previews show at most twelve lines, each at most 50 characters;
HTTP draft metadata is capped at 320 characters; UDP edits target a separate
normal TextInput buffer.

MMS already had `File.read_text`, JSON parse/stringify, HTTP client/server,
TextInput, and gameplay keyboard signals. This change adds `File.list_dir(path)`
for general MMS tools and `JSON.try_parse_object(text)` for callbacks that need
to reject malformed packets and still send a response. `list_dir` returns
sorted entries with `name`, `path`, `extension`, `is_dir`, `is_file`, and
`is_symlink`; it reports I/O and size errors as host errors. File and JSON
bindings now live in focused modules instead of extending the large host file.

## The implemented preview exchange

The headset example listens on port 7000. In the desktop window, enter
`HEADSET_IP:7000` and click Connect. Connecting replaces the dummy listing with
the PC's current directory. Disconnecting stops updates; after three seconds
the headset marks the browser offline and clears its entries. Source snapshots are previewed without evaluation. Terminal edits use UDP
independently of the HTTP preview exchange.

Both applications and their protocol helpers are MMS. Existing HTTP bindings
carry the first prototype; the desktop does not need an inbound listener. Every
300 ms, with at most one request in flight, the PC posts to `/mitten-droid/ui`:

```json
{
  "protocol": "mitten-droid", "version": 1, "kind": "ui.snapshot",
  "session": "desktop-1", "request_id": "7",
  "listing": {
    "path": "examples/mitten-droid", "revision": "2", "can_up": true,
    "entries": [{"id": "2:0", "name": "hello.mms", "kind": "mms"}]
  },
  "selected": "hello.mms", "source": "// source preview",
  "draft": "6 * 7", "action_ack": ""
}
```

The response repeats the session and request ID, with kind `ui.reply`, and
includes `action_id`, `action_kind`, `entry_id`, and the displayed `revision`.
An empty `action_id` means no action. The headset repeats its latest action
until acknowledged. The PC suppresses duplicate IDs and rejects stale listing
revisions. Directory changes rotate entry IDs. The two views scroll independently;
a headset folder navigation changes the shared current directory on the PC.
Rapid headset selections coalesce to the latest selection. This is a preview
protocol for one development client, not a durable execution queue or pairing
protocol. Requests are capped at one million characters in the examples.

## Pointer selection, scrolling, and UDP typing

Both Aim controllers have a visible gold ray and tip. The native browser owns
an actual single `SelectionComponent` and an `OptionComponent` on each pooled
cell. Built-in visual feedback is disabled; its selection events drive the
opaque beige tile style. The selected color keeps the same hue as the original
cream and changes lightness/saturation. Selection survives scrolling by entry
ID, while the visible Selection component is reconciled to the reused cell.
No selection downloads assets or executes code.

Drag upward over the grid, use either thumbstick, or use a desktop mouse wheel
to scroll by rows. The dummy catalog has 35 entries. Only twenty native cells
exist at any time; scrolling rebinds them without stencil clipping, intermediate
render targets, or rebuilding icon meshes.

The headset shell listens for UDP terminal edits on port 7001. In the PC shell,
enter the headset's numeric IP in the address field, then click the terminal
input and type. That focused input explicitly enables forwarding; editing the
address or interacting with other controls does not send terminal keystrokes.
The desktop launch adapter captures the existing normalized text/edit events,
including committed Unicode text, paste, Backspace, Delete, left/right caret
movement, and newline. It sends to the entered IP on UDP port 7001; an HTTP
connection is not required for typing. The desktop preview also listens on 7001.

`UdpTextInputProvider` implements the portable `HidProvider` boundary. The host
focuses the headset terminal when a received edit arrives, then the normal
TextInput system applies it. File selection can clear text focus without
breaking the next UDP edit. This slice forwards text/edit operations only; it
never holds gameplay keys, evaluates input, or adds parallel network key events.

The version-1 JSON wire envelope contains `version`, UUID `session`, increasing
`sequence`, `events`, and optional `ack`. Sequence zero with no events opens a
session. Each event is `{ "op": "insert", "text": "hello" }`, `backspace`,
`delete`, `left`, or `right`. Newline is inserted text. ACKs contain the applied
sequence and no events. The sender uses one packet in flight, 50 ms bounded
retries, up to 128 queued edit chunks, and at most 1,200 bytes per datagram.
UTF-8 paste is split on character boundaries. The receiver acknowledges the
committed prefix and suppresses duplicate/out-of-order application. Poll work
is bounded; pause/resume drains stale input and retires the old session. This
is a development terminal protocol, without authentication or encryption.

The Android renderer currently rejects skinning and morph meshes, so this
example uses pointers without a Mitten avatar. The Focus 3's graphics queue
supports compute, but no compute pipelines or dispatches are added here.

## Execution and keyboard protocol boundary

There is still no general MMS UDP socket or gameplay-key relay, and no queued
remote-evaluation host API. The examples deliberately expose that status.
The next kinds should remain distinct from `ui.snapshot`:

- `scene.launch`: an immutable project snapshot and fresh content scene.
- `mms.eval`: explicit source, execution ID, and target scene generation; evaluate
  after the current callback returns, then return a correlated result/error.
- `input.batch`: controller ID, scene generation, sequence, and normalized
  key/text events; acknowledge, deduplicate, and release held keys on timeout.

Keep reliable launch/evaluation on the existing HTTP path initially. Add UDP
for input through the reusable socket/relay boundary in
`android/docs/udp-lan-keyboard.md`, including its ordering and release behavior.
Execution retries need request deduplication and retained results. Source/draft
snapshots must never become an implicit execution request. Project transfer,
scene replacement, persistent terminal bindings, authentication, and remote HID
injection remain separate implementation work described in
`android/docs/mms-lan-development.md`.

## Validation

Focused browser/selection/scrolling, UDP text/edit, filesystem, and JSON tests
pass, including an
HTTP round trip that navigates from the headset to `examples` and back. Both
desktop launchers compile. The screenshot is rendered through the production
mobile renderer with custom pipelines and fixed native cells. The
`shell-demo` APK builds, installs, and initializes on Focus 3 with 1712 × 1712
XR images, 4x transient MSAA, and lazy transient depth/color attachments. The
initial headset placement was subsequently lowered by 1.55 m to center the
windows in the host's eye-level LOCAL reference space.

The broader existing renderer smoke command passes its triangle/custom-scene
checks, then stops in the rooftop scene on `mobile renderer requires a custom
Material(definition) on every surface` for generated editor-panel surfaces.
That separate stage path was left unchanged; the shell preview uses custom
materials throughout and renders successfully.
