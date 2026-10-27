# Component Runtime API

This document lists the component receiver API. Engine-specific methods below
describe the current live `ComponentObject` surface; universal reflection
describes the normative target for both `ComponentExpr` and
`ComponentObject`.

Scope:

- methods callable from MMS at runtime, for example `text.set_text("hi")`
- methods handled in `crates/meow-meow-script/src/evaluator.rs`
- methods handled in `src/scripting/component_method_registry.rs`

For the engine-specific method inventory, non-scope:

- component constructors such as `HttpServer.bind(...)`
- body-builder calls inside component expressions
- draft or planned APIs that are not implemented yet

## Current reflection gap

Live components support session-owned script fields: `component.field = value`,
`component.field` and calls to function-valued fields. See
[the per-instance field contract](../../task/mms-component-instance-methods.md).
This does not implement native authored-property reflection or expose component
block locals. Native methods/properties cannot be replaced by script fields.

Rust can inspect `MaterializedCE` labels and ordered children today. MMS cannot
yet uniformly inspect native types, enumerate component children or read native
authored properties. Script component fields and table function-field calls are
supported; the universal behavior below concerns the remaining reflection gap.
Functions already return ordinary component
expressions; they do not need a separate component runtime type.

## Universal component reflection

These operations apply to static, collected, attached, and live component
values:

### `component.type()`

- Returns: the authored or canonical component type as a string.

### `component.children()`

- Returns: a new ordered array of immediate component children only.
- Named fields, string positionals, and unrelated expression results are not
  children.

### `component.field`

- Returns: the authored named field value.
- A missing local/static field returns `null`.

Static `ComponentExpr` reads are served from the crate-owned artifact. Live
`ComponentObject` reads use generic host inspection: a confirmed missing field
returns `null`, unsupported inspection is a typed error, and foreign/stale
handles keep their distinct typed errors.

`type()` and `children()` are reserved only when called on a component
receiver. `component.type` may still read an authored field, and table fields
with either name remain legal.

## Source of truth

Today, the runtime-callable method surface is split across two codepaths:

- `crates/meow-meow-script/src/evaluator.rs`
- `src/scripting/component_method_registry.rs`

The evaluator handles the broader `ComponentObject.method(...)` surface. The component
method registry handles a smaller set of live component methods used by the runner path.

## Host-neutral component navigation

`ComponentObject` is MMS's host-neutral handle for a live component tree. A
host chooses which component types, selectors, signals, and domain-specific
methods it provides; it does not choose whether component values can navigate
their own subtree. Consequently, these methods apply to every live
`ComponentObject`, regardless of component type or script host:

### `component.query(selector)`

- Args: `selector: string`
- Returns: first matching live `ComponentObject`, or `null`
- Effect: asks the host to search within this component's subtree.

### `component.query_all(selector)`

- Args: `selector: string`
- Returns: array of matching live `ComponentObject`s
- Effect: asks the host to search within this component's subtree.

The evaluator dispatches these through the generic `HostRequest::Query`
protocol rather than through the host's component-method registry or
`RuntimeSpec` component-method table. A host that exposes live components must
therefore implement scoped query handling; a host with no component model need
not expose `ComponentObject`s in the first place.

## Transform

Receiver aliases:

- `T`
- `Transform`
- `TransformComponent`
- `transform`

### `set_position(x, y, z)`

- Args: three numbers
- Effect: emits `UpdateTransform`

### `update_transform(translation, rotation_euler, scale)`

- Args:
  - `translation: [x, y, z]`
  - `rotation_euler: [rx, ry, rz]`
  - `scale: [sx, sy, sz]`
- Effect: emits `UpdateTransform`

### `look_at(target_world)`

- Args: `target_world: [x, y, z]`
- Effect: emits `LookAt`

## Layout

Receiver aliases:

- `LayoutRoot`
- `LayoutComponent`
- `layout`

### `available_width()`

- Args: none
- Returns: current available width

### `available_height()`

- Args: none
- Returns: current available height

### `set_available_width(width)`

- Args: number or dimension
- Effect: emits `SetLayoutAvailableWidth`

### `set_available_height(height)`

- Args: number or dimension
- Effect: emits `SetLayoutAvailableHeight`

### `set_inspect(enabled)`

- Args: `enabled: bool`
- Effect: emits `SetLayoutInspect`

### `enable_inspect()`

- Args: none
- Effect: emits `SetLayoutInspect { enabled: true }`

### `disable_inspect()`

- Args: none
- Effect: emits `SetLayoutInspect { enabled: false }`

## Text

Receiver aliases:

- `Text`
- `TXT`
- `TextComponent`
- `text`

