"""Colour-coded report of tests/profile-interceptor-vantages.sh runs: vantages.md.

Estimated fps per vantage is 1000 / max(CPU, GPU) ms, where CPU is update +
prepare + record and GPU is the stereo graphics time. Headless: no compositor,
streaming or headset pacing, so headset fps will be lower.
"""
import json
import pathlib
import statistics
import sys

# One colour per vantage, in sweep order (see VANTAGES in
# vulkano_ship_profile_motion.rs). North is -Z; yaw is degrees left of north.
COLOURS = ["🟥", "🟧", "🟨", "🟩", "🟦", "🟪"]


def arrow(yaw, pitch):
    turn = "↑" if abs(yaw) < 1 else ("↖" if yaw > 0 else "↗")
    tilt = "▲" if pitch > 1 else ("▼" if pitch < -1 else "■")
    return turn + tilt


def fps_marker(fps):
    if fps >= 60.0:
        return "🟢"
    if fps >= 45.0:
        return "🟡"
    return "🔴"


def summarize_run(path):
    rows = json.loads(path.read_text())
    order, groups = [], {}
    for row in rows:
        vantage = row.get("vantage")
        if not vantage or vantage["settling"]:
            continue
        name = vantage["name"]
        if name not in groups:
            order.append(name)
            groups[name] = {"vantage": vantage, "rows": []}
        groups[name]["rows"].append(row)
    results = []
    for name in order:
        samples = groups[name]["rows"]
        gpu_samples = [r["gpu_graphics_ms"] for r in samples if r.get("gpu_graphics_ms") is not None]
        gpu = statistics.mean(gpu_samples) if gpu_samples else 0.0
        gpu_p95 = sorted(gpu_samples)[int(0.95 * (len(gpu_samples) - 1))] if gpu_samples else 0.0
        cpu = statistics.mean(r["update_ms"] + r["prepare_ms"] + r["record_submit_ms"] for r in samples)
        draws = statistics.mean(sum(g.get("calls", 0) for g in r["draws"].values()) for r in samples)
        results.append({"name": name, "vantage": groups[name]["vantage"], "gpu": gpu, "gpu_p95": gpu_p95,
                        "cpu": cpu, "draws": draws, "fps": 1000.0 / max(gpu, cpu), "samples": len(samples)})
    return results


def report(title, results):
    lines = [f"## {title}", "",
             "| | Vantage | Est. fps | GPU mean ms | GPU p95 ms | CPU ms | Draws | Limit |",
             "|---|---|---|---|---|---|---|---|"]
    for i, r in enumerate(results):
        v = r["vantage"]
        limit = "CPU" if r["cpu"] >= r["gpu"] else "GPU"
        lines.append(f"| {COLOURS[i % len(COLOURS)]} | {arrow(v['yaw'], v['pitch'])} {r['name']} | "
                     f"{fps_marker(r['fps'])} {r['fps']:.0f} | {r['gpu']:.2f} | {r['gpu_p95']:.2f} | "
                     f"{r['cpu']:.2f} | {r['draws']:.0f} | {limit} |")
    if results:
        worst = min(results, key=lambda r: r["fps"])
        lines += ["", f"Slowest: {worst['name']} at {worst['fps']:.0f} fps "
                      f"({worst['samples']} sampled frames per vantage).", ""]
    return lines


def main(out):
    lines = ["# Interceptor vantages", "",
             "🟢 ≥ 60 fps · 🟡 45–60 · 🔴 < 45 (estimated: 1000 / max(CPU, GPU) ms, headless).",
             "↑ forward (north), ↖ left, ↗ right; ▲ up, ■ level.", ""]
    for path in sorted(out.glob("vantage-*.frames.json")):
        lines += report(path.name.removesuffix(".frames.json"), summarize_run(path))
    (out / "vantages.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    print(f"Saved {out / 'vantages.md'}")


if __name__ == "__main__":
    main(pathlib.Path(sys.argv[1]))
