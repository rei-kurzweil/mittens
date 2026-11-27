#!/usr/bin/env python3
"""Cartoon grass tufts: a 2x2 atlas of four single tufts for the grass shader.

Each quarter holds one tuft drawn as outlined arches (see
generate_cartoon_grass.py): by default an upside-down U, an m, three n
shapes, and a second m. Its base sits on the quarter's bottom edge, centred,
and it fills most of the quarter's width. The shader
(assets/shaders/cartoon-grass.frag) scatters these over the ground and draws
each one upright toward the viewer.

RGB is the outline's lightness everywhere (so filtering never bleeds dark
edges); alpha is the outline's coverage, plus `--fill` inside the arches.

    python3 scripts/generate_grass_tufts.py
    python3 scripts/generate_grass_tufts.py --outline 10 --taper 0.55 --arches 1,2,3,4
"""
import argparse
import random
import types

import numpy as np
from PIL import Image, ImageDraw

from generate_cartoon_grass import tuft

SUPERSAMPLE = 4


def generate(args):
    rng = random.Random(args.seed)
    cell = args.size // 2
    big = args.size * SUPERSAMPLE
    line_layer = Image.new("L", (big, big), 0)
    fill_layer = Image.new("L", (big, big), 0)
    draw_line = ImageDraw.Draw(line_layer)
    draw_fill = ImageDraw.Draw(fill_layer)
    stroke = max(1, round(args.outline * SUPERSAMPLE))
    width = cell * args.width
    shape = types.SimpleNamespace(tuft_size=[width, width], taper=args.taper)
    arches = [int(a) for a in args.arches.split(",")]
    assert len(arches) == 4, "--arches needs four counts"
    for index, count in enumerate(arches):
        cx = (index % 2 + 0.5) * cell
        base_y = (index // 2 + 1) * cell - args.outline * 1.5
        curves = tuft(rng, shape, {count: 1.0})
        # Scale the tallest arch to fit the quarter's height.
        top = -min(y for curve in curves for _, y in curve)
        scale = min(1.0, (cell - args.outline * 4) / max(top, 1e-6))
        for curve in curves:
            points = [((cx + x * scale) * SUPERSAMPLE, (base_y + y * scale) * SUPERSAMPLE)
                      for x, y in curve]
            if args.fill > 0:
                draw_fill.polygon(points, fill=255)
            draw_line.line(points, fill=255, width=stroke, joint="curve")
            r = stroke / 2.0
            for end in (points[0], points[-1]):
                draw_line.ellipse([end[0] - r, end[1] - r, end[0] + r, end[1] + r], fill=255)

    def down(image):
        return np.asarray(image.resize((args.size, args.size), Image.LANCZOS), dtype=np.float64) / 255.0

    line, fill = down(line_layer), down(fill_layer)
    alpha = np.clip(line + fill * args.fill * (1 - line), 0, 1)
    rgb = np.full((args.size, args.size), args.line)
    # Where fill dominates, lighten instead of darken.
    lighter = fill * (1 - line) * args.fill
    rgb = np.where(alpha > 0, (line * args.line + lighter * 1.0) / np.maximum(line + lighter, 1e-6), args.line)
    out = np.dstack([np.clip(rgb, 0, 1)] * 3 + [alpha])
    Image.fromarray((out * 255 + 0.5).astype(np.uint8), "RGBA").save(args.out)
    print(f"wrote {args.out}: arches {arches}, {args.size}px")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--out", default="assets/textures/grass/cartoon-grass-tufts.png")
    parser.add_argument("--size", type=int, default=512, help="atlas size (four quarters)")
    parser.add_argument("--seed", type=int, default=11)
    parser.add_argument("--arches", default="1,2,3,2",
                        help="arch count per quarter (1 = U, 2 = m, 3+ = several n)")
    parser.add_argument("--width", type=float, default=0.82, help="tuft width / quarter width")
    parser.add_argument("--outline", type=float, default=9.0, help="outline width in pixels")
    parser.add_argument("--taper", type=float, default=0.45,
                        help="how far legs lean in at the bottom (0 straight, 1 meet)")
    parser.add_argument("--fill", type=float, default=0.0,
                        help="opacity of a lighter fill inside the arches")
    parser.add_argument("--line", type=float, default=0.50, help="outline lightness")
    generate(parser.parse_args())


if __name__ == "__main__":
    main()
