# XR haptic actuators and tool recoil

Date: 2026-10-26
Status: proposed API and task sequence; **documentation only**.

The first use is a brief vibration in the holding Vive controller for each shot
of the pistol's burst. This output should also support UI clicks, contact and
other tools. It does not move tracked hands or provide physical force feedback.
Visual recoil, if wanted later, is a separate effect.

## Start from the MMS API

The proposed component is `HapticActuator.xr()`. Proposed usage, not currently
executable:

```mms
let recoil = HapticActuator.xr()
// Attach recoil under the existing controller node, identifying its hand.
recoil.pulse({ duration = 0.020 strength = 0.7 })
recoil.stop()
```

Duration is seconds, independent of the scene's beat clock. Strength is
normalized 0..1. Optional frequency could be in Hz, with omission letting the
runtime choose. Pulse/stop names and the parameter record remain proposals.

The user/input adapter owns the actuator reference. The tool decides when each
shot actually occurs and requests recoil then. The projectile has no haptic
responsibility. One possible extension to the current use context is:

```mms
pistol.use_start({
    user = avatar
    on_recoil = fn(pulse) { recoil.pulse(pulse) }
})
```

The tool would call this optional callback beside its actual flash/projectile
emission. Busy triggers and cancelled shots would produce no pulse. Desktop
users could omit it. Capture the chosen feedback endpoint for the accepted use;
a subsequent grab must not redirect an old use to another hand. This callback
is a design proposal, not part of the current pistol implementation.

## OpenXR end of the contract

Core OpenXR provides vibration output actions, `XrHapticVibration`,
`xrApplyHapticFeedback` and `xrStopHapticFeedback`. Parameters include duration,
amplitude and frequency. Runtimes may clamp duration/frequency;
`XR_FREQUENCY_UNSPECIFIED` requests the preferred frequency. See the
[OpenXR specification](https://registry.khronos.org/OpenXR/specs/1.1/html/xrspec.html#haptic-feedback).

The Vive profile exposes `/user/hand/left/output/haptic` and
`/user/hand/right/output/haptic`, shown in Khronos'
[interaction guide](https://www.khronos.org/developers/linkto/interaction-in-openxr).
Core vibration is sufficient for this slice; no advanced waveform extension is
needed.

The existing action creation, hand subaction paths and profile binding reports
live in `src/engine/ecs/system/openxr_system.rs`, particularly
`try_init_controller_input`. There is no vibration action or submission yet.
Optical hand tracking does not itself provide a motor: output availability must
follow the actual controller binding, independently of the avatar's solved pose.

## Task sequence: meet from both sides

1. **Review authoring first.** Settle construction, controller/hand resolution,
   pulse units and names, optional frequency, stop semantics and unavailable
   device behavior. Write controller, tool-adapter and desktop examples before
   shaping the backend around them.
2. **Trace the OpenXR lifecycle alongside the API design.** Check the Rust
   openxr crate's vibration action and apply/stop methods. Define a backend
   request with hand, duration, strength and optional frequency. Keep OpenXR
   handles out of MMS/tool assets and fit the existing/planned XR backend seam.
3. **Join the contracts at ECS intents.** Define component registration, hand
   ownership and typed pulse/stop requests. Resolve the existing controller
   ancestry, validate finite values/ranges and define inert output when XR or
   the controller is unavailable. Desktop scene loading must still work.
4. **Implement OpenXR output.** Create the vibration action with left/right
   subactions before attaching the action set. Add supported profile bindings,
   including Vive, through existing best-effort binding diagnostics. Submit
   apply/stop on the session-owning path. Handle inactive sessions, disconnects
   and session recreation without stale handles or replaying expired pulses.
5. **Bind MMS through RuntimeSpec.** Add the component, `.xr()`, chosen builders
   and pulse/stop methods through the current host/intents path. Do not add a
   legacy-only evaluator method. Check both authored and queried handles.
6. **Connect reusable tool feedback.** Supply the holding hand's callback or
   actuator at use start. Pulse for each actual burst shot, initially trying
   15–25 ms. Preserve firing locks and release cancellation. Haptics belong to
   visual/gameplay execution; animation audio lookahead must not duplicate them.
   Keep recoil requests out of the projectile and its hit callback.
7. **Test, then pause for hardware acceptance.** Mock correct hand routing,
   parameters, one request per emitted shot, no busy/cancelled pulses and no
   audio-phase duplicates. In VR test Vive motors, tune strength/duration,
   switch hands, drop mid-burst, and disconnect/restart the session. Headless
   Vulkan tests cannot verify a physical motor.

## Decisions before implementation

- Controller ancestry only, or an explicit `.hand("left")` override too? Missing
  or multiple controller parents must not silently select the wrong hand.
- Record versus positional pulse arguments; whether frequency belongs in v1.
- Whether scripts need an availability query. Missing output should not prevent
  an otherwise valid tool use.
- Overlapping requests to one physical hand. A first version can replace pulses
  rather than mix them, but define whose output `stop()` cancels. Several
  actuator components do not create independent physical motor channels.
- Removal/session cleanup: explicitly finish or stop an already-issued pulse;
  never leave sustained vibration running after its owner disappears.

Advanced envelopes, streaming, effect mixing, additional backends and visual or
physics recoil remain later work. Review the MMS example and backend request
shape together before committing to these choices.
