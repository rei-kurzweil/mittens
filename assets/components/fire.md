# Fire controller

`fire.mms` exports `fire(options)`. A fire owns a burning material and a bounded
smoke pool. Its airflow controls how newly emitted rounded cubes begin their
lives, while each cube independently grows and fades. This adds no engine
features or renderer passes.

```mms
import { fire } from "assets/components/fire.mms"

let ember = fire({
    name = "cigarette"
    airflow = 0.2
    burn_rate = 0.025
    smoke_options = {
        name = "cigarette_smoke"
        capacity = 16
        size = 0.025
        lifetime = 2.0
        growth = 3.0
    }
})
ember.root
T.position(0.0, 1.0, -1.0) {
    R.cylinder(0.012, 0.20, 16) {
        ember.material
        C.rgba(0.88, 0.85, 0.72, 1.0)
    }
    ember.source
}
ember.ignite()
on_global("FrameTick", fn(event) { ember.update(event.dt_sec) })
```

Resolve imports relative to the importing scene. For the real cigarette, attach
`material` to its GLTF material scope and `source` underneath its ember-tip
transform. Keep `root` in the world, separate from the moving object. Match the
burning material's UV axis/interval to that asset; moving its tip with the burn
front remains caller-owned because UV coordinates do not identify a world axis.
The preview in `assets/scenes/smoke-materials.mms` demonstrates this linkage.

## Controls and lifetime

| Member | Behavior |
| --- | --- |
| `root`, `source`, `material` | Smoke geometry root, emitter transform, and burning appearance. |
| `smoke` | The contained smoke controller, including its material, bursts and stats. |
| `ignite()` | Start burn progression and continuous smoke. Returns false if already lit, disposed or fully consumed. |
| `set_airflow(amount)` | Set normalized airflow in 0–1. Existing smoke keeps its birth settings. |
| `extinguish()` | Pause burn progression and stop new smoke; existing cubes continue growing/fading through `update`. |
| `set_burn_progress(amount)` | Seek normalized burn progress in 0–1. Reaching 1 extinguishes the source. Seeking backward does not automatically reignite it. |
| `update(dt_sec)` | Advance combustion, ember animation and smoke using seconds. |
| `dispose()` | Dispose the owned smoke pool; later calls are inert. |
| `stats()` | Snapshot of lit/disposed state, airflow, burn progress and smoke pool stats. |

The factory starts unlit. The caller supplies one frame handler for its retained
effects. Fuel exhaustion stops births at the exhaustion time within an update;
the remaining time ages already emitted cubes. A large frame therefore does not
emit smoke for time after the fire was consumed. Reuse the component by seeking
progress back to zero and calling `ignite()`.

## Parameters and response

| Option | Default | Meaning |
| --- | --- | --- |
| `name` | `"fire"` | Prefix for the default smoke pool when `smoke_options` is omitted. Set `smoke_options.name` when supplying smoke options. |
| `airflow` | `0.35` | Normalized air supply in 0–1. |
| `burn_rate` | `0.025` | Normalized fuel consumed per second before the airflow multiplier. Zero keeps the burn front fixed while emitting smoke. |
| `burn_progress` | `0` | Initial fuel consumption, with 1 fully ash. |
| `material` | New burning material | Optional caller-supplied burning material instance. |
| `smoke_options` | Default smoke options | Options forwarded to `smoke(options)`, including its own smoke material, source, pool size, motion and fades. |
| `smoke_size_low` | Base smoke size × 0.7 | Cube width at zero airflow. |
| `smoke_size_high` | Base smoke size × 1.6 | Cube width at maximum airflow; clamped to at least the low size. |

Airflow linearly interpolates the starting cube width between the low/high
sizes. It also scales upward velocity by `0.7 + 0.9 × airflow`, velocity spread
by `0.5 + airflow`, and birth frequency by `0.65 + 0.7 × airflow`. Horizontal
authored wind is preserved. Burn speed scales by `0.5 + 1.5 × airflow`.

These are small authored response curves for the cartoon effect. They do not
solve airflow, oxygen transport or combustion. Higher airflow is deliberately
mapped to larger new cubes; the two size endpoints can be tuned per source.
Smoke's seeded size variation is applied after selecting that starting size.

Smoke keeps its short fade-in and long fade-out, and its growth is independent
of later airflow changes. Live cubes never snap to a new starting size or
velocity. The pool remains fixed in size and drops births when full. Ignition
and airflow edits reuse the existing mesh, materials and pipelines.

This controller drives an ember/char/ash material and smoke. It does not add
visible flame geometry, consume the mesh, or request light/bloom effects. GPU
timing and stereo appearance still require hardware acceptance.