### `set_text(text)`

- Args: `text: string`
- Effect: emits `SetText`

### `set_font_size(font_size)`

- Args: `font_size: number`
- Effect: mutates the live text component font size, then emits `SetText` to refresh

## Emissive

Receiver aliases:

- `EM`
- `Emissive`
- `EmissiveComponent`
- `emissive`

### `set_intensity(intensity)`

- Args: `intensity: number`
- Effect: emits `SetEmissiveIntensity`

### `on()`

- Args: none
- Effect: emits `SetEmissiveIntensity { intensity: 1.0 }`

### `off()`

- Args: none
- Effect: emits `SetEmissiveIntensity { intensity: 0.0 }`

## Camera3D

Receiver aliases:

- `Camera3D`
- `Camera3DComponent`
- `camera3d`
- `C3D`

### `enabled()`

- Args: none
- Returns: current enabled state as `bool`

### `enabled(value)`

- Args: `value: bool`
- Effect: mutates the live camera enabled state

### `make_active_camera()`

- Args: none
- Effect: emits `MakeActiveCamera`

## CameraXR

Receiver aliases:

- `CameraXR`
- `CameraXRComponent`
- `camera_xr`
- `CXR`

### `enabled()`

- Args: none
- Returns: current enabled state as `bool`

### `enabled(value)`

- Args: `value: bool`
- Effect: mutates the live camera enabled state

### `make_active_camera()`

- Args: none
- Effect: emits `MakeActiveCamera`

## Signal Observer Router

Receiver aliases:

- `ObserverRouter`
- `SignalObserverRouterComponent`
- `signal_observer_router`

### `blacklist(names)`

- Args: `names: [string, ...]`
- Effect: replaces the router blacklist

### `whitelist(names)`

- Args: `names: [string, ...]`
- Effect: replaces the router whitelist

### `block(name)`

- Args: `name: string`
- Effect: adds `name` to the blacklist if missing

### `allow(name)`

- Args: `name: string`
- Effect: removes `name` from the blacklist

## AudioClip

Receiver aliases:

- `AudioClip`
- `AudioClipComponent`
- `audio_clip`

### `instance(start_beat?, stop_beat?)`

- Args:
  - optional `start_beat: number | null`
  - optional `stop_beat: number | null`
- Returns: a new detached `AudioClip` `ComponentObject`

## HTTP Client

Receiver aliases:

- `HttpClient`
- `HttpClientComponent`
- `http_client`

### `get(url)`

- Args: `url: string`
- Effect: emits `HttpClientRequest` with method `GET`

### `delete(url)`

- Args: `url: string`
- Effect: emits `HttpClientRequest` with method `DELETE`

### `post(url, body_text)`

- Args:
  - `url: string`
  - `body_text: string`
- Effect: emits `HttpClientRequest` with method `POST`

### `put(url, body_text)`

- Args:
  - `url: string`
  - `body_text: string`
- Effect: emits `HttpClientRequest` with method `PUT`

### Related events

An `HttpClient` can receive these events in `on(...)` handlers:

#### `HttpResponse`

Fields exposed to MMS:

- `request_id: number`
- `status: number`
- `ok: bool`
- `headers: map`
- `body_text: string`
- `url: string`

#### `HttpError`

Fields exposed to MMS:

- `request_id: number | null`
- `phase: string`
- `message: string`
- `url: string | null`
- `bind_addr: string | null`

## HTTP Server

Receiver aliases:

- `HttpServer`
- `HttpServerComponent`
- `http_server`

### `reply_text(request, status, body_text)`

- Args:
  - `request: HttpRequest event payload map`
  - `status: number` in `0..=65535`
  - `body_text: string`
- Effect: emits `HttpServerReply`

Notes:

- `request` must be the event object from an `HttpRequest` handler
- the implementation extracts `request.request_id`

### Related events

An `HttpServer` can receive these events in `on(...)` handlers:

#### `HttpRequest`

Fields exposed to MMS:

- `request_id: number`
- `method: string`
- `path: string`
- `query: string | null`
- `url: string`
- `target: string`
- `headers: map`
- `body_text: string`
- `remote_addr: string | null`

## Error behavior

If a method is not implemented for the receiver type, MMS currently errors with one of:

- `no method '...' on component type '...'`
- `unsupported live component method 'Type.method'`

That means this document should stay tightly aligned with the current code, not with
draft API ideas.

The universal reflection target additionally distinguishes unsupported
inspection, foreign handles, and stale handles as specified by
[Mittens host and MMS runtime boundary](mittens-host-and-runtime-boundary.md#component-values-and-reflection).
