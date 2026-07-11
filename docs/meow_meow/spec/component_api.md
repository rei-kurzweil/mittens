# Component Runtime API

This document lists the **current MMS runtime component methods** that can be called on
live `ComponentObject` values.

Scope:

- methods callable from MMS at runtime, for example `text.set_text("hi")`
- methods handled in `src/meow_meow/evaluator.rs`
- methods handled in `src/meow_meow/component_method_registry.rs`

Non-scope:

- component constructors such as `HttpServer.bind(...)`
- body-builder calls inside component expressions
- draft or planned APIs that are not implemented yet

## Source of truth

Today, the runtime-callable method surface is split across two codepaths:

- `src/meow_meow/evaluator.rs`
- `src/meow_meow/component_method_registry.rs`

The evaluator handles the broader `ComponentObject.method(...)` surface. The component
method registry handles a smaller set of live component methods used by the runner path.

## Generic query methods

These methods apply to any live `ComponentObject`:

### `component.query(selector)`

- Args: `selector: string`
- Returns: first matching live `ComponentObject`, or `null`

### `component.query_all(selector)`

- Args: `selector: string`
- Returns: array of matching live `ComponentObject`s

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
