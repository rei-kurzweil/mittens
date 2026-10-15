# XR settings and floor correction

Implemented 2026-10-14. Headset visual acceptance is pending.

`XrSettingsSystem` owns local runtime calibration independently of panels and
avatars. Its MMS API works from scenes, imported factories, and callbacks:

```mms parse-only
XrSettings.floor.set_correction_meters(0.15)
let correction = XrSettings.floor.correction_meters()
XrSettings.floor.set_correction_meters(0.0)
```

The setter accepts a finite number from -0.50 to +0.50 metres. Invalid input
leaves the current setting unchanged. Readback immediately returns the accepted
value; `XrSettingsChanged` is emitted only for a changed value.

Positive correction lowers the tracked head, hands, and rendered eyes relative
to the simulated floor; negative correction raises them. For example, +0.15 m
subtracts 15 cm from raw STAGE Y before the rig's world mapping. The panel uses
0.5 cm steps and a signed centimetre readout. Reset sets the correction to zero.

## Panel and example

Run the calibration scene:

```sh
cargo run --release -- load examples/xr-settings.mms
```

It uses the same stage, boxes, teleport pit, Rei(mu) avatar, mirror, microphone,
and editor panels as `graphics-settings.mms`, replacing the graphics panel with
XR settings. Use the mirror and nearby floor as visual references while adjusting.
Its title bar supports the same drag/grab interaction as the graphics example.

Other examples can explicitly import and place the same factory:

```mms parse-only
import { xr_settings_panel } from "../assets/components/panels/xr_settings_panel.mms"

T.position(-1.28, 2.3, -1.0) {
    xr_settings_panel({ root_name = "xr_settings" })
}
```

The factory requires a unique `root_name`. Optional `title` accepts text or an
authored title-bar component, following the graphics panel. Its returned tree
can also be attached to a caller-owned mount. The panel requires no EditorUI.
Restored content reads backend state, and multiple panels silently synchronize.
Removing or minimizing a panel preserves calibration. Scene/session replacement
or closing the MMS session resets it to zero; persistent profiles are not included.

## InputXR and rendered poses

OpenXR frame preparation locates one sample before the engine's XR pose and
avatar phases. InputXR drives its first direct Transform child from that sample
with the floor correction included. CXR resolves its owning InputXR and renders
from that driven transform's final world matrix:

```text
head_world = base_rig_world * T(0, -correction, 0) * raw_head
eye_world  = head_world * inverse(raw_head) * raw_eye
hand_world = head_world * inverse(raw_head) * raw_hand
```

The raw head and eye poses come from the same prepared frame. Full relative eye
orientation/translation and OpenXR FOV are preserved. Rendering does not read a
second independent calibration value, and does not apply tracking twice. Avatar
camera wrappers remain anatomy/anchor authoring data rather than another XR
view offset. Legacy CXR without an owning InputXR retains its authored-origin
behavior; this calibration operates through InputXR.

The setting is latched during frame preparation. A later slider callback takes
effect on the next prepared frame, keeping eyes and tracked transforms on the
same applied value. Eye cameras are refreshed after locomotion/contact and
avatar/mount changes, before mirrors and drawing. Raw located poses and the
projection-view poses submitted to OpenXR retain their original reference space.
Disabled/removed drivers or invalid tracking do not supply stale rendered eyes.

The correction does not change the pre-tracking locomotion frame or AVC capsule
calibration. Gravity and floor contact continue to own simulated root placement;
physical crouching remains free. Parent scale maps the correction consistently
with tracking; existing capsule restrictions still govern supported avatar scale.
An avatar eye-to-sole/proportion mismatch requires separate AVC calibration and
posture work, even when the tracking floor is correct.

See [the implementation task](../task/xr-settings-system-and-mms-floor-correction-panel.md)
and [AVC standing calibration](../task/avc-vr-standing-height-and-sole-floor-calibration.md).
