# Draft: OpenXR `XR_KHR_vulkan_enable2` ownership and bootstrap

Date: 2026-06-25

Status: draft only. This is not a committed implementation direction.

This note records a likely architectural implication from the recent OpenXR comparison work:

- if the target runtime meaningfully requires `XR_KHR_vulkan_enable2`
- and if `vulkan_enable2` only works correctly when OpenXR creates the Vulkan instance/device
- then Cat Engine may eventually need an ownership/bootstrapping model where XR can supply the
  Vulkan graphics root instead of always consuming one that already exists

This is similar in shape to an existing engine pattern:

- if the audio system is active, it can become the effective clock source for animation/clock work
- otherwise the main thread supplies the clock

The possible Vulkan/XR analogue is:

- if OpenXR is active and the chosen runtime/setup requires XR-owned Vulkan creation, OpenXR may
  need to supply the Vulkan instance/device path to the renderer
- otherwise the engine can continue using its normal non-XR-owned Vulkan bring-up path

This note is only about the architectural shape of that possibility.

It is **not** a commitment to implement it yet.

---

## 1. Why this draft exists

The attempted `khr_vulkan_enable2` experiment was intentionally narrow:

- enable `XR_KHR_vulkan_enable2`
- disable legacy `XR_KHR_vulkan_enable`
- keep Cat Engine's current pre-created Vulkan instance/device model unchanged

That failed immediately because the current `openxr` crate path used by Cat Engine still routes
session creation through the legacy `KHR_vulkan_enable` loader.

So the experiment established:

- a real `vulkan_enable2` test is not just an extension-bit change
- it likely requires the actual OpenXR-owned Vulkan creation path

That means the next step, if ever pursued, is not "flip one boolean."
It is a renderer/bootstrap ownership problem.

---

## 2. The high-level architecture question

Today the engine model is roughly:

- the renderer stack creates Vulkan
- `OpenXRSystem` consumes raw Vulkan handles later to create an XR session

A real `vulkan_enable2` path may want the inverse:

- OpenXR runtime participates in creating Vulkan instance/device
- the renderer stack is then built on top of those XR-approved Vulkan objects

That is a meaningful inversion of ownership.

The main question becomes:

- who is allowed to own graphics bootstrap when XR is active?

This is why the issue looks more like the engine's "clock source ownership" pattern than like
an ordinary OpenXR action-binding tweak.

---

## 3. Why the clock-source analogy fits

The clock analogy is useful because the engine already tolerates a conditional provider model:

- one subsystem can become the authoritative provider when active
- otherwise a simpler default provider remains in charge

In the graphics/XR case, a future model might look like:

- desktop / non-XR path:
  - engine-owned Vulkan bootstrap
  - renderer owns graphics bring-up
- XR path with legacy-compatible runtime:
  - current model may remain sufficient
- XR path with runtime that effectively requires `vulkan_enable2` ownership:
  - XR bootstrap owns Vulkan creation
  - renderer attaches to XR-created Vulkan objects

That kind of split is plausible, but it should be treated as a major bootstrap design decision,
not a local `OpenXRSystem` refactor.

---

## 4. Why we are not committing to this yet

There are several reasons not to commit yet:

- the current evidence does not prove `vulkan_enable2` ownership is the real blocker
- the required code change surface is much larger than the controller-action experiments
- it would cut across renderer initialization, OpenXR bring-up, and probably swapchain ownership
- it should be planned together with non-XR startup behavior so the engine does not grow two
  incoherent graphics boot paths by accident

So this should remain a draft until the runtime/session investigation makes the need stronger.

---

## 5. What a future plan would need to answer

If this direction becomes necessary, a proper task/spec would need to answer at least:

- where Vulkan bootstrap authority lives when XR is enabled
- whether XR is allowed to create the Vulkan instance/device before the main renderer exists
- how the renderer consumes XR-created Vulkan handles
- how swapchain and image-format negotiation changes under that ownership model
- how desktop-only and XR-enabled startup paths stay coherent
- whether the engine wants a provider interface for graphics bootstrap similar in spirit to other
  conditional subsystem providers

---

## 6. Current conclusion

For now, the right conclusion is only:

- a real `XR_KHR_vulkan_enable2` experiment probably requires a larger bootstrap/ownership design

The right non-conclusion is:

- "therefore we should implement XR-owned Vulkan bootstrap now"

That is still premature.
