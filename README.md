# tetraplot

`tetraplot` is a Rust library for interactive and static scientific visualization in tetrahedral barycentric coordinates. It is a new foundation for quaternary composition diagrams, phase data, finite-element meshes, and embedded ternary charts.

The scientific model is independent from the renderer:

```text
barycentric scientific input -> validation and explicit policies -> domain clipping
-> Cartesian world coordinates -> prepared primitives -> software or three-d rendering
```

## Status

Version `0.1.0` establishes the core coordinate, geometry, preparation, chart, embedding, and rendering boundaries. It supports validated tetrahedral points, segment clipping, point/line/surface preparation, deterministic in-memory/PNG rendering, and a three-d native window for points, tubes, indexed surfaces, and the outer frame.

Planar triangular sections and triangulated curved embeddings are first-class scientific models. Their local ternary points and lines are retained separately from their parent tetrahedral positions. Plane intersections correctly distinguish empty, point, segment, triangle, and quadrilateral cases; only triangular intersections can host a local ternary chart.

The current initial renderer draws ordinary tetrahedral points, lines, surfaces, frames, and basic planar-section fills, boundaries, points, and lines. It does not yet render text, legends, colour bars, contour bands, arbitrary clipped surface triangles, or interactive section controls.

## Quick start

```rust
use tetraplot::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut plot = TetraplotBuilder::new()
        .caption("Quaternary composition")
        .vertex_labels(["A", "B", "C", "D"])
        .build()?;

    plot.configure_frame().faces(false).draw()?;
    plot.draw_series(
        TetraPointSeries::new([[0.25, 0.25, 0.25, 0.25]])
            .color(RED)
            .size(7.0),
    )?;
    plot.save_png("composition.png", (1200, 900))?;
    Ok(())
}
```

`TetraPoint::new` is strict. To pass raw arrays to a series, use the series constructors; they validate every item under the selected `Normalization`, `InvalidPointPolicy`, and `DomainClip` policy during preparation.

## Features

- `window` (default): native interactive `three-d` window adapter.
- `image-export` (default): PNG writing through `image`.
- `serde`: reserved serialization derives are not yet enabled on public model types.

`cargo check --no-default-features` builds the full numerical, chart, and preparation core without graphics dependencies.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.