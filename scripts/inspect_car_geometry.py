#!/usr/bin/env python3
"""Read-only topology audit of car.glb using the CSG importer's exact-position weld."""
import collections
import json
from pathlib import Path
import struct


def inspect(path):
    raw = Path(path).read_bytes()
    json_size = struct.unpack_from("<I", raw, 12)[0]
    model = json.loads(raw[20:20 + json_size])
    binary = raw[28 + json_size:]

    def accessor(index):
        item = model["accessors"][index]
        assert "sparse" not in item, "sparse accessors need explicit decoding"
        view = model["bufferViews"][item["bufferView"]]
        assert view.get("buffer", 0) == 0
        size = {"SCALAR": 1, "VEC2": 2, "VEC3": 3}[item["type"]]
        kind = {5126: "f", 5125: "I", 5123: "H", 5121: "B"}[item["componentType"]]
        fmt = "<" + kind * size
        offset = view.get("byteOffset", 0) + item.get("byteOffset", 0)
        stride = view.get("byteStride", struct.calcsize(fmt))
        return [struct.unpack_from(fmt, binary, offset + i * stride)
                for i in range(item["count"])]

    results = []
    for node in model["nodes"]:
        if "mesh" not in node:
            continue
        for primitive in model["meshes"][node["mesh"]]["primitives"]:
            assert primitive.get("mode", 4) == 4, "only triangles are audited"
            positions = accessor(primitive["attributes"]["POSITION"])
            indices = [i[0] for i in accessor(primitive["indices"])]
            canonical = {}
            welded = [canonical.setdefault(p, len(canonical)) for p in positions]
            edges = collections.defaultdict(lambda: [0, 0])
            degenerate = 0
            volume6 = 0.0
            origin = positions[indices[0]]
            for offset in range(0, len(indices), 3):
                tri = indices[offset:offset + 3]
                p = [positions[i] for i in tri]
                a = [p[1][i] - p[0][i] for i in range(3)]
                b = [p[2][i] - p[0][i] for i in range(3)]
                cross = [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
                degenerate += sum(v*v for v in cross) == 0
                volume6 += sum((p[0][i]-origin[i])*cross[i] for i in range(3))
                ids = [welded[i] for i in tri]
                for a, b in zip(ids, ids[1:] + ids[:1]):
                    edge = edges[tuple(sorted((a, b)))]
                    edge[0] += 1
                    edge[1] += 1 if a < b else -1
            results.append(dict(
                node=node.get("name"), vertices=len(positions), triangles=len(indices)//3,
                attributes=sorted(primitive["attributes"]),
                rigid="skin" not in node and not primitive.get("targets"),
                over_source_budget=len(indices)//3 > 16384,
                degenerate_triangles=degenerate,
                boundary_edges=sum(n == 1 for n, _ in edges.values()),
                non_manifold_edges=sum(n > 2 for n, _ in edges.values()),
                inconsistent_edges=sum(n == 2 and d != 0 for n, d in edges.values()),
                signed_volume=volume6/6,
            ))
    return results


if __name__ == "__main__":
    import sys
    path = sys.argv[1] if len(sys.argv) > 1 else Path(__file__).resolve().parents[1] / "assets/models/car.glb"
    print(json.dumps(inspect(path), indent=2))
