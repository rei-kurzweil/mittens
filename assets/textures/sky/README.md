# Kawaii twilight sky masks

Regenerate from the repository root:

```sh
cargo run --release --example generate-twilight-moon
cargo test --example generate-twilight-moon
```

`kawaii-twilight-masks.png` is a 1024×512 **linear UNORM data texture**:

| Channel | Meaning | Sampling coordinates |
| --- | --- | --- |
| R | Unmodified red channel of `../space/sky-cloud-mask.png` | Spherical panorama |
| G | Antialiased crescent coverage | Moon-local square spanning ±3 moon radii |
| B | Offline multi-scale blur of actual crescent coverage | Same moon-local square |
| A | Constant 255 | Unused by sky shading |

The generator supersamples the crescent and applies three separable Gaussian
blurs with sigmas 0.06, 0.18, and 0.35 moon radii. Different horizontal/vertical
pixel sigmas account for the packed image's panorama aspect ratio. The shader
maps those channels back to a square angular domain, so the moon is circular.
Body and glow have transparent-equivalent zero padding; clouds retain the source
panorama's continuous seam and pole rows. Do not color-convert or upload these
masks as sRGB, or use the packed PNG as ordinary visible RGBA artwork.

`crescent-moon-glow.png` is a separate 512×512 tinted, straight-alpha preview.
Its alpha combines body and glow; transparent border RGB is zero. It is for
inspection/reuse as art, not the runtime sky's intensity/mask source. Runtime
shading samples independent body/glow channels and adds tinted radiance to the
gradient/cloud sky. The crescent cutout leaves that sky visible, and no alpha
blend, runtime blur, or extra render target is needed.

The material is `assets/components/materials/kawaii-twilight-sky.mms`; the
background wrapper is `assets/components/backgrounds/kawaii-twilight-sky.mms`.
Use `kawaii_twilight_sky(false)` for defaults, or pass a table of numeric overrides.
The material-only factory expects this packed texture on the consuming renderable.
Colors are linear shader RGB, angular radius/orientation are radians, and gradient
stop positions are normalized view-direction Y. Plain `BG` deliberately keeps
this sky out of bloom extraction while its baked glow remains visible in color.
