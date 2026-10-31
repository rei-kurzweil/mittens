#!/usr/bin/env python3
"""Rebuild the smoke section's static, unit-width rounded-box GLB (stdlib only)."""

import json
import math
from pathlib import Path
import struct


def build():
    vertices, indices = [], []
    grid = [-0.5, -0.435, -0.28, 0.28, 0.435, 0.5]
    # Face tangent pairs have cross(u, v) == outward normal.
    faces = [
        ((1, 0, 0), (0, 1, 0), (0, 0, 1)),
        ((-1, 0, 0), (0, -1, 0), (0, 0, 1)),
        ((0, 1, 0), (0, 0, 1), (1, 0, 0)),
        ((0, -1, 0), (0, 0, -1), (1, 0, 0)),
        ((0, 0, 1), (1, 0, 0), (0, 1, 0)),
        ((0, 0, -1), (-1, 0, 0), (0, 1, 0)),
    ]
    for normal, tangent_u, tangent_v in faces:
        first = len(vertices)
        for v in grid:
            for u in grid:
                cube = [normal[i] * 0.5 + tangent_u[i] * u + tangent_v[i] * v for i in range(3)]
                core = [max(-0.28, min(0.28, p)) for p in cube]
                delta = [cube[i] - core[i] for i in range(3)]
                length = math.sqrt(sum(d * d for d in delta))
                n = [d / length for d in delta]
                p = [core[i] + n[i] * 0.22 for i in range(3)]
                vertices.append((*p, *n, u + 0.5, v + 0.5))
        for row in range(5):
            for column in range(5):
                a = first + row * 6 + column
                indices.extend((a, a + 1, a + 7, a, a + 7, a + 6))
    vertex_bytes = b"".join(struct.pack("<8f", *v) for v in vertices)
    index_bytes = struct.pack(f"<{len(indices)}H", *indices)
    binary = vertex_bytes + index_bytes
    binary += b"\0" * (-len(binary) % 4)
    document = {
        "asset": {"version": "2.0", "generator": "generate-smoke-rounded-box.py"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": "smoke_rounded_box", "mesh": 0}],
        "meshes": [{"name": "rounded_box", "primitives": [{
            "attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2},
            "indices": 3, "material": 0,
        }]}],
        # Dormant sections remain invisible even before the first script update.
        "materials": [{"alphaMode": "BLEND", "pbrMetallicRoughness": {
            "baseColorFactor": [1, 1, 1, 0], "metallicFactor": 0, "roughnessFactor": 1,
        }}],
        "buffers": [{"byteLength": len(binary)}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": len(vertex_bytes), "byteStride": 32, "target": 34962},
            {"buffer": 0, "byteOffset": len(vertex_bytes), "byteLength": len(index_bytes), "target": 34963},
        ],
        "accessors": [
            {"bufferView": 0, "byteOffset": 0, "componentType": 5126, "count": len(vertices), "type": "VEC3", "min": [-0.5] * 3, "max": [0.5] * 3},
            {"bufferView": 0, "byteOffset": 12, "componentType": 5126, "count": len(vertices), "type": "VEC3"},
            {"bufferView": 0, "byteOffset": 24, "componentType": 5126, "count": len(vertices), "type": "VEC2"},
            {"bufferView": 1, "componentType": 5123, "count": len(indices), "type": "SCALAR"},
        ],
    }
    encoded = json.dumps(document, separators=(",", ":")).encode()
    encoded += b" " * (-len(encoded) % 4)
    length = 12 + 8 + len(encoded) + 8 + len(binary)
    glb = struct.pack("<III", 0x46546C67, 2, length)
    glb += struct.pack("<II", len(encoded), 0x4E4F534A) + encoded
    glb += struct.pack("<II", len(binary), 0x004E4942) + binary
    path = Path(__file__).with_name("smoke-rounded-box.glb")
    path.write_bytes(glb)
    print(f"{path}: {len(vertices)} vertices, {len(indices) // 3} triangles, {len(glb)} bytes")


if __name__ == "__main__":
    build()
