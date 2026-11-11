#!/usr/bin/env python3
"""CPU-only oracle for the resident WGSL primitive-sum contact law.

A point crosses the diagonal of two coplanar triangles at a fixed height.
One triangle contains its projection; the other contributes an edge distance.
That second barrier creates a tangential force on a frictionless plane.
No GPU, simulation, external packages or changes to material parameters.
"""
import json
import math

RADIUS = 0.003
STIFFNESS = 100000.0
HEIGHT = 0.001


def barrier(distance):
    if distance >= RADIUS:
        return 0.0, 0.0
    s = (distance / RADIUS) ** 2
    u = s - 1
    scale = STIFFNESS * RADIUS**2 / 8
    first = -(2 * u * math.log(s) + u * u / s)
    return -scale * u * u * math.log(s), scale * first * 2 * distance / RADIUS**2


samples = []
for i in range(161):
    x = -0.004 + 0.008 * i / 160
    edge_distance = math.sqrt(HEIGHT**2 + x*x/2)
    face_energy, _ = barrier(HEIGHT)
    edge_energy, edge_gradient = barrier(edge_distance)
    samples.append(dict(
        x_m=x, gap_m=HEIGHT, energy_j=face_energy+edge_energy,
        tangential_force_n=-edge_gradient*x/(2*edge_distance),
    ))
print(json.dumps(dict(
    scope="Two-triangle plane, actual squared-distance primitive-sum barrier; no friction",
    energy_min_j=min(s["energy_j"] for s in samples),
    energy_max_j=max(s["energy_j"] for s in samples),
    tangential_force_max_n=max(abs(s["tangential_force_n"]) for s in samples),
    samples=samples,
), indent=2))
