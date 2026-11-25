#!/usr/bin/env python3
"""Seamless cartoon grass: one hue in several lightnesses, for tinting.

Little tufts are scattered over soft lightness patches. Each tuft is a row of
one to four arches drawn as outlined curves: an upside-down U, an m, or
several n shapes side by side. Neighbouring arches share a leg, and every leg
leans in toward the tuft's centre at the bottom (`--taper`), so a tuft is
wider at the top than where it meets the ground. Outlines are darker than the ground;
`--fill` optionally lightens arch interiors.

The image is greyscale. Colour it with the material's colour (C.rgba
multiplies the texture), so one texture serves every scene's grass.

    python3 scripts/generate_cartoon_grass.py
    python3 scripts/generate_cartoon_grass.py --outline 4 --taper 0.6 \
        --shapes 1:0.2,2:0.5,3:0.3 --out /tmp/grass.png

Opposite edges match: tufts crossing an edge wrap to the other side.
"""
import argparse
import math
import random

import numpy as np
from PIL import Image, ImageDraw

SUPERSAMPLE = 4


def parse_shapes(text):
    weights = {}
    for part in text.split(","):
        arches, weight = part.split(":")
        weights[int(arches)] = float(weight)
    return weights


def tileable_noise(size, rng, frequencies):
    """Smooth periodic noise in [-1, 1] from a few integer-frequency waves."""
    y, x = np.mgrid[0:size, 0:size] / size
    total = np.zeros((size, size))
    for f in frequencies:
        for _ in range(3):
            fx, fy = rng.randint(-f, f), rng.randint(-f, f)
            if fx == 0 and fy == 0:
                fx = f
            phase = rng.random() * math.tau
            total += np.sin(math.tau * (fx * x + fy * y) + phase) / f
    return total / max(np.abs(total).max(), 1e-6)


def bezier(p0, p1, p2, p3, steps=28):
    points = []
    for i in range(steps + 1):
        t = i / steps
        u = 1.0 - t
        points.append(tuple(
            u * u * u * p0[k] + 3 * u * u * t * p1[k] + 3 * u * t * t * p2[k] + t * t * t * p3[k]
            for k in range(2)
        ))
    return points


def tuft(rng, args, weights):
    """The arches of one tuft, as curves around (0, 0) at the base centre."""
    arches = rng.choices(list(weights), list(weights.values()))[0]
    width = rng.uniform(args.tuft_size[0], args.tuft_size[1])
    height = width * rng.uniform(0.45, 0.8)
    lean = rng.uniform(-0.18, 0.18) * height
    # Leg tops across the tuft's width, unevenly spaced.
    cuts = sorted(rng.uniform(0.25, 0.75) for _ in range(arches - 1))
    spans = [0.0] + cuts + [1.0] if arches > 1 else [0.0, 1.0]
    if arches > 1:
        spans = [i / arches + rng.uniform(-0.25, 0.25) / arches for i in range(arches + 1)]
        spans[0], spans[-1] = 0.0, 1.0
    tops = [(s - 0.5) * width for s in spans]
    bases = [x * (1.0 - args.taper) for x in tops]
    curves = []
    for i in range(arches):
        h = height * rng.uniform(0.75, 1.0)
        if arches >= 3 and 0 < i < arches - 1:
            h = height
        # Controls above the leg tops give the round arch; the base points
        # are pulled toward the centre.
        reach = h * 1.33
        p0 = (bases[i], 0.0)
        p1 = (tops[i] + lean * 0.8, -reach)
        p2 = (tops[i + 1] + lean * 0.8, -reach)
        p3 = (bases[i + 1], 0.0)
        curves.append(bezier(p0, p1, p2, p3))
    return curves


