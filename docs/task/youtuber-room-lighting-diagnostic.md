# Youtuber room: isolated point-light comparison

Date: 2026-10-24
Status: implemented; headset comparison pending.

Run the rebuilt executable:

```sh
./target/release/mittens-engine load examples/youtuber-room.mms
```

The example starts in **Anime** mode with facing-gated distance lighting. Three
pointer buttons select raw Distance only, raw Smooth diffuse, or Anime facing.
Desktop pointers and avatar-free XR lasers operate them, and the heading reports
the selected mode. The former normal-blend setting and slider have been removed.

The architectural shell and floor share gray albedo. The monitor supplies the
only point light (white, intensity 1.5, range 5 m). Directional light, spotlight,
and ambient are disabled. Rim lighting is restored in Anime mode at strength
0.18, power 4, with a white tint, including on the seated NPC, straw and strip
backing. The built-in default was never disabled: it remains strength 0.18,
power 4, with the original warm pink tint. Raw Distance/Diffuse diagnostic modes
still bypass rim. LED emission and Bloom have been restored; the
explicit `led_effects_enabled` flag near the top of the example controls both.
Set it to false for an isolated comparison. The GPU diagnostic fixture does
this automatically; the interactive scene starts with the effects enabled.

## Seated Rei(mu) comparison

Rei(mu) is a separate NPC at the middle of the left wall, with no AvatarControl,
XR camera or tracked hands. Her imported mesh uses the **same `lighting_probe`
source** as the room, so all three buttons apply
to both static room geometry and her skinned meshes without losing textures.

The [floor-seated pose](../../assets/components/poses/rei-mu/floor-seated.pose.mms)
has about 110 degrees of hip flexion, a small backward lean, raised knees and
low shin slopes. Her feet are about 52 cm apart, with shins extending forward.
Elbows stay nearer the torso than the wrists; the upper-arm roll aligns the
elbow hinge so forearm flexion uses bind-local positive X instead of arbitrary
body-space rotation/twist. Both complete arm chains then swing backward 30
degrees about the avatar's lateral X axis at the shoulders. Wrists sit about
20 cm above the floor, with fingertips still roughly 15 cm clear.
Ankles ease 40% from the former flat-sole pose toward each foot's bind-local
rest rotation relative to the posed lower leg. Heels contact the floor and toe
ends sit about 5 cm above it; the feet are not counter-rotated to stay flat.

The scene reuses the existing hair/tail/shirt secondary-motion setup, body
colliders and bow-ribbon chains. `GLTFInitialized` attaches the existing ambient
eye-saccade animation to the actual eye joints, using the scene's 120 BPM clock.
There is no dependency on player tracking or microphone input. A close-up GPU
preview is saved to `/tmp/youtuber-reimu-seated.png`.

All three modes run inside the existing Anime shader, using the same actual
engine light buffer, transformed world positions, normals, and attenuation:

| Mode | Output |
| --- | --- |
| Distance only | Albedo times clamped intensity/attenuation, without the normal factor |
| Smooth diffuse | Albedo times clamped accumulated Lambert diffuse lighting, without the Anime ramp |
| Anime facing | Shade/lit interpolation driven by distance/cone-attenuated intensity from facing lights, plus the restored view-dependent rim |

The current built-in Anime rule is evaluated per light: `dot(N, L) > 0` admits
its full nonnegative intensity multiplied by distance and spotlight-cone
attenuation. Tangent and back-facing surfaces admit none. The cosine magnitude
does not weight admitted light. Contributions are summed before the existing
shade/lit ramp; surfaces without admitted light keep the configured shade color.
The cutoff is fixed at 90 degrees, with no angular fade or blend input.

Follow-up: add a ramp around the N-dot-L facing gate once this hard-cutoff version
has been evaluated in VR. Gate threshold/width authoring is not implemented yet.

The temporary scene setup is explicit in `examples/youtuber-room.mms`. Restored
Bloom settings are intensity 2.25, Bloom radius 0.086, and extraction blur radius
0.04. The flag's false branch uses `RenderGraph.off()` and zero LED intensity.
Previous ambient was 0.12,
directional intensity 0.26, and monitor spotlight intensity 0.65. Architectural
colors can be restored after the experiment. Rim lighting has been restored;
set the scene's Anime declarations to `.rim_strength(0.0)` to isolate it again.

The reusable diagnostic API is:

```mms
let probe = Shading.anime().rim_strength(0.0)
probe.set_lighting_debug("distance")
probe.set_lighting_debug("diffuse")
probe.get_lighting_debug() // "diffuse"
probe.set_lighting_debug("anime")
```

`anime` is the default, using the facing-gated Anime ramp. The selected mode
occupies the previously reserved W component of the rim-color GPU parameter;
the uniform-buffer layout stays the same. Switching modes updates the retained
material revision without changing instance or pipeline identity. Invalid
mode names and Toon targets are rejected. Nondefault modes serialize through
the `lighting_debug` builder.


Validation covers live getters/setters and invalid targets, real scene loading
and all three button callbacks, and stereo GPU rendering of both raw diagnostic
views and Anime facing. The shared material identity stays stable. A controlled
GPU receiver test compares head-on and grazing incidence at equal distance,
then moves the light behind the receiver: facing values match, while the back
side stays shaded.
The character checks cover posed joint heights, narrower foot spacing,
bind-local elbow flexion, successfully bound secondary-motion chains, actual
eye animation, absence of player AvatarControl, and shared skinned material
bindings across all modes.
Previews are saved as `/tmp/youtuber-lighting-distance.png`,
`/tmp/youtuber-lighting-diffuse.png`, and `/tmp/youtuber-lighting-anime.png`. The LED emission proof explicitly reenables
Bloom and LED intensity and omits the diagnostic panel in its test fixture while
the interactive scene is in diagnostic mode.
