#!/usr/bin/env python3
"""Optional CPU oracle: read actual USDA with Pixar OpenUSD, never a text parser.

Requires the separately installed `usd-core` Python package; it is not a default
test dependency. --template additionally checks rectangular arrangement geometry
against the original JSON and an independent analytical placement calculation.
"""
import argparse
import json
import math
from pathlib import Path


def require(condition, message):
    if not condition:
        raise ValueError(message)


def placed(uv, placement):
    # Apply Rz, then Ry, then Rx to column-vector (u, -v, 0).
    x, y, z = uv[0], -uv[1], 0.0
    rx, ry, rz = map(math.radians, placement["rotation_degrees"])
    x, y = math.cos(rz) * x - math.sin(rz) * y, math.sin(rz) * x + math.cos(rz) * y
    x, z = math.cos(ry) * x + math.sin(ry) * z, -math.sin(ry) * x + math.cos(ry) * z
    y, z = math.cos(rx) * y - math.sin(rx) * z, math.sin(rx) * y + math.cos(rx) * z
    return [a + b for a, b in zip((x, y, z), placement["translation"])]


def validate(scene_path, template_path=None):
    from pxr import Sdf, Usd, UsdGeom

    stage = Usd.Stage.Open(str(scene_path))
    require(stage is not None, "OpenUSD could not open stage")
    require(stage.GetDefaultPrim().GetPath() == "/World", "defaultPrim must be /World")
    require(UsdGeom.GetStageMetersPerUnit(stage) == 1.0, "stage must explicitly use metres")
    require(stage.HasAuthoredMetadata("metersPerUnit"), "stage metric must be authored")
    require(UsdGeom.GetStageUpAxis(stage) == "Y", "stage must be Y-up")
    require(stage.HasAuthoredMetadata("upAxis"), "stage up-axis must be authored")
    group = stage.GetPrimAtPath("/World/Garment")
    require(bool(group), "missing /World/Garment scope")
    require(group.GetAttribute("clothcraft:snapshotVersion").Get() == 1, "unknown snapshot version")
    snapshot_time = group.GetAttribute("clothcraft:timeSeconds").Get()
    require(isinstance(snapshot_time, (int, float)) and math.isfinite(snapshot_time), "missing or invalid snapshot time")
    source = None
    if template_path:
        source = json.loads(Path(template_path).read_text(encoding="utf-8"))
        require(all(p.get("polygon") is None for p in source["panels"]), "template oracle supports rectangular panels only")
        require(not source.get("pin_targets") and not source.get("frozen_panels"), "template oracle excludes authored world pin targets and frozen panels")
        source = {p["id"]: p for p in source["panels"]}
    seen = set()
    vertices = faces = 0
    for prim in stage.Traverse():
        if not prim.IsA(UsdGeom.Mesh):
            continue
        mesh = UsdGeom.Mesh(prim)
        ident = prim.GetAttribute("clothcraft:panelId").Get()
        require(isinstance(ident, str) and ident and ident not in seen, "missing or duplicate panel ID")
        seen.add(ident)
        require(str(prim.GetPath()) == "/World/Garment/panel_" + ident.encode("utf-8").hex(), "unstable semantic prim path")
        points = mesh.GetPointsAttr().Get()
        counts = mesh.GetFaceVertexCountsAttr().Get()
        indices = mesh.GetFaceVertexIndicesAttr().Get()
        require(points and counts and all(c == 3 for c in counts), "expected nonempty triangles")
        valid, reason = UsdGeom.Mesh.ValidateTopology(indices, counts, len(points))
        require(valid, f"invalid topology: {reason}")
        require(all(math.isfinite(v) for p in points for v in p), "nonfinite point")
        require(mesh.GetSubdivisionSchemeAttr().Get() == "none", "polygon mesh must disable subdivision")
        require(mesh.GetOrientationAttr().Get() == "rightHanded", "unexpected orientation")
        require(mesh.GetDoubleSidedAttr().Get(), "cloth must be two-sided")
        attr = prim.GetAttribute("clothcraft:restUV")
        require(attr.IsCustom() and attr.GetTypeName() == Sdf.ValueTypeNames.Double2Array, "rest UV must be custom double2[]")
        rest = attr.Get()
        require(rest is not None and len(rest) == len(points) and all(math.isfinite(v) for uv in rest for v in uv), "invalid rest UV")
        st = UsdGeom.PrimvarsAPI(prim).GetPrimvar("st")
        require(st.GetTypeName() == Sdf.ValueTypeNames.TexCoord2fArray and st.GetInterpolation() == "vertex", "invalid st primvar")
        require(len(st.Get()) == len(points), "st must match point count")
        for expected, actual in zip(rest, st.Get()):
            require(all(math.isclose(a, b, rel_tol=1e-6, abs_tol=1e-7) for a, b in zip(expected, actual)), "render UV differs from material metres")
        if source is not None:
            require(ident in source, f"unknown panel {ident}")
            panel = source[ident]
            pat = panel["pattern"]
            require(len(points) == (pat["nx"] + 1) * (pat["ny"] + 1), "point count differs from source")
            require(len(counts) == 2 * pat["nx"] * pat["ny"], "face count differs from source")
            expected_uv = [(pat["width"] * i / pat["nx"], pat["height"] * j / pat["ny"]) for j in range(pat["ny"] + 1) for i in range(pat["nx"] + 1)]
            for uv, point, actual_uv in zip(expected_uv, points, rest):
                require(all(math.isclose(a, b, rel_tol=1e-14, abs_tol=1e-14) for a, b in zip(uv, actual_uv)), "f64 rest coordinates differ from template")
                require(all(math.isclose(a, b, rel_tol=1e-6, abs_tol=2e-7) for a, b in zip(placed(uv, panel["placement"]), point)), "world positions differ from source placement")
        vertices += len(points)
        faces += len(counts)
    require(seen, "no meshes")
    if source is not None:
        require(seen == set(source), "missing source panels")
    return {"scene": str(scene_path), "panels": len(seen), "points": vertices, "triangles": faces, "oracle": "Pixar OpenUSD", "source_geometry_checked": source is not None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("scene", type=Path)
    parser.add_argument("--template", type=Path)
    args = parser.parse_args()
    try:
        print(json.dumps(validate(args.scene, args.template), indent=2))
    except ImportError as exc:
        parser.exit(2, f"Optional Pixar OpenUSD dependency unavailable: {exc}. Install usd-core in an isolated environment.\n")
    except (ValueError, RuntimeError, TypeError, AttributeError, KeyError, OSError) as exc:
        parser.exit(1, f"USD validation failed: {exc}\n")


if __name__ == "__main__":
    main()
