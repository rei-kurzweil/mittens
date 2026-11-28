"""Tabulate tests/profile-interceptor-matrix.sh reports into results.md and results.csv."""
import csv
import json
import pathlib
import sys

MSAA_ORDER = ["off", "2x", "4x"]
FIELDS = ["npcs", "npcs_present", "msaa", "bloom", "gpu_mean_ms", "gpu_p95_ms", "cpu_total_mean_ms",
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
            "npcs_present": report.get("npcs_present"),
            "draws": sum(group.get("calls", 0) for group in (report.get("sample_draws") or {}).values()),
            "weapon_held": None if held is None else round(held, 3),
            "gpu": report.get("gpu"),
            "extent": "x".join(str(v) for v in report.get("extent", [])),
            "stages": {name: summary.get("mean_ms", 0.0)
                       for name, summary in (report.get("update_stages_ms") or {}).items()},
        })
    rows.sort(key=lambda r: (r["npcs"] or 0,
                             MSAA_ORDER.index(r["msaa"]) if r["msaa"] in MSAA_ORDER else 9,
                             r["bloom"]))
    with open(out / "results.csv", "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=FIELDS, extrasaction="ignore")
        writer.writeheader()
        writer.writerows(rows)
    lines = ["| NPCs (present) | MSAA | Bloom | GPU mean ms | GPU p95 ms | CPU frame ms | Update ms | Draws | Launcher held |",
             "|---|---|---|---|---|---|---|---|---|"]
    for r in rows:
        held = "-" if r["weapon_held"] is None else f"{r['weapon_held']:.0%}"
        lines.append(f"| {r['npcs']} ({r['npcs_present']}) | {r['msaa']} | {r['bloom']} | {r['gpu_mean_ms']} | {r['gpu_p95_ms']} | "
                     f"{r['cpu_total_mean_ms']} | {r['update_mean_ms']} | {r['draws']} | {held} |")
    if rows:
        lines += ["", f"GPU: {rows[0]['gpu']}, per-eye extent {rows[0]['extent']}; ms per stereo frame."]
    # Where update time goes as NPCs are added: one run per NPC count.
    by_npcs = {}
    for r in rows:
        if r["stages"] and (r["npcs"] not in by_npcs or (r["msaa"], r["bloom"]) == ("4x", "off")):
            by_npcs[r["npcs"]] = r
    if by_npcs:
        lines += ["", "Top update stages (mean ms; MSAA 4x, bloom off where available):", ""]
        for npcs in sorted(by_npcs):
            top = sorted(by_npcs[npcs]["stages"].items(), key=lambda item: -item[1])[:8]
            lines.append(f"- {npcs} NPCs: " + ", ".join(f"{name} {ms:.2f}" for name, ms in top))
    (out / "results.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    print(f"\nSaved {out / 'results.md'} and {out / 'results.csv'}")


if __name__ == "__main__":
    main(pathlib.Path(sys.argv[1]))
