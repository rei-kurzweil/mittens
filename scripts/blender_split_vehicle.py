"""Split vehicle_body using cut_ quads; run with Blender's Python interpreter.

Headless:
  blender --background --disable-autoexec car-8.blend --python \
    scripts/blender_split_vehicle.py -- --output /tmp/car-8-split.blend --export-glb

In Blender's Text Editor, Run Script uses the current file and writes a sibling
<name>-split.blend. The original file and source mesh data are preserved.
The positive local Z side of each cutter is extracted, in --cuts order.
"""
import argparse
import collections
import json
import math
from pathlib import Path
import sys

import bmesh
import bpy
from mathutils import Vector


DEFAULT_CUTS = ("cut_roof", "cut_legs", "cut_wing_left", "cut_wing_right")
COLORS = {
    "roof": (0.95, 0.59, 0.13, 1),
    "legs": (0.39, 0.75, 0.29, 1),
    "wing_left": (0.83, 0.27, 0.53, 1),
    "wing_right": (0.16, 0.62, 0.82, 1),
    "center": (0.50, 0.41, 0.83, 1),
}


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", default="vehicle_body")
    parser.add_argument("--cuts", nargs="+", default=list(DEFAULT_CUTS))
    parser.add_argument("--output", type=Path)
    parser.add_argument("--export-glb", action="store_true")
    parser.add_argument("--epsilon", type=float, default=1e-6,
                        help="World-space classification tolerance; not a vertex merge distance")
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else [])
    if not args.output:
        if not bpy.data.filepath:
            parser.error("Save the source file or supply --output")
        original = Path(bpy.data.filepath)
        args.output = original.with_name(original.stem + "-split.blend")
    args.output = args.output.expanduser().resolve()
    if args.output.suffix != ".blend":
        parser.error("--output must end in .blend")
    if bpy.data.filepath and args.output == Path(bpy.data.filepath).resolve():
        parser.error("The output must differ from the open source .blend")
    outputs = [args.output, args.output.with_suffix(".json")]
    if args.export_glb:
        outputs.append(args.output.with_suffix(".glb"))
    for output in outputs:
        if output.exists():
            parser.error(f"Output already exists: {output}; choose a fresh output path")
    if not math.isfinite(args.epsilon) or args.epsilon <= 0:
        parser.error("--epsilon must be finite and positive")
    if len(set(args.cuts)) != len(args.cuts):
        parser.error("Cutter names must be unique")
    return args


def cutter_plane(obj, epsilon):
    if obj.type != "MESH" or len(obj.data.vertices) != 4 or len(obj.data.polygons) != 1:
        raise ValueError(f"{obj.name}: expected exactly one quad")
    if len(obj.data.polygons[0].vertices) != 4 or obj.modifiers:
        raise ValueError(f"{obj.name}: expected an unmodified quad")
    matrix = obj.matrix_world
    if abs(matrix.to_3x3().determinant()) < 1e-12:
        raise ValueError(f"{obj.name}: singular transform")
    points = [matrix @ v.co for v in obj.data.vertices]
    center = sum(points, Vector()) / 4
    # Plane normal uses inverse transpose, rather than vertex winding. Its
    # positive side agrees with the displayed local +Z even with negative scale.
    normal = (matrix.to_3x3().inverted().transposed() @ Vector((0, 0, 1))).normalized()
    if max(abs((p - center).dot(normal)) for p in points) > epsilon:
        raise ValueError(f"{obj.name}: quad must lie in the object's local XY plane")
    if obj.data.polygons[0].area <= 1e-12:
        raise ValueError(f"{obj.name}: zero-area quad")
    return center, normal


def source_bmesh(obj):
    if obj.type != "MESH" or obj.modifiers or obj.parent_type == "BONE":
        raise ValueError("Source must be a rigid mesh without unapplied modifiers")
    if obj.data.shape_keys:
        raise ValueError("Shape keys need an explicit deformation policy before splitting")
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bm.transform(obj.matrix_world)
    if obj.matrix_world.to_3x3().determinant() < 0:
        bmesh.ops.reverse_faces(bm, faces=list(bm.faces))
    bm.normal_update()
    return bm


def weld_exact_seams(bm):
    # UVs are per face loop in BMesh, so exact geometry welding retains UV seams.
    # No tolerance weld: nearby authored points and real gaps remain distinct.
    canonical = {}
    duplicates = {}
    for vertex in bm.verts:
        key = tuple(vertex.co)
        if key in canonical:
            duplicates[vertex] = canonical[key]
        else:
            canonical[key] = vertex
    if duplicates:
        bmesh.ops.weld_verts(bm, targetmap=duplicates)
    return len(duplicates)


