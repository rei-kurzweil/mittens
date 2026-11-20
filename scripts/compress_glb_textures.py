#!/usr/bin/env python3
"""Bake embedded GLB images into level-zero BC7 and ASTC 4x4 sidecars for Mittens."""
import argparse
import io
import json
import os
from pathlib import Path
import struct
import sys
import tempfile
import zlib

try:
    import ispc_texcomp as codec
    from PIL import Image
except ImportError:
    sys.exit(
        "Set up the encoder from the repository root:\n"
        "  python -m venv tools/texture/venv\n"
        "  tools/texture/venv/bin/python -m pip install Pillow ispc_texcomp==1.0.1\n"
        "Then rerun this script with tools/texture/venv/bin/python."
    )


def embedded_images(path):
    with path.open("rb") as stream:
        magic, version, length = struct.unpack("<4sII", stream.read(12))
        if (magic, version, length) != (b"glTF", 2, path.stat().st_size):
            raise ValueError("invalid GLB 2 header")
        size, kind = struct.unpack("<I4s", stream.read(8))
        if kind != b"JSON" or size > length - 20:
            raise ValueError("invalid JSON chunk")
        doc = json.loads(stream.read(size))
        if not doc.get("images"):
            return
        size, kind = struct.unpack("<I4s", stream.read(8))
        start = stream.tell()
        if kind != b"BIN\0" or start + size > length:
            raise ValueError("missing or truncated BIN chunk")
        for index, image in enumerate(doc["images"]):
            if image.get("mimeType") not in ("image/jpeg", "image/png") or "bufferView" not in image:
                raise ValueError(f"image {index} is not an embedded PNG or JPEG")
            view = doc["bufferViews"][image["bufferView"]]
            offset, count = view.get("byteOffset", 0), view["byteLength"]
            if view["buffer"] != 0 or min(offset, count) < 0 or offset + count > size:
                raise ValueError(f"invalid buffer view for image {index}")
            stream.seek(start + offset)
            yield index, stream.read(count)


def padded_surface(image):
    width, height = image.size
    # The block encoders expect complete tiles. Extend border texels while
    # keeping the authored dimensions in the container and the engine.
    pw, ph = (width + 3) & ~3, (height + 3) & ~3
    if (pw, ph) != image.size:
        padded = Image.new("RGBA", (pw, ph))
        padded.paste(image)
        if pw > width:
            edge = image.crop((width - 1, 0, width, height))
            padded.paste(edge.resize((pw - width, height)), (width, 0))
        if ph > height:
            edge = padded.crop((0, height - 1, pw, height))
            padded.paste(edge.resize((pw, ph - height)), (0, height))
        image = padded
    return codec.RGBASurface(image.tobytes(), pw, ph)


def encode(surface, width, height, fmt, alpha):
    if fmt == "bc7":
        settings = codec.BC7EncSettings.from_profile("alpha_basic" if alpha else "basic")
        blocks = codec.compress_blocks_bc7(surface, settings)
        # DDS_HEADER, DDS_PIXELFORMAT (DX10 FourCC), then DDS_HEADER_DXT10.
        header = struct.pack("<4s7I44x8I5I5I", b"DDS ", 124, 0x81007, height, width,
                             len(blocks), 0, 1, 32, 4, int.from_bytes(b"DX10", "little"),
                             0, 0, 0, 0, 0, 0x1000, 0, 0, 0, 0, 98, 3, 0, 1, 0)
    else:
        settings = codec.ASTCEncSettings.from_profile("alpha_fast" if alpha else "fast", 4, 4)
        blocks = codec.compress_blocks_astc(surface, settings)
        header = bytes.fromhex("13 ab a1 5c 04 04 01") + b"".join(
            n.to_bytes(3, "little") for n in (width, height, 1))
    if len(blocks) != ((width + 3) // 4) * ((height + 3) // 4) * 16:
        raise ValueError(f"unexpected {fmt} encoder output length")
    return header + blocks


def bake(path, formats):
    directory = path.with_name(path.name + ".textures")
    count = 0
    for index, data in embedded_images(path):
        with Image.open(io.BytesIO(data)) as source:
            image = source.convert("RGBA")
        alpha = image.getextrema()[3][0] < 255
        surface = padded_surface(image)
        directory.mkdir(exist_ok=True)
        for fmt in formats:
            ext = "dds" if fmt == "bc7" else "astc"
            dest = directory / f"{index}-{zlib.crc32(data):08x}.{ext}"
            payload = encode(surface, *image.size, fmt, alpha)
            tmp = tempfile.NamedTemporaryFile(dir=directory, delete=False)
            try:
                with tmp:
                    tmp.write(payload)
                os.replace(tmp.name, dest)
            finally:
                Path(tmp.name).unlink(missing_ok=True)
            print(f"{dest} ({image.width}x{image.height}, {len(payload):,} bytes)")
        count += 1
    if not count:
        print(f"{path}: no embedded images")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("models", nargs="+", type=Path, metavar="MODEL.glb")
    parser.add_argument("--format", choices=("both", "bc7", "astc"), default="both")
    args = parser.parse_args()
    for model in args.models:
        try:
            bake(model, ("bc7", "astc") if args.format == "both" else (args.format,))
        except (OSError, ValueError, KeyError, IndexError, struct.error) as exc:
            parser.exit(1, f"{model}: {exc}\n")
