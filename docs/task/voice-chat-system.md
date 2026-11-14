# VoiceChatSystem: transmission policy

Status: rough follow-up, deferred until local channel/avatar testing and the
[compressed-audio milestone](typed-udp-channels-and-mms-bindings.md#intermediate-milestone-after-local-avatar-testing-compressed-audio).
No voice-chat implementation is part of the current slice.

- Add VoiceChatSystem to own when a local audio source should transmit: push to
  talk, loudness/RMS threshold activation, and explicit mute/enable controls.
- Expose policy and controls in MMS. Define mode switching, threshold tuning,
  hysteresis and a short release/hangover interval so speech does not chatter
  on/off or lose word endings. Consider bounded pre-roll to preserve word starts.
- Route permitted audio to the selected socket/channel through the encoding
  pipeline. Keep policy separate from raw socket I/O, codec mechanics and the
  generic AudioStream playback source. Opus is the leading codec candidate.
- Decide where gating happens relative to encoding, and distinguish deliberate
  silence/mute from packet loss. Keep audio timing and stream lifecycle explicit.
- Use bounded queues and worker-owned encoding/network work; audio callbacks
  must not block, and transmission must continue through render-frame hitches.
- Validate push-to-talk transitions, threshold/noise behavior, mute overriding
  other modes, stream teardown, latency and actual transmitted packet counts.

Related design: [typed channels, AudioStream and voice chat](../draft/typed-telemetry-mms-bindings.md).
