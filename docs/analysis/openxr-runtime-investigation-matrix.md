# OpenXR Runtime Investigation Matrix

Date: 2026-06-26

This note tracks the OpenXR controller-interaction-profile investigation across runtime
stacks, runtime branches, and comparison apps.

It exists because the current evidence is mixed:

- Cat Engine can create an OpenXR instance and Vulkan session
- Cat Engine can reach focused session state and read head/hand pose
- Cat Engine still reports active controller interaction profiles as `none`
- WayVR was confirmed to receive live controller input on this machine
- but the confirmed working WayVR path is currently OpenVR fallback, not a confirmed
  working OpenXR path

So the next investigation needs a matrix, not more memory-based assumptions.

Related context:

- [docs/task/openxr-runtime-session-comparison-with-wayvr.md](../task/openxr-runtime-session-comparison-with-wayvr.md)
- [docs/task/openxr-wayvr-style-controller-action-experiment.md](../task/openxr-wayvr-style-controller-action-experiment.md)
- [docs/task/openxr-controller-actions-and-default-stick-locomotion.md](../task/openxr-controller-actions-and-default-stick-locomotion.md)
- [src/engine/ecs/system/openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs)

---

## 1. What the OpenXR spec says

The OpenXR spec does **not** guarantee that `xrGetCurrentInteractionProfile(...)` will
always return a non-null profile just because controllers physically exist.

Important details from the spec:

- the runtime only returns interaction profiles that the application has provided
  suggested bindings for
- the runtime may return an anticipated profile even when no controllers are active
- `XR_NULL_PATH` does **not** by itself prove controller absence
- if the runtime cannot emulate any application-provided profile, it must return
  `XR_NULL_PATH`

That means Cat Engine seeing `none` can still be caused by:

- wrong or insufficient suggested bindings
- a runtime that does not emulate the profiles we suggested
- a runtime-specific controller routing issue
- an app/session/runtime-policy issue

It does **not** yet prove a pure engine-side logic bug.

---

## 2. Web-search-backed runtime notes

### A. Monado is a serious OpenXR comparison runtime on Linux

Current Monado docs say it provides:

- OpenXR-conformant runtime behavior
- action-based input
- full support for Local / Stage / Local Floor / Action space relations
- `XR_EXTX_overlay`

That makes Monado a useful comparison runtime when the goal is:

- compare controller-action behavior between runtimes
- compare interaction-profile activation policy between runtimes
- separate "our app shape is wrong" from "this runtime stack behaves differently"

### B. Monado does not look like an obvious Vive Focus 3 runtime path

Current Monado hardware docs list native/open-source support for devices like:

- Vive / Vive Pro / Valve Index
- Rift S
- Windows MR
- PSVR / PS Move

but this public support list does **not** list Vive Focus 3.

So "test with Monado instead" is a good idea for runtime comparison in general, but it is
not yet evidence that Monado is a practical Focus 3 runtime substitute in this setup.

### C. OpenComposite is probably not the main variable for Cat Engine's direct OpenXR path

Monado's docs explicitly describe setup flows involving Monado plus OpenComposite for
Valve Index scenarios. That strongly suggests the usual role of OpenComposite here is:

- compatibility for OpenVR applications on top of an OpenXR runtime

Cat Engine's blocked path is different:

- Cat Engine is already a direct OpenXR application

So OpenComposite may matter for:

- whether an OpenVR app works on top of a chosen runtime
- whether WayVR's OpenVR fallback works on a chosen runtime

but it is **probably not** the primary explanation for Cat Engine's direct OpenXR
`current_interaction_profile == none` result.

That is an inference from the role these stacks play, not a direct proof.

### D. SteamVR branch-specific evidence is still missing

A web search during this note did **not** find strong current public evidence tying this
specific symptom to:

- `steamvr-previous`
- SteamVR stable
- SteamVR beta

So the SteamVR branch question remains open and needs direct testing rather than assumption.

---

## 3. Current matrix

Legend:

- `yes` = observed working
- `no` = observed failing
- `?` = not yet tested or not yet confirmed
- `n/a` = not applicable to that row

