# Transparent Noto Sakura sprite

`sakura-blossom-256.png` is one Noto Color Emoji 🌸 glyph on a transparent
256×256 RGBA canvas. It is separate from the opaque blossom-pattern texture.

Regenerate from retained Rust source:

```sh
cargo run --release --example generate-sakura-sprite
# Or select the bitmap font explicitly:
cargo run --release --example generate-sakura-sprite -- --font /path/to/NotoColorEmoji.ttf
```

The generator reads the font's largest embedded PNG glyph, crops transparent
margins, fits it inside 224×224 pixels and centres it with transparent padding.
Resampling uses premultiplied channels, then restores straight alpha. Edge RGB is
bled into nearby transparent texels to avoid dark bilinear-filter fringes.
The local NotoColorEmoji.ttf remains external; no font binary is bundled.

`assets/components/plants/sakura_tree.mms` reuses the original mitten tree's
branch angles and three-way recursion, with cylindrical branches and crossed
textured planes at every leaf endpoint. At depth=3 there are 27 endpoints and
81 double-sided blossom planes. TransparentCutout uses the existing alpha
coverage path, so transparent corners don't create opaque squares or require
per-frame sorting/billboarding. Both the Sakura rooftop scene and mitten garden
use the same factory:

```mms
import { sakura_tree } from "../assets/components/plants/sakura_tree.mms"
sakura_tree(3, 1.15, 0.24)
```

Arguments are recursion depth, initial branch length and initial branch diameter.