def generate(args):
    rng = random.Random(args.seed)
    size = args.size
    weights = parse_shapes(args.shapes)

    patches = tileable_noise(size, rng, [2, 3, 5]) * args.patch
    fine = tileable_noise(size, rng, [13, 21]) * args.patch * 0.25
    ground = np.clip(args.ground + patches + fine, 0.0, 1.0)

    big = size * SUPERSAMPLE
    fill_layer = Image.new("L", (big, big), 0)
    fill_value = Image.new("L", (big, big), 0)
    line_layer = Image.new("L", (big, big), 0)
    line_value = Image.new("L", (big, big), 0)
    draw_fill_mask = ImageDraw.Draw(fill_layer)
    draw_fill_value = ImageDraw.Draw(fill_value)
    draw_line_mask = ImageDraw.Draw(line_layer)
    draw_line_value = ImageDraw.Draw(line_value)

    # Spread tufts out: keep candidates at least min_gap apart (wrapping).
    placed = []
    min_gap = args.tuft_size[0] * 0.9
    attempts = 0
    while len(placed) < args.tufts and attempts < args.tufts * 60:
        attempts += 1
        x, y = rng.uniform(0, size), rng.uniform(0, size)
        ok = True
        for px, py in placed:
            dx = min(abs(x - px), size - abs(x - px))
            dy = min(abs(y - py), size - abs(y - py))
            if dx * dx + dy * dy < min_gap * min_gap:
                ok = False
                break
        if ok:
            placed.append((x, y))
    placed.sort(key=lambda p: p[1])

    stroke = max(1, round(args.outline * SUPERSAMPLE))
    for cx, cy in placed:
        curves = tuft(rng, args, weights)
        shade = rng.uniform(-1.0, 1.0)
        fill = int(255 * np.clip(args.ground + args.fill + shade * args.patch * 0.5, 0, 1))
        line = int(255 * np.clip(args.line + shade * 0.05, 0, 1))
        for ox in (-size, 0, size):
            for oy in (-size, 0, size):
                for curve in curves:
                    points = [((cx + ox + px) * SUPERSAMPLE, (cy + oy + py) * SUPERSAMPLE)
                              for px, py in curve]
                    if all(p[0] < -stroke or p[0] > big + stroke for p in points):
                        continue
                    if all(p[1] < -stroke or p[1] > big + stroke for p in points):
                        continue
                    if args.fill == 0.0:
                        continue
                    draw_fill_mask.polygon(points, fill=255)
                    draw_fill_value.polygon(points, fill=fill)
                for curve in curves:
                    points = [((cx + ox + px) * SUPERSAMPLE, (cy + oy + py) * SUPERSAMPLE)
                              for px, py in curve]
                    if all(p[0] < -stroke or p[0] > big + stroke for p in points):
                        continue
                    if all(p[1] < -stroke or p[1] > big + stroke for p in points):
                        continue
                    draw_line_mask.line(points, fill=255, width=stroke, joint="curve")
                    draw_line_value.line(points, fill=line, width=stroke, joint="curve")
                    r = stroke / 2.0
                    for end in (points[0], points[-1]):
                        box = [end[0] - r, end[1] - r, end[0] + r, end[1] + r]
                        draw_line_mask.ellipse(box, fill=255)
                        draw_line_value.ellipse(box, fill=line)

    def down(image):
        return np.asarray(image.resize((size, size), Image.LANCZOS), dtype=np.float64) / 255.0

    fill_alpha, line_alpha = down(fill_layer), down(line_layer)
    # Values are drawn at full coverage; divide out the antialiased edge.
    fill_shade = down(fill_value) / np.maximum(fill_alpha, 1e-3)
    line_shade = down(line_value) / np.maximum(line_alpha, 1e-3)
    out = ground * (1 - fill_alpha) + fill_shade * fill_alpha
    out = out * (1 - line_alpha) + line_shade * line_alpha
    gray = (np.clip(out, 0, 1) * 255 + 0.5).astype(np.uint8)
    Image.fromarray(np.stack([gray] * 3, axis=-1), "RGB").save(args.out)
    print(f"wrote {args.out}: {len(placed)} tufts, {size}px")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--out", default="assets/textures/grass/cartoon-grass.png")
    parser.add_argument("--size", type=int, default=1024)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--tufts", type=int, default=140)
    parser.add_argument("--tuft-size", type=float, nargs=2, default=[30.0, 58.0],
                        help="tuft width range in pixels")
    parser.add_argument("--outline", type=float, default=3.0, help="outline width in pixels")
    parser.add_argument("--taper", type=float, default=0.45,
                        help="how far legs lean in at the bottom (0 straight, 1 meet)")
    parser.add_argument("--shapes", default="1:0.35,2:0.4,3:0.2,4:0.05",
                        help="arch count:weight pairs (1 = U, 2 = m, 3+ = several n)")
    parser.add_argument("--ground", type=float, default=0.80, help="ground lightness")
    parser.add_argument("--patch", type=float, default=0.06, help="lightness variation")
    parser.add_argument("--fill", type=float, default=0.0,
                        help="tuft interior lightening (0 leaves it the ground)")
    parser.add_argument("--line", type=float, default=0.50, help="outline lightness")
    generate(parser.parse_args())


if __name__ == "__main__":
    main()
