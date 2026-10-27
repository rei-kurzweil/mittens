#!/usr/bin/env python3
"""Sequential Sakura stereo comparisons and warmed Nsight captures.
Build with cargo test --release --lib profile_sakura_lumae_stereo --no-run,
then pass the resulting test executable using --binary.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

CASES = {f"{n}-{motion}": (n, motion, False) for n in (0, 1, 2) for motion in ("static", "moving")}
CASES["2-moving-hidden"] = (2, "moving", True)
CAPTURES = ("0-static", "2-static", "0-moving", "2-moving")


def wait_marker(marker, process):
    start = time.monotonic()
    while not marker.exists():
        if process.poll() is not None:
            raise RuntimeError(f"process exited {process.returncode} before {marker}")
        if time.monotonic() - start > 180:
            raise TimeoutError(str(marker))
        time.sleep(0.05)


def run(args, label, repeat, capture=False):
    count, motion, hidden = CASES[label]
    stem = args.output / f"{label}-r{repeat}{'-nsight' if capture else ''}"
    result = stem.with_suffix(".json")
    complete = stem.with_suffix(".complete")
    if complete.exists() and result.exists() and (not capture or (stem.with_suffix(".nsys-rep").exists() and stem.with_suffix(".sqlite").exists())):
        print(f"reuse {result}", flush=True)
        return json.loads(result.read_text())
    env = dict(os.environ, SAKURA_PROFILE_LUMAE=str(count), SAKURA_PROFILE_MOTION=motion,
               SAKURA_PROFILE_HIDE=str(int(hidden)), SAKURA_PROFILE_FRAMES=str(args.frames),
               SAKURA_PROFILE_WARMUP=str(args.warmup), SAKURA_PROFILE_OUTPUT=str(result))
    command = [str(args.binary.resolve()), "profile_sakura_lumae_stereo", "--ignored", "--nocapture"]
    gate = Path(str(stem) + ".gate")
    session = f"sakura-{os.getpid()}-{label}-{repeat}"
    if not capture and repeat == 1 and label in ("0-static", "2-static", "2-moving-hidden"):
        env["SAKURA_PROFILE_PREVIEW"] = str(stem.with_suffix(".png"))
    if capture:
        env["SAKURA_PROFILE_GATE"] = str(gate)
        for extension in ("ready", "go", "done", "exit"):
            gate.with_suffix("." + extension).unlink(missing_ok=True)
        command = [str(args.nsys), "profile", "--trace=vulkan,osrt", "--sample=process-tree",
                   "--cpuctxsw=process-tree", "--gpu-metrics-devices=none", "--start-later=true",
                   f"--session-new={session}", "--kill=none", f"--output={stem}"] + command
    print(f"start {stem.name}", flush=True)
    with stem.with_suffix(".log").open("w") as log:
        process = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)
        try:
            if capture:
                wait_marker(gate.with_suffix(".ready"), process)
                subprocess.run([str(args.nsys), "start", f"--session={session}"], stdout=log, stderr=subprocess.STDOUT, check=True, timeout=60)
                gate.with_suffix(".go").touch()
                wait_marker(gate.with_suffix(".done"), process)
                subprocess.run([str(args.nsys), "stop", f"--session={session}"], stdout=log, stderr=subprocess.STDOUT, check=True, timeout=120)
                gate.with_suffix(".exit").touch()
            status = process.wait(timeout=180)
            if status:
                raise RuntimeError(f"{stem.name} failed ({status}); see {stem.with_suffix('.log')}")
        finally:
            if capture:
                gate.with_suffix(".exit").touch()
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)
    if capture:
        with stem.with_suffix(".export.log").open("w") as log:
            subprocess.run([str(args.nsys), "export", "--type=sqlite", f"--output={stem}.sqlite", f"{stem}.nsys-rep"], stdout=log, stderr=subprocess.STDOUT, check=True, timeout=120)
    value = json.loads(result.read_text())
    complete.touch()
    print(f"done {stem.name}: median frame {value['timing']['total_ms']['median_ms']:.3f} ms, morph upload {value['counters']['deformation_morph_delta_upload_bytes']/args.frames/1e6:.3f} MB/frame", flush=True)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--output", type=Path, default=Path("/tmp") / ("mittens-sakura-gpu-" + datetime.datetime.now().strftime("%Y%m%d-%H%M%S")))
    parser.add_argument("--case", action="append", choices=CASES)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--frames", type=int, default=600)
    parser.add_argument("--warmup", type=int, default=240)
    parser.add_argument("--capture", action="store_true")
    parser.add_argument("--capture-only", action="store_true")
    parser.add_argument("--nsys", type=Path, default=Path.home() / ".local/bin/nsys-2025.3")
    args = parser.parse_args()
    if not args.binary.is_file() or args.repeat < 1 or args.frames < 1 or args.warmup < 0:
        parser.error("provide an existing executable and positive frame/repetition counts")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    with args.binary.open("rb") as stream:
        binary_sha = hashlib.file_digest(stream, "sha256").hexdigest()
    config = {"binary": str(args.binary.resolve()), "binary_sha256": binary_sha,
              "frames": args.frames, "warmup": args.warmup}
    manifest = args.output / "run-config.json"
    if manifest.exists() and json.loads(manifest.read_text()) != config:
        parser.error("output folder was created with a different executable or frame counts; choose a new folder")
    manifest.write_text(json.dumps(config, indent=2))
    results = []
    if not args.capture_only:
        labels = args.case or list(CASES)
        for repeat in range(1, args.repeat + 1):
            for label in (labels if repeat % 2 else list(reversed(labels))):
                results.append(run(args, label, repeat))
    if args.capture or args.capture_only:
        for label in args.case or CAPTURES:
            results.append(run(args, label, 1, capture=True))
    (args.output / "results.json").write_text(json.dumps(results, indent=2))
    print(f"reports: {args.output}", flush=True)


if __name__ == "__main__":
    main()
