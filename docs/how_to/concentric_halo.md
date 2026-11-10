# Concentric halo

Run `cargo run --release -- load examples/halo.mms` for a moving head stand with a
striped backdrop to inspect motion and transparency. The paper-craft XR Mitten also
receives the halo on its imported `J_Bip_C_Head` transform after GLTF initialization.

```javascript
import { concentric_halo } from "assets/components/accessories/concentric_halo.mms"
head.attach(concentric_halo("my_halo", {
    opacity=0.70
    emission_gain=1.8
    rim_strength=1.4
    rim_power=2.5
    ring_gap=0.10
    head_clearance=0.26
    band_width=0.034
    motion_enabled=true
    position=[0,0,-0.10]
    rotation=[-0.7853981634,0,0]
    response_scale=0.60
    drag_force=0.55
    allow_stretch=true
    outer_opacity=0.35
    gap_angle=1.0471975512
    insert_angle=0.4188790205
}))
```

The two rings are red and pastel magenta. The root transform offsets them behind
the head and tilts the chain 45 degrees between back (-Z) and up (+Y). `position`
and Euler `rotation` (radians) change that basis; an additional parent `T` works too.
Four torus profile sides
produce a rectangular band section, flattened vertically into an extruded annulus.
Each main band is a capped 300-degree torus arc. A 24-degree segment of the same
profile sits in its 60-degree gap, leaving 18 degrees open at each end. The inner
insert is cyan and the outer insert is pink; both follow their ring's joint.
`R.partial_torus(radius,tube_radius,start,sweep,segments,sides)` exposes the underlying
primitive with angles in radians and a positive sweep up to one full revolution.
Nested rigid joint transforms accumulate lag toward the outer ring. The spring solver uses
captured transform rest poses without generating vertex weights or skinning resources.

`response_scale` multiplies spring stiffness; smaller values respond more slowly and
lag farther. The default 0.60 returns faster than the earlier 0.25 setting. `drag_force`
controls damping: more drag removes more velocity each step. `allow_stretch=true`
lets joint translations and ring spacing change during motion; ring meshes stay rigid.
Set it to false for fixed-length swinging bones. Stretch mode currently supports
rest-only armatures with directly nested joints and collisions disabled; unsupported
configurations report a binding error. Existing hair retains fixed lengths by default.

The shared custom surface material uses single-layer alpha blending with 70% opacity
for the inner band and inserts, and 35% opacity for the pale outer band, self-lit
color gain, and a view-dependent rim highlight. It draws
the front shell of each closed ring. This first version has no transparent bloom
extraction and does not emit light onto nearby objects. The gain is a shader input,
so it does not require a renderer emissive flag or a new material base.

Automated checks cover shader compilation, transparency classification, rigid meshes,
rest capture, increasing lag, stretching, response tuning, basis transforms, settling,
parent rotation, and stable mesh uploads.
The standalone scene still needs visual review on desktop and device.
