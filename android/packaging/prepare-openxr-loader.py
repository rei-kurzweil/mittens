#!/usr/bin/env python3
"""Fetch the pinned Khronos loader used by the local ALVR client."""
from hashlib import sha256
from pathlib import Path
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[2]
VERSION = "1.1.36"
AAR_SHA = "7bb8ef37f7fe138233998c64529e0808ee33f588986a6e54a9015abc40b44be7"
LIB_SHA = "6b9aeb6ea2e429bae91328cc85b0e19947cc77ca2f4948a763a4a41c23511dd6"
destination = ROOT / "target/android-runtime-libs/arm64-v8a/libopenxr_loader.so"
if destination.exists() and sha256(destination.read_bytes()).hexdigest() == LIB_SHA:
    raise SystemExit(0)
archive = ROOT / f"target/android-runtime-libs/openxr-{VERSION}.aar"
archive.parent.mkdir(parents=True, exist_ok=True)
if not archive.exists() or sha256(archive.read_bytes()).hexdigest() != AAR_SHA:
    subprocess.run([
        "curl", "--silent", "--show-error", "--fail", "--location",
        f"https://github.com/KhronosGroup/OpenXR-SDK-Source/releases/download/release-{VERSION}/openxr_loader_for_android-{VERSION}.aar",
        "--output", str(archive),
    ], check=True)
if sha256(archive.read_bytes()).hexdigest() != AAR_SHA:
    raise ValueError("OpenXR archive checksum mismatch")
with zipfile.ZipFile(archive) as package:
    library = package.read("prefab/modules/openxr_loader/libs/android.arm64-v8a/libopenxr_loader.so")
if sha256(library).hexdigest() != LIB_SHA:
    raise ValueError("OpenXR loader checksum mismatch")
destination.parent.mkdir(parents=True, exist_ok=True)
destination.write_bytes(library)
print(f"Prepared OpenXR loader {VERSION}: {destination}")