def clipped(source, center, normal, positive, epsilon, cap_material):
    bm = source.copy()
    cap_normal = -normal if positive else normal
    result = bmesh.ops.bisect_plane(
        bm, geom=list(bm.verts) + list(bm.edges) + list(bm.faces),
        plane_co=center, plane_no=normal, dist=epsilon,
        clear_inner=positive, clear_outer=not positive,
    )
    # Fill only boundary edges on this cut, never unrelated pre-existing holes.
    edges = [edge for edge in result["geom_cut"]
             if isinstance(edge, bmesh.types.BMEdge) and edge.is_valid and edge.is_boundary]
    before = len(edges)
    filled = bmesh.ops.triangle_fill(bm, edges=edges, normal=cap_normal, use_beauty=True) if edges else {"geom": []}
    faces = [face for face in filled["geom"] if isinstance(face, bmesh.types.BMFace)]
    bm.normal_update()
    tangent = normal.cross(Vector((0, 0, 1)))
    if tangent.length < 0.1:
        tangent = normal.cross(Vector((0, 1, 0)))
    tangent.normalize()
    bitangent = normal.cross(tangent).normalized()
    uv = bm.loops.layers.uv.active
    for face in faces:
        if face.normal.dot(cap_normal) < 0:
            face.normal_flip()
        face.material_index = cap_material
        face.smooth = False
        if uv:
            for loop in face.loops:
                offset = loop.vert.co - center
                loop[uv].uv = (offset.dot(tangent), offset.dot(bitangent))
    bm.normal_update()
    remaining = sum(edge.is_boundary for edge in edges if edge.is_valid)
    stats = dict(cut_boundary_edges=before, cap_triangles=len(faces),
                 unsealed_cut_edges=remaining)
    if not bm.faces:
        bm.free()
        raise ValueError("Cut produced an empty half")
    return bm, stats


def audit(mesh):
    """Mirror the engine's exact-position topology checks after triangulation."""
    mesh.calc_loop_triangles()
    positions = [tuple(v.co) for v in mesh.vertices]
    canonical = {}
    welded = [canonical.setdefault(p, len(canonical)) for p in positions]
    edges = collections.defaultdict(lambda: [0, 0])
    volume6 = 0.0
    degenerate = 0
    origin = positions[mesh.loop_triangles[0].vertices[0]]
    for tri in mesh.loop_triangles:
        p = [positions[i] for i in tri.vertices]
        a = [p[1][i] - p[0][i] for i in range(3)]
        b = [p[2][i] - p[0][i] for i in range(3)]
        cross = (a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0])
        degenerate += sum(v*v for v in cross) == 0
        volume6 += sum((p[0][i] - origin[i])*cross[i] for i in range(3))
        ids = [welded[i] for i in tri.vertices]
        for a, b in zip(ids, ids[1:] + ids[:1]):
            edge = edges[tuple(sorted((a, b)))]
            edge[0] += 1
            edge[1] += 1 if a < b else -1
    boundary = sum(n == 1 for n, _ in edges.values())
    non_manifold = sum(n > 2 for n, _ in edges.values())
    winding = sum(n == 2 and d != 0 for n, d in edges.values())
    triangles = len(mesh.loop_triangles)
    volume = volume6 / 6
    return dict(vertices=len(positions), triangles=triangles,
                degenerate_triangles=degenerate, boundary_edges=boundary,
                non_manifold_edges=non_manifold, inconsistent_edges=winding,
                signed_volume=volume,
                passes_engine_topology_checks=(triangles <= 16384 and not
                    (degenerate or boundary or non_manifold or winding) and volume > 0))


def main():
    args = arguments()
    if bpy.context.object and bpy.context.object.mode != "OBJECT":
        bpy.ops.object.mode_set(mode="OBJECT")
    source = bpy.data.objects.get(args.source)
    if not source:
        raise ValueError(f"Missing source object: {args.source}")
    if bpy.data.collections.get("VEHICLE_PARTS"):
        raise ValueError("VEHICLE_PARTS already exists; run from the original source file")
    cutters = []
    for name in args.cuts:
        obj = bpy.data.objects.get(name)
        if not obj or not name.startswith("cut_"):
            raise ValueError(f"Missing cut_ quad: {name}")
        center, normal = cutter_plane(obj, args.epsilon)
        cutters.append((obj, center, normal))
    unknown = [o.name for o in bpy.data.objects if o.name.startswith("cut_") and o.name not in args.cuts]
    if unknown:
        raise ValueError(f"Unconfigured cutters: {unknown}; specify --cuts in extraction order")
    collection = bpy.data.collections.new("VEHICLE_PARTS")
    bpy.context.scene.collection.children.link(collection)
    # A separate cap material would become a separate OPEN glTF primitive.
    # Keep cap triangles in the source's material/primitive instead.
    materials = list(source.data.materials)
    if len(materials) > 1:
        raise ValueError("Source uses multiple materials; partitioning per closed primitive needs an explicit material policy")
    bm = source_bmesh(source)
    source_stats = audit(source.data)
    welded_vertices = weld_exact_seams(bm)
    pieces = []
    cut_stats = []
    try:
        for cutter, center, normal in cutters:
            distances = [(v.co-center).dot(normal) for v in bm.verts]
            if min(distances) >= -args.epsilon or max(distances) <= args.epsilon:
                raise ValueError(f"{cutter.name}: does not divide the remaining source")
            piece, extracted_stats = clipped(bm, center, normal, True, args.epsilon, 0)
            remainder, retained_stats = clipped(bm, center, normal, False, args.epsilon, 0)
            bm.free()
            bm = remainder
            pieces.append((cutter.name.removeprefix("cut_"), piece))
            cut_stats.append(dict(cutter=cutter.name, point=list(center), local_z_direction=list(normal),
                                  extracted=extracted_stats, retained=retained_stats))
        pieces.append(("center", bm))
        bm = None
        report = dict(source_file=bpy.data.filepath, source_object=source.name,
                      output_file=str(args.output), epsilon=args.epsilon,
                      exact_seam_vertices_welded=welded_vertices,
                      source_topology=source_stats, cuts=cut_stats, parts=[])
        for name, mesh_bm in pieces:
            mesh = bpy.data.meshes.new(args.source + "_" + name)
            mesh_bm.to_mesh(mesh)
            mesh_bm.free()
            for material in materials:
                mesh.materials.append(material)
            mesh.update()
            obj = bpy.data.objects.new(mesh.name, mesh)
            collection.objects.link(obj)
            obj.color = COLORS.get(name, (0.6, 0.6, 0.6, 1))
            obj["vehicle_split_source"] = source.name
            obj["vehicle_split_region"] = name
            stats = audit(mesh)
            obj["passes_engine_topology_checks"] = stats["passes_engine_topology_checks"]
            report["parts"].append(dict(name=obj.name, **stats))
        report["warnings"] = [
            f"{cut['cutter']}: {cut[side]['unsealed_cut_edges']} cut edges remain unsealed on {side} side; inspect source boundaries/junctions"
            for cut in cut_stats for side in ("extracted", "retained")
            if cut[side]["unsealed_cut_edges"]
        ]
        for part in report["parts"]:
            if not part["passes_engine_topology_checks"]:
                report["warnings"].append(f"{part['name']}: fails engine topology checks; not ready for destruction")
        source.hide_set(True)
        source.hide_render = True
        for cutter, _, _ in cutters:
            cutter.hide_set(True)
            cutter.hide_render = True
        bpy.ops.object.select_all(action="DESELECT")
        for obj in collection.objects:
            obj.select_set(True)
        bpy.context.view_layer.objects.active = collection.objects[0]
        for screen in bpy.data.screens:
            for area in screen.areas:
                if area.type == "VIEW_3D":
                    area.spaces.active.shading.color_type = "OBJECT"
        args.output.parent.mkdir(parents=True, exist_ok=True)
        if args.export_glb:
            # Export generated parts plus original accessories. Exclude the
            # hidden source and cutters, even if they were originally selected.
            for obj in bpy.context.scene.objects:
                obj.select_set(obj.type == "MESH" and obj != source and not obj.name.startswith("cut_") and not obj.hide_get())
            bpy.ops.export_scene.gltf(filepath=str(args.output.with_suffix(".glb")),
                                      export_format="GLB", use_selection=True)
        bpy.ops.file.make_paths_absolute()
        bpy.ops.wm.save_as_mainfile(filepath=str(args.output), check_existing=False)
        args.output.with_suffix(".json").write_text(json.dumps(report, indent=2) + "\n")
        for warning in report["warnings"]:
            print("WARNING: " + warning)
        passed = sum(p["passes_engine_topology_checks"] for p in report["parts"])
        print(f"Split complete: {len(pieces)} pieces; {passed} pass engine topology checks")
        print("VEHICLE_SPLIT_REPORT " + json.dumps(report))
    finally:
        if bm is not None and bm.is_valid:
            bm.free()
        for _, piece in pieces:
            if piece.is_valid:
                piece.free()


if __name__ == "__main__":
    main()
