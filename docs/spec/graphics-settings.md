# Shared graphics settings and XR foveation preferences

The reusable panel is
[`graphics_settings_panel.mms`](../../assets/components/panels/graphics_settings_panel.mms).
Sakura drone conversion and other examples import the same factory. Its controls
use the `GraphicsSettings` built-in table; they do not own independent applied
settings.

## XR foveation

```mms parse-only
GraphicsSettings.foveation.quality("low")
let requested_quality = GraphicsSettings.foveation.quality_level()
```

Accepted values are `off`, `low`, `medium`, and `high`, describing **how much
fixed foveation** is requested. Low means a mild reduction toward the edges;
high means more foveation. The default is off. Invalid names are rejected without
changing the setting. Readback immediately returns the accepted preference.

The shared panel has a stepped quality slider and a grey, noninteractive eye
tracking control fixed to Off. Eye tracking has no setter or active toggle in
this slice. A visible note states that the quality preference is not applied
yet: **this UI/state slice does not enable native or Vulkan foveation**. The
optional native backend and supported/applied-state reporting are follow-up work.

Changing quality uses the existing `GraphicsSettingsChanged` event. All open
panels synchronize without emitting extra user-change events. Minimizing removes
the panel body but preserves the preference in `GraphicsSettingsSystem`; restoring
reads the latest value, including changes made by another panel or MMS callback.
Scene/session reset restores the default. Saved user profiles are separate work.

Bloom, mirrors, secondary motion, and foveation share the same built-in table,
engine-thread host service and notification path. Their MMS registration and
dispatch live in `src/scripting/graphics_settings_api.rs`; extracting that
responsibility avoids expanding the oversized host/runtime configuration files.

Headless regression coverage checks the built-in setter/getter, invalid levels,
two-panel synchronization, minimize/restore and disabled eye-tracking interaction.
