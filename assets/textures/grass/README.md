# Ship park grass

Regenerate the seamless albedo with:

```sh
cargo run --release --example generate-ship-grass-texture
```

An optional directory argument writes a preview elsewhere. The seed and palette
are in the Rust source. Wrapped value-noise patches blend pale yellow, green and
brown; short antialiased strokes use varied angles and lengths. Opposite image
edges match exactly, including strokes crossing the tile boundary.

The regular park uses this texture on its flat ground and clipped implicit
hills. The hill mesh supplies its own triangle collision; the paths stay level.
