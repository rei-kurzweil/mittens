"""Run: blender --background --factory-startup --python scripts/test_blender_split_vehicle.py"""
from pathlib import Path
import runpy
import unittest

import bmesh
import bpy
from mathutils import Matrix, Vector

split = runpy.run_path(str(Path(__file__).with_name("blender_split_vehicle.py")), run_name="vehicle_split_test_module")


def stats(bm):
    mesh = bpy.data.meshes.new("test_audit")
    try:
        bm.to_mesh(mesh)
        return split["audit"](mesh)
    finally:
        bpy.data.meshes.remove(mesh)


class VehicleSplitTests(unittest.TestCase):
    def test_five_regions_are_closed_and_conserve_volume(self):
        bm = bmesh.new()
        bmesh.ops.create_cube(bm, size=8)
        bm.transform(Matrix.Translation((0, 0, 4)))
        parts = []
        try:
            for center, normal in [((0, 0, 6), (0, 0, 1)), ((0, 0, 2), (0, 0, -1)),
                                   ((-2, 0, 4), (-1, 0, 0)), ((2, 0, 4), (1, 0, 0))]:
                args = (bm, Vector(center), Vector(normal))
                piece, cap = split["clipped"](*args, True, 1e-6, 0)
                rest, retained = split["clipped"](*args, False, 1e-6, 0)
                self.assertEqual(cap["unsealed_cut_edges"], 0)
                self.assertEqual(retained["unsealed_cut_edges"], 0)
                self.assertGreater(cap["cap_triangles"], 0)
                bm.free()
                bm = rest
                parts.append(piece)
            parts.append(bm)
            audits = [stats(part) for part in parts]
            self.assertTrue(all(a["passes_engine_topology_checks"] for a in audits), audits)
            self.assertAlmostEqual(sum(a["signed_volume"] for a in audits), 512, places=5)
        finally:
            for part in parts:
                if part.is_valid:
                    part.free()
            if bm.is_valid:
                bm.free()

    def test_cutter_uses_local_z_despite_negative_scale_and_offset_mesh(self):
        mesh = bpy.data.meshes.new("test_quad")
        mesh.from_pydata([(-1, -1, 0.4), (1, -1, 0.4), (1, 1, 0.4), (-1, 1, 0.4)], [], [(0, 1, 2, 3)])
        cutter = bpy.data.objects.new("test_cutter", mesh)
        cutter.matrix_world = Matrix.Translation((3, 1, 2)) @ Matrix.Rotation(0.7, 4, "Y") @ Matrix.Diagonal((-2, 3, -1, 1))
        try:
            center, normal = split["cutter_plane"](cutter, 1e-6)
            expected = (cutter.matrix_world.to_3x3() @ Vector((0, 0, 1))).normalized()
            self.assertGreater(normal.dot(expected), 0.99999)
            self.assertLess((center - cutter.matrix_world @ Vector((0, 0, 0.4))).length, 1e-6)
            for v in mesh.vertices:
                self.assertLess(abs((cutter.matrix_world @ v.co-center).dot(normal)), 1e-6)
        finally:
            bpy.data.objects.remove(cutter)
            bpy.data.meshes.remove(mesh)

    def test_cap_preserves_a_hole_through_a_hollow_part(self):
        # A closed square tube: section area 4*4 - 2*2 = 12.
        bm = bmesh.new()
        rings = []
        for z, size in [(-2, 2), (-2, 1), (2, 2), (2, 1)]:
            rings.append([bm.verts.new((x*size, y*size, z)) for x, y in [(-1,-1),(1,-1),(1,1),(-1,1)]])
        for a, b in [(0, 2), (1, 3), (0, 1), (2, 3)]:
            for i in range(4):
                k = (i+1) % 4
                bm.faces.new((rings[a][i], rings[a][k], rings[b][k], rings[b][i]))
        bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
        piece = None
        try:
            piece, cap = split["clipped"](bm, Vector((0,0,0)), Vector((0,0,1)), True, 1e-6, 0)
            self.assertEqual(cap["unsealed_cut_edges"], 0)
            audit = stats(piece)
            self.assertTrue(audit["passes_engine_topology_checks"], audit)
            self.assertAlmostEqual(audit["signed_volume"], 24, places=5)
            area = sum(face.calc_area() for face in piece.faces if all(abs(v.co.z)<1e-6 for v in face.verts))
            self.assertAlmostEqual(area, 12, places=5)
        finally:
            bm.free()
            if piece:
                piece.free()

    def test_exact_seam_welding_preserves_uvs_without_closing_nearby_gaps(self):
        bm = bmesh.new()
        uv = bm.loops.layers.uv.new("UVMap")
        for offset, uv_y in [(0, 0.1), (1, 0.9), (1+1e-5, 0.5)]:
            face = bm.faces.new([bm.verts.new((x+offset,y,0)) for x,y in [(0,0),(1,0),(1,1),(0,1)]])
            for loop in face.loops:
                loop[uv].uv = (offset, uv_y)
        try:
            merged = split["weld_exact_seams"](bm)
            self.assertEqual(merged, 2)
            self.assertEqual(len(bm.verts), 10, "nearby vertices must stay distinct")
            shared_uvs = {round(loop[uv].uv.y, 2) for face in bm.faces for loop in face.loops if tuple(loop.vert.co)==(1,0,0)}
            self.assertEqual(shared_uvs, {0.1, 0.9}, "UV discontinuity survives geometry welding")
        finally:
            bm.free()


if __name__ == "__main__":
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(VehicleSplitTests))
    if not result.wasSuccessful():
        raise SystemExit(1)