| Runtime stack | Branch / mode | App path | Date last checked | Session reaches focused/usable | Head pose | Hand tracking / hand-root fallback | `current_interaction_profile` | Controller actions / buttons / sticks | Notes |
|---|---|---|---|---|---|---|---|---|---|
| SteamVR OpenXR | `steamvr-previous` | Cat Engine direct OpenXR | 2026-06-25 | yes | yes | yes | no (`none`) | no | Current main blocker. |
| SteamVR OpenXR | stable | Cat Engine direct OpenXR | ? | ? | ? | ? | ? | ? | Needs explicit retest. |
| SteamVR OpenXR | beta | Cat Engine direct OpenXR | ? | ? | ? | ? | ? | ? | Needs explicit retest. |
| SteamVR OpenXR | `steamvr-previous` | WayVR forced `--openxr` | 2026-06-25 | no | n/a | n/a | n/a | n/a | Failed with missing `EXTX_overlay`. |
| SteamVR OpenVR fallback | `steamvr-previous` | WayVR default / fallback path | 2026-06-25 | yes | ? | ? | n/a | yes | Confirms working controller path on this machine, but not via Cat Engine's OpenXR path. |
| Monado OpenXR | current docs/runtime | Cat Engine direct OpenXR | ? | ? | ? | ? | ? | ? | Useful runtime comparison, but Focus 3 hardware path is not yet established. |
| Monado OpenXR | current docs/runtime | OpenXR sample / interaction-profile demo | ? | ? | ? | ? | ? | ? | Good control case if hardware/runtime path can be made real. |
| Monado + OpenComposite | current docs/runtime | OpenVR comparison app | ? | ? | ? | ? | ? | ? | More relevant to OpenVR-on-OpenXR compatibility than Cat Engine's direct OpenXR path. |

---

## 4. Suggested next tests

Order these to maximize signal:

1. Retest Cat Engine direct OpenXR on SteamVR stable.
   Keep the same machine, headset, and app build if possible.

2. Retest Cat Engine direct OpenXR on SteamVR beta.
   This isolates runtime branch differences from engine changes.

3. Record exact runtime identity in logs for each run.
   Capture runtime name/version string and whether the profile ever changes away from
   `none` after session focus.

4. Run a very small direct OpenXR comparison app on the same runtime.
   Prefer a sample that:
   - is not overlay-dependent
   - prints `xrGetCurrentInteractionProfile`
   - creates basic controller actions

5. Only treat Monado as a Focus 3 comparison once the hardware path is real.
   If Focus 3 cannot actually run through Monado in this setup, Monado is still useful as a
   Linux OpenXR comparison runtime, but not a direct replacement test.

6. Keep OpenVR/OpenComposite comparisons separate from direct OpenXR comparisons.
   Those results are still valuable, but they answer a different question.

---

## 5. Outstanding questions

- Was Cat Engine's OpenXR interaction-profile test ever run against SteamVR stable on
  Linux on or after 2026-06-25?
- Was it ever run against SteamVR beta on Linux on or after 2026-06-25?
- Is the `Missing EXTX_overlay extension` failure specific to `steamvr-previous`, or does
  it also occur on stable and beta?
- Does a non-overlay direct OpenXR sample on the same SteamVR branch also report
  `current_interaction_profile == XR_NULL_PATH`?
- Can Vive Focus 3 be exercised through Monado in this environment at all?
- If Monado is not a practical Focus 3 path here, what is the best second direct OpenXR
  runtime/app comparison target on Linux?

---

## 6. Working hypothesis after this round

The current best hypothesis is:

- the blocked Cat Engine result is still most likely runtime-stack-specific or
  runtime-policy-specific, not yet proven to be a pure Cat Engine bug

More specifically:

- OpenComposite is probably not the main variable for Cat Engine's direct OpenXR path
- Monado is worth comparing as a Linux OpenXR runtime, but may not be a practical Focus 3
  replacement path
- the SteamVR branch question is still unresolved and needs direct branch-by-branch tests

That means the next most valuable data is not another local action-binding rewrite.
It is a runtime matrix with exact branch/runtime/app outcomes.

---

## Sources

- OpenXR 1.1 spec:
  https://registry.khronos.org/OpenXR/specs/1.1/html/xrspec.html
- Monado developer site:
  https://monado.freedesktop.org/
