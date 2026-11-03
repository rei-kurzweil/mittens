# Peanut-butter jar artwork

Regenerate the checked-in PNGs from the repository root:

```sh
cargo run --release --example generate-peanut-butter-texture
```

An optional directory argument writes previews elsewhere. The generator uses
only Rust and the existing `image` dependency; no fonts or downloaded images
are required. Edit its palette constants, `peanut`, `badge`, and lettering to
refine the design.

- `peanut-butter-wrapper.png`: 1024 × 512, light red wrapper with two repeated
  medallions and product lettering for the front and back of a jar.
- `peanut-butter-lid.png`: 512 × 512, red cap with a circular gradient badge
  and a dark peanut silhouette.

`assets/components/groceries.mms` supplies brown jar bodies, solid red cap
sides, cylindrical wrapper UVs, and a separate top disc for the lid artwork.
`grocery_shelf_stock.mms` batches the bodies/caps, wrappers, and top discs
separately so `CombineMesh` preserves each texture. The same `retail_shelf`
factory supplies the stores and barracks; its second argument selects stock
(`packages`, `oranges`, `bananas`, `toilet_paper`, `peanut_butter`, or `empty`).
