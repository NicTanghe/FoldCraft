# Third-party material

## MakeHuman assets

`assets/makehuman/base.obj` comes from the MakeHuman community repository at revision `a8bc2d54ff0ac92e78ff71431b1023eda42bf482`. The base mesh is CC0, as distinguished from program code in the [upstream notice](https://github.com/makehumancommunity/makehuman/blob/a8bc2d54ff0ac92e78ff71431b1023eda42bf482/LICENSE.md).

The asset notice is retained at [LICENSE.ASSETS.md](assets/makehuman/LICENSE.ASSETS.md), with [provenance](assets/makehuman/provenance.json). `scripts/prepare-avatar.py` extracts the body, removes helpers, triangulates and normalizes to 1.75 m; `mannequin.obj` and `measurements.json` are derived outputs. No MakeHuman program code is incorporated.

## Dependencies and references

Rust dependencies are resolved by `Cargo.lock` and retain their respective upstream licenses. They are not vendored here. Consult each dependency's manifest/notices when preparing a binary distribution.

Research and public manual references are recorded in `docs/research.md`, `docs/research-patterns.md`, `docs/ogc.md` and `docs/manual-study.md`. Citations do not imply that upstream implementations, assets or trademarks are included or licensed by this project.

## ClothCraft source

ClothCraft source is distributed under the MIT license; see [LICENSE](LICENSE).
The MakeHuman CC0 notice applies to the bundled assets, not the program source.

## Optional Box3D research harness

`experiments/box3d` contains ClothCraft's comparison harness. It requires a
separate checkout of Box3D and does not vendor that project's source. Box3D
retains its own MIT license; keep its notice when distributing a combined
build. The pinned source and experimental limitations are recorded in
[the solver laboratory](docs/solver-lab.md).

## Stable Cosserat Rods CPU implementation

`src/cosserat.rs` adapts the CPU position and orientation updates from Jerry
Hsu's [StableCosseratRods](https://github.com/jerry060599/StableCosseratRods),
revision `11081ed22ceb3f8824f447b64557745895676d0a`, particularly
`KittenEngine/Cosserat/sim/lambda.cpp`, `vbd.cpp`, `iterate.cpp` and
`initialization/init.cpp`. The original code uses the MIT license below. The
separate YarnBall GPU repository is GPL-licensed and is not incorporated.

Copyright 2025 Jerry Hsu

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the “Software”), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
