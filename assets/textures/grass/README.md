# Grass

## Cartoon grass (`cartoon-grass.png`)

Used by every grass surface: the ship park, the Sakura street, target
practice and the truck bed demo. Regenerate with:

```sh
python3 scripts/generate_cartoon_grass.py
```

A seamless 1024 px greyscale tile: soft lightness patches with little tufts
drawn as dark outlined arches (an upside-down U, an m, or several n shapes),
legs leaning in toward the bottom. It is one hue in several lightnesses, so
each scene colours it with the material colour (`C.rgba` multiplies it). One
repeat is meant to cover about 4 m. Options: `--outline` (line width, px),
`--taper` (how far legs lean in), `--shapes` (arch count weights, e.g.
`1:0.35,2:0.4,3:0.2,4:0.05`), `--tufts`, `--tuft-size`, `--fill` (lighten
tuft interiors), `--ground`, `--patch`, `--line`, `--seed`, `--size`,
`--out`.

The park's hills are an ImplicitSurface whose UVs are world metres, so they
use `.uv_scale(0.25)` to match.

## Previous ship park grass (`ship-park-grass.png`, unused)

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
