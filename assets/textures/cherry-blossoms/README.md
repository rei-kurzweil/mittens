# Cherry blossom background

Regenerate the 2048 × 2048 opaque white PNG from the actual Noto Color Emoji 🌸 glyph:

```sh
cargo run --release --example generate-cherry-blossoms
```

The upper-left side has fewer, much larger blossoms, transitioning into a populated band of small blossoms in the
middle. Beyond that band, blossoms become slightly smaller and much sparser,
leaving generous white space toward the right and bottom. Placement density is
reduced where blossoms are large to give them breathing room.
All blossoms render at twice the previous size, keeping the same placement
pattern while covering more of the white space. Five touching grey squares sit
flush with the bottom-left edge, progressing from dark grey to light grey
(`#404040`, `#686868`, `#909090`, `#b8b8b8`, `#e0e0e0`). Each square is
one-sixteenth of the shorter image dimension on each side.
Three scales of seeded value noise create
clusters and gaps; a separate noise field varies sizes, with individual random
rotation and opacity. The seed reproduces the same pattern. This is a directional
background, so it is not seamless across opposite edges like the pool tiles.

```sh
cargo run --release --example generate-cherry-blossoms -- \
  --width 2560 --height 1440 --seed 123 --density 1.5 \
  --sparse-side left --output /tmp/cherry-blossoms.png
```

Use `--help` for all options. Sizes scale with the shorter image dimension.
`--density 0` produces white with only the grey squares; the default density is 1.

The generator auto-detects `NotoColorEmoji.ttf` in common Linux font directories.
On other systems, pass `--font /path/to/NotoColorEmoji.ttf`. The font must contain
the bitmap PNG version of the color glyph (CBDT/CBLC); SVG and COLR vector fonts
are not supported by this generator. The font stays external and is not bundled
in the repository. Rendering requires no display, GPU, or network connection.
