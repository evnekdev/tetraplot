# tetraplot

`tetraplot` is a Rust library for static and interactive scientific visualization in tetrahedral barycentric coordinates. It keeps scientific coordinates, validation, topology, geometry preparation, and rendering separate.

```text
local ternary diagram -> validation and local clipping -> embedding-aware splitting
-> tetrahedral coordinates -> Cartesian world geometry -> software or three-d rendering
```

## Current functional slice

The crate now renders a ternary diagram embedded on a piecewise-planar triangular surface inside a tetrahedron. A `TriangulatedEmbedding` owns validated local and tetrahedral mesh vertices, patches, mesh adjacency, and declared break lines. `TernaryDiagram` owns local points and lines; `EmbeddedTernaryChart` pairs it with the embedding. The cached `PreparedEmbeddedChart` is shared by the deterministic software renderer and the optional native `three-d` adapter.

Planar triangular sections remain a convenience plane/intersection API, but are converted through the same prepared embedded-chart representation before rendering. They are therefore the affine special case of the embedded-chart pipeline, not an unrelated renderer path.

The initial slice supports supporting-surface triangles, patch-aware normals, explicit break overlays, mapped points, break-aware mapped lines, local ternary grids, PNG output, and a compiled native window backend. See [`examples/piecewise_curved_chart.rs`](examples/piecewise_curved_chart.rs). It deliberately does not yet implement contours, filled curved polygons, arbitrary smooth parametric surfaces, picking, or interactive topology editing.

## Quick start

```rust
use tetraplot::prelude::*;

let mut plot = TetraplotBuilder::new().build()?;
plot.draw_series(
    TetraPointSeries::new([[0.25, 0.25, 0.25, 0.25]])
        .color(RED)
        .size(7.0),
)?;
plot.save_png("composition.png", (1200, 900))?;
# Ok::<(), tetraplot::TetraplotError>(())
```

`TetraPoint::new` is strict. Series constructors accept raw arrays but validate them under their selected policy before preparation. `TriangulatedEmbedding::new` validates the local domain; use `set_patches` followed by `add_break_line` to create a piecewise surface.

## Features

- `window` (default): native interactive `three-d` adapter.
- `image-export` (default): PNG writing through `image`.

`Cargo.lock` is intentionally committed for this early renderer project so local examples and CI resolve the same graphics/image dependency graph. This may be revisited if the crate becomes a widely consumed library-only dependency.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.