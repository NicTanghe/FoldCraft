#!/usr/bin/env python3
"""CPU-only scalar design probe, not a cloth solver or performance benchmark.

Dimensionless r=k=1; inertia a varies, target y=-0.5 presses into contact.
Exact scalar primal minimization with a conservative positive-distance path
bound alternates with bracketed slack minimization and an unscaled dual update.
JSON contains failures as well as converged cases. No GPU or third-party packages.
"""
import json
import math


def barrier(distance):
    if distance >= 1:
        return (0.0, 0.0, 0.0)
    s = distance * distance
    u = s - 1
    log = math.log(s)
    first = -(2 * u * log + u * u / s)
    second = -(2 * log + 4 * u / s - u * u / (s * s))
    return (
        -u * u * log / 8,
        first * distance / 4,
        (second * 4 * distance * distance + first * 2) / 8,
    )


def root(function, lower=1e-14, upper=2.0):
    assert function(lower) < 0
    while function(upper) < 0:
        upper *= 2
    for _ in range(100):
        middle = (lower + upper) / 2
        if function(middle) > 0:
            upper = middle
        else:
            lower = middle
    return (lower + upper) / 2


def run(inertia, rho):
    target = -0.5
    distance = 0.9
    slack = distance
    dual = barrier(distance)[1]
    reference = root(lambda x: inertia * (x - target) + barrier(x)[1])
    for iteration in range(1, 10001):
        trial = (inertia * target + rho * slack - dual) / (inertia + rho)
        displacement = trial - distance
        # Bound actual motion; positive slack alone does not ensure separation.
        fraction = (
            min(1.0, 0.8 * (distance - 1e-12) / max(-displacement, 1e-300))
            if displacement < 0 else 1.0
        )
        distance += fraction * displacement
        slack = root(lambda x: barrier(x)[1] - dual + rho * (x - distance))
        dual += rho * (distance - slack)
        primal = abs(distance - slack)
        force = abs(inertia * (distance - target) + barrier(distance)[1])
        converged = primal < 1e-8 and force < 1e-7
        if converged:
            break
    return dict(
        inertia=inertia, rho=rho, iterations=iteration, distance=distance,
        reference=reference, primal=primal, force=force, converged=converged,
    )


if __name__ == "__main__":
    print(json.dumps(
        [run(a, rho) for a in [0.1, 1, 10, 100] for rho in [0.1, 1, 10, 100]],
        indent=2,
    ))
