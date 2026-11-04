# Sky cloud mask

```sh
cargo run --release --example generate-sky-cloud-texture
```

The optional directory argument writes a preview elsewhere. The deterministic
Rust generator bakes spherical cloud noise into `sky-cloud-mask.png`, a
1024 × 512 monochrome panorama. RGB channels are equal and alpha is opaque.
Longitude borders match exactly, and both pole rows are uniform.

The station shader samples one channel and applies its own cloud tint over the
same two-stop gradient as Sakura drone conversion. Colors, gradient stops and
cloud strength live at the top of `examples/ship_interior.mms`. It retains the
analytic crescent and a halo tinted from the sky palette, with no runtime noise.
Pale cream-white stars are separate geometry at 30% of the shared default size, half their
previous station radius. All layers use the rotation-only background.

The older `ship-nebula.png` and `generate-ship-nebula-texture` are retained for
reference; the station no longer samples their baked purple palette.
