"""Colour-coded report of tests/profile-interceptor-vantages.sh runs: vantages.md.

Estimated fps per vantage is 1000 / max(CPU, GPU) ms, where CPU is update +
prepare + record and GPU is the stereo graphics time. Headless: no compositor,
streaming or headset pacing, so headset fps will be lower.
"""
import json
import pathlib
import statistics
import sys

# North is -Z, east is +X. Sections are a 3 x 3 grid of the deck.
SECTIONS = ["NW", "N", "NE", "W", "C", "E", "SW", "S", "SE"]
SECTION_COLOUR = {"NW": "🟥", "N": "🟧", "NE": "🟨", "W": "🟩", "C": "⬜",
                  "E": "🟦", "SW": "🟪", "S": "🟫", "SE": "⬛"}
SECTION_NAME = {"NW": "north-west", "N": "north", "NE": "north-east", "W": "west",
                "C": "centre", "E": "east", "SW": "south-west", "S": "south", "SE": "south-east"}
COMPASS = {(0, -1): ("↑", "north"), (1, -1): ("↗", "north-east"), (1, 0): ("→", "east"),
           (1, 1): ("↘", "south-east"), (0, 1): ("↓", "south"), (-1, 1): ("↙", "south-west"),
           (-1, 0): ("←", "west"), (-1, -1): ("↖", "north-west")}


def direction_label(d):
    x, y, z = d
    if (x, z) == (0, 0):
        return ("⤒ straight up", "⤒") if y > 0 else ("⤓ straight down", "⤓")
    arrow, name = COMPASS[(x, z)]
    tilt = {1: ("▲", ", upward"), 0: ("■", ""), -1: ("▼", ", downward")}[y]
    return f"{arrow}{tilt[0]} {name}{tilt[1]}", f"{arrow}{tilt[0]}"


def fps_marker(fps):
    if fps >= 60.0:
        return "🟢"
    if fps >= 45.0:
        return "🟡"
    return "🔴"


def summarize_run(path):
    rows = json.loads(path.read_text())
    groups = {}
    for row in rows:
        vantage = row.get("vantage")
        if not vantage or vantage["settling"]:
            continue
        key = (vantage["section"], tuple(vantage["direction"]))
        groups.setdefault(key, []).append(row)
    vantages = {}
    for key, samples in groups.items():
        gpu_samples = [r["gpu_graphics_ms"] for r in samples if r.get("gpu_graphics_ms") is not None]
        gpu = statistics.mean(gpu_samples) if gpu_samples else 0.0
        cpu = statistics.mean(r["update_ms"] + r["prepare_ms"] + r["record_submit_ms"] for r in samples)
        draws = statistics.mean(sum(g.get("calls", 0) for g in r["draws"].values()) for r in samples)
        vantages[key] = {"gpu": gpu, "cpu": cpu, "draws": draws, "fps": 1000.0 / max(gpu, cpu)}
    return vantages


def report(title, vantages):
    directions = sorted({d for _, d in vantages}, key=lambda d: (-d[1], d[2], d[0]))
    lines = [f"## {title}", ""]
    # Map view: each section's worst and mean fps, laid out north-up.
    lines += ["Worst / mean estimated fps by section (north at the top):", "",
              "| | West | Centre | East |", "|---|---|---|---|"]
    for row_name, row in [("North", ["NW", "N", "NE"]), ("Middle", ["W", "C", "E"]), ("South", ["SW", "S", "SE"])]:
        cells = []
        for section in row:
            fps = [v["fps"] for (s, _), v in vantages.items() if s == section]
            if fps:
                cells.append(f"{SECTION_COLOUR[section]} {section} {fps_marker(min(fps))} {min(fps):.0f} / {statistics.mean(fps):.0f}")
            else:
                cells.append("-")
        lines.append(f"| {row_name} | " + " | ".join(cells) + " |")
    # Full grid: directions down, sections across.
    lines += ["", "Estimated fps per vantage:", "",
              "| Direction | " + " | ".join(f"{SECTION_COLOUR[s]} {s}" for s in SECTIONS) + " |",
              "|---|" + "---|" * len(SECTIONS)]
    for d in directions:
        cells = []
        for section in SECTIONS:
            v = vantages.get((section, d))
            cells.append("-" if not v else f"{fps_marker(v['fps'])} {v['fps']:.0f}")
        lines.append(f"| {direction_label(d)[0]} | " + " | ".join(cells) + " |")
    # Direction averages across the deck.
    lines += ["", "By direction (mean fps across sections, worst section):", ""]
    by_direction = []
    for d in directions:
        values = [(vantages[(s, d)]["fps"], s) for s in SECTIONS if (s, d) in vantages]
        by_direction.append((statistics.mean(v for v, _ in values), min(values), d))
    for mean_fps, (worst, section), d in sorted(by_direction):
        lines.append(f"- {fps_marker(mean_fps)} {direction_label(d)[0]}: {mean_fps:.0f} fps mean, "
                     f"worst {worst:.0f} from {SECTION_COLOUR[section]} {SECTION_NAME[section]}")
    # Hot spots and vantage points.
    ranked = sorted(vantages.items(), key=lambda item: item[1]["fps"])
    def describe(item):
        (section, d), v = item
        return (f"{fps_marker(v['fps'])} {SECTION_COLOUR[section]} {SECTION_NAME[section]} looking "
                f"{direction_label(d)[0]}: {v['fps']:.0f} fps (GPU {v['gpu']:.2f} ms, CPU {v['cpu']:.2f} ms, "
                f"{v['draws']:.0f} draws)")
    lines += ["", "Hot spots (slowest 10 vantages):", ""] + [f"1. {describe(i)}" for i in ranked[:10]]
    lines += ["", "Fastest 5 vantages:", ""] + [f"1. {describe(i)}" for i in ranked[-5:][::-1]]
    limited = sum(1 for _, v in vantages.items() if v["cpu"] >= v["gpu"])
    lines += ["", f"CPU-limited at {limited} of {len(vantages)} vantages (CPU time >= GPU time).", ""]
    return lines


def main(out):
    lines = ["# Interceptor vantage sweep", "",
             "🟢 ≥ 60 fps · 🟡 45–60 · 🔴 < 45 (estimated: 1000 / max(CPU, GPU) ms, headless).",
             "Sections: " + " ".join(f"{SECTION_COLOUR[s]} {SECTION_NAME[s]}" for s in SECTIONS) + ".",
             "Tilt: ▲ upward, ■ level, ▼ downward; ⤒ / ⤓ straight up / down. North is -Z.", ""]
    for path in sorted(out.glob("vantage-*.frames.json")):
        lines += report(path.name.removesuffix(".frames.json"), summarize_run(path))
    (out / "vantages.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    print(f"Saved {out / 'vantages.md'}")


if __name__ == "__main__":
    main(pathlib.Path(sys.argv[1]))
