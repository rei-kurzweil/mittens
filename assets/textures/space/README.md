# Ship nebula panorama

```sh
cargo run --release --example generate-ship-nebula-texture
```

The optional directory argument writes a preview elsewhere. The deterministic
Rust generator bakes spherical cloud noise into a 1024 × 512 black/purple panorama.
Longitude borders match exactly, and both pole rows are uniform.

`ship-space-sky.frag` performs one texture lookup for the nebula. It adds an
analytic crescent and purple halo; it does not evaluate cloud noise at runtime.
The background's extracted radiance feeds the scene bloom. The halo also remains
visible with bloom disabled. Golden kawaii stars are separate geometry at 60%
of their usual size. All layers use the rotation-only background, so views from
the bridge windscreen, cafeteria windows, park dome and mirrors share one sky.
