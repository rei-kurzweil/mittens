# Grass

## Cartoon grass tufts (`cartoon-grass-tufts.png`)

Used by every grass surface (the ship park, the Sakura street, target
practice, the truck bed demo) through `cartoon_grass(bucket, options)` in
`assets/components/materials/cartoon_grass.mms` and its shader
`assets/shaders/cartoon-grass.frag`. Regenerate with:

```sh
python3 scripts/generate_grass_tufts.py
```

A 512 px 2x2 atlas of four single tufts, each drawn as outlined arches (an
upside-down U, an m, three n shapes, another m), base on the bottom edge of
its quarter. RGB is the outline's lightness and alpha its coverage. Options:
`--arches` (four arch counts), `--outline` (line width, px), `--taper` (how
far legs lean in at the bottom), `--width`, `--fill`, `--line`, `--seed`,
`--size`, `--out`.

The shader paints the ground as the material colour times a soft lightness
pattern and scatters tufts by world position (at most one per
`cell_size` cell, `density` of them filled, random size and variant). Each
tuft is drawn upright toward the viewer with its curved side up, wherever
you look from, and tufts fade out between `fade_near` and `fade_far`. One
colour, several lightnesses: each scene sets the colour with `C.rgba`.

`scripts/generate_cartoon_grass.py` draws the same tufts into a seamless
ground tile; it is no longer used by any scene.

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
