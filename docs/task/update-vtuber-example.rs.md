# Task: Fix vtuber-example torso-twist bug
2
Fix the bug in `vtuber-example.rs` where the entire avatar body rotates with
the HMD. This occurs because the example manually parents the model to the
HMD transform, bypassing the `AvatarControlSystem` rotation-isolation logic.
6
## Objective
8
Transition `examples/vtuber-example.rs` to use `AvatarControlComponent` (AVC)
for proper head/body rotation decoupling.
1
## Proposed Changes
3
### `examples/vtuber-example.rs`
5
- Import `AvatarControlComponent`.
- Instantiate an `AvatarControlComponent` configured for VR:
    - `head_bone("J_Bip_C_Head")`
    - `camera_bone("J_Bip_C_Head")`
    - `initial_yaw(PI)`
- Re-wire the topology:
    - `xr_head` -> `avatar_control`
    - `avatar_control` -> `xr_camera`
    - `avatar_control` -> `model_root`

## Verification

- Run `cargo run --release --example vtuber-example`.
- Verify the body stays oriented forward (with threshold/lag) while the head rotates freely with the HMD.


