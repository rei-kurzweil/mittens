"""Tabulate tests/profile-interceptor-matrix.sh reports into results.md and results.csv."""
import csv
import json
import pathlib
import sys

MSAA_ORDER = ["off", "2x", "4x"]
FIELDS = ["npcs", "msaa", "bloom", "gpu_mean_ms", "gpu_p95_ms", "cpu_total_mean_ms",
          "update_mean_ms", "draws", "weapon_held", "gpu", "extent"]


def metric(timing, key, stat):
    value = timing.get(key, {}).get(stat)
    return None if value is None else round(value, 3)


def main(out):
    rows = []
    for path in sorted(out.glob("npcs*.json")):
        if path.name.endswith(".frames.json"):
            continue
        report = json.loads(path.read_text())
        timing = report["timing"]
        held = report.get("weapon_held_fraction")
        rows.append({
            "npcs": report.get("interceptor_npcs"),
            "msaa": report.get("msaa"),
            "bloom": "on" if report.get("bloom_active") else "off",
            "gpu_mean_ms": metric(timing, "gpu_graphics_ms", "mean_ms"),
            "gpu_p95_ms": metric(timing, "gpu_graphics_ms", "p95_ms"),
            "cpu_total_mean_ms": metric(timing, "total_ms", "mean_ms"),
            "update_mean_ms": metric(timing, "update_ms", "mean_ms"),
            "draws": report.get("sample_draws"),
            "weapon_held": None if held is None else round(held, 3),
            "gpu": report.get("gpu"),
            "extent": "x".join(str(v) for v in report.get("extent", [])),
        })
    rows.sort(key=lambda r: (r["npcs"] or 0,
                             MSAA_ORDER.index(r["msaa"]) if r["msaa"] in MSAA_ORDER else 9,
                             r["bloom"]))
    with open(out / "results.csv", "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=FIELDS)
        writer.writeheader()
        writer.writerows(rows)
    lines = ["| NPCs | MSAA | Bloom | GPU mean ms | GPU p95 ms | CPU frame ms | Update ms | Launcher held |",
             "|---|---|---|---|---|---|---|---|"]
    for r in rows:
        held = "-" if r["weapon_held"] is None else f"{r['weapon_held']:.0%}"
        lines.append(f"| {r['npcs']} | {r['msaa']} | {r['bloom']} | {r['gpu_mean_ms']} | {r['gpu_p95_ms']} | "
                     f"{r['cpu_total_mean_ms']} | {r['update_mean_ms']} | {held} |")
    if rows:
        lines += ["", f"GPU: {rows[0]['gpu']}, per-eye extent {rows[0]['extent']}; ms per stereo frame."]
    (out / "results.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    print(f"\nSaved {out / 'results.md'} and {out / 'results.csv'}")


if __name__ == "__main__":
    main(pathlib.Path(sys.argv[1]))
