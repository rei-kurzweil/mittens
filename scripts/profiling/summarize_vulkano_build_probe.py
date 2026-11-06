#!/usr/bin/env python3
"""Reconcile a temporary Vulkano build probe capture with ship frame samples."""
import argparse
import json
import math
from pathlib import Path


def summary(values):
    ordered = sorted(values)
    return {
        "mean_ms": sum(values) / len(values),
        "median_ms": ordered[len(ordered) // 2],
        "p95_ms": ordered[math.ceil((len(ordered) - 1) * 0.95)],
    }


def analyze(log, frames):
    records = []
    sampling = False
    for line in log.splitlines():
        if line.startswith("SHIP_PROFILE_BEGIN "):
            sampling = True
        if sampling and line.startswith("VULKANO_BUILD_PROBE "):
            records.append(json.loads(line.split(" ", 1)[1]))
        if line.startswith("SHIP_PROFILE_RESULT "):
            sampling = False
    if len(records) != 2 * len(frames):
        raise ValueError(f"Expected {2 * len(frames)} eye builds, got {len(records)}")
    if any(record["level"] != "Primary" for record in records):
        raise ValueError("Unexpected secondary command builds in stereo fixture")
    keys = sorted(key for key in records[0] if key.endswith("_ns"))
    per_frame = []
    for index, frame in enumerate(frames):
        eyes = records[2 * index:2 * index + 2]
        phases = {key[:-3] + "_ms": sum(eye[key] for eye in eyes) / 1e6 for key in keys}
        total = sum(phases.values())
        outer = frame["cpu_record_stages_ms"]["command_buffer_build"]
        per_frame.append({
            "frame": frame["frame"],
            "commands": sum(eye["commands"] for eye in eyes),
            "resource_uses": sum(eye["resource_uses"] for eye in eyes),
            "phases_ms": phases,
            "inner_total_ms": total,
            "outer_build_ms": outer,
            "wrapper_logging_overhead_ms": outer - total,
        })
    result = {
        "stereo_frames": len(per_frame),
        "eye_builds": len(records),
        "commands_per_stereo_frame": sorted(set(frame["commands"] for frame in per_frame)),
        "resource_uses_per_stereo_frame": sorted(set(frame["resource_uses"] for frame in per_frame)),
        "phases_ms": {key: summary([f["phases_ms"][key] for f in per_frame])
                      for key in per_frame[0]["phases_ms"]},
    }
    for key in ["inner_total_ms", "outer_build_ms", "wrapper_logging_overhead_ms"]:
        result[key] = summary([frame[key] for frame in per_frame])
    return result, per_frame


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    parser.add_argument("frames", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    result, frames = analyze(args.log.read_text(), json.loads(args.frames.read_text()))
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    args.output.with_suffix(".frames.json").write_text(json.dumps(frames) + "\n")
    print(json.dumps(result, indent=2))
