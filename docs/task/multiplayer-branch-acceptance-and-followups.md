# Multiplayer branch: acceptance and follow-ups

Status: working two-peer MMS prototype; hardware/network acceptance in progress.
This checklist records current scope, not authorization to implement every follow-up
on this branch. Upper-body/tracking expansion is documentation-only for now.

## Implemented checkpoint

- Binary typed telemetry channels authored in MMS, transform/morph receive bindings.
- Local and received Mitten rigs: root/head/hands, blink replication, local arm/leg
  IK, ambient gaze, local secondary motion, first-person desktop and XR-only modes.
- Calibrated desktop eye/sole height, shared eye origin, pool-tiled support floor,
  paper-craft twilight sky, bloom, and mirror scene workaround.
- PointerGrabbed/PointerReleased and MMS shared cube/gohei ownership/pose prototype.
- Automated scene checks: both avatar soles contact the floor; rendered desktop
  camera matches and follows the local eye pose; local/network ownership isolation.

The control session still uses the earlier JSON protocol. Rapid spatial/face
telemetry uses the binary typed path. Voice, weapon firing, and body tracking are
not silently supplied by the existing scene.

## Next acceptance checkpoint

- [ ] Same revision on both machines, desktop defaults committed, Rei's XR-only
      flags kept as an uncommitted local override. Restart each process after pulling.
- [ ] Confirm desktop actually renders from the local avatar's eyes and moves/looks
      with its player. Verify the XR camera offset stays correct during head motion.
- [ ] Verify both avatar copies' feet at rest and while walking/turning/crouching;
      check floor correction, different XR standing heights, and headset reacquisition.
      Headless desktop sole checks do not prove human/XR standing calibration.
- [ ] Check visible left/right blink closure and ambient gaze from both ends.
- [ ] Confirm mirror renders without backing; keep the reflected side clear.
      The renderer clipping bug is still open, not fixed by removing the panel.
- [ ] Grab/move/release cube and gohei from either end, including desktop camera-held
      placement, simultaneous grabs, handoff, and disconnect while held.
- [ ] Test host ball/kick authority and reconnect/peer cleanup over Tailscale.
- [ ] Observe latency/jitter and packet loss; record glitches before choosing an
      interpolation delay or expanding protocol complexity.

## Follow-ups after basic acceptance

1. **Remote motion quality and lifecycle.** Pose interpolation; explicit freshness
   and validity for head/body as well as hands; decide visual behavior on stale
   samples/disconnect rather than permanently displaying a frozen live avatar.
2. **MMS binding ergonomics.** Review reusable scene/controller factories and
   finer translation/rotation bindings after testing the existing imperative API.
3. **Shared interactions and weapons.** Move beyond latest-transition ownership
   toward host validation where required; add pistol/STA to shared object setup,
   constructor socket/weapon IDs, accepted-shot channels, exactly-once damage,
   release/throw physics, and late-join state. See
   [shared grabbing and firing](multiplayer-shared-grabbing.md).
4. **Desktop embodiment.** Replace scripted hand waves/camera levitation with
   deliberate untracked-hand and held-item pose behavior. Reuse
   [desktop held-item poses](desktop-held-item-avatar-poses.md) and
   [untracked pointer drivers](untracked-pointer-holding-pose-driver.md).
5. **STA calibration.** Resolve the launcher grip/model/muzzle origin displacement
   while retaining the machine pistol's good grip. See [STA task](sta-rocket-launcher.md).
6. **Audio phase 1.5.** Measure Opus latency/CPU/bitrate and packet sizes; binary
   encoded-byte payloads, media timestamps/sequence, bounded encoding work,
   receive jitter/loss handling and decode. Then phase 2 AudioStream / writable
   producer / receive binding, audio-source enum composition, and
   [VoiceChatSystem policy](voice-chat-system.md). No network voice yet.
7. **Protocol/platform scope.** Decide whether to migrate the remaining JSON
   control plane; do not confuse this with the binary telemetry already shipped.
   Multi-peer sessions, richer spawn/despawn, authority/security and additional
   hardware/runtime combinations remain explicit later work.

## Deferred design track

[Avatar posture and tracking inventory](avatar-posture-tracking-inventory.md)
records standing lean, coordinated crouch, all-fours, prone/supine transitions,
partial FBT ownership, OpenXR body/tracker capabilities, and webcam/MediaPipe
fusion. Do not implement those features as incidental multiplayer branch fixes.

Existing detailed references:
[typed-channel task](typed-udp-channels-and-mms-bindings.md),
[authoring/running the demo](../how_to/multiplayer.md),
[mirror clipping investigation](mirror-camera-projection-debug-followup.md).
