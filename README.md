# tetraplot

`tetraplot` is a Rust library for scientific visualization and editing in tetrahedral barycentric coordinates. It keeps scientific coordinates, validation, topology, prepared geometry, document state, and native rendering separate.

```text
local ternary diagram -> validation and local clipping -> embedding-aware splitting
-> tetrahedral coordinates -> Cartesian world geometry -> software / three-d rendering

composition grid + scalar fields -> document -> table / TSV / linked prepared grid points
```

## Current functional slice

The library renders local ternary diagrams on planar triangular sections and piecewise-planar curved surfaces inside a tetrahedron. `TriangulatedEmbedding` owns validated mesh topology, patches, adjacency, and authoritative break lines. `TernaryDiagram` remains the single local dataset used by both embedded 3D geometry and the optional flat `plotters-ternary` adapter.

The optional editor adds a fixed scientific application layout around `TetraplotDocument`: scene tree, native orbit viewport, properties, status, one cached flat ternary view, spreadsheet-style grid data, TSV helpers, selection, and scientific prepared-surface ray picking. `plot.show()` remains the deliberately lightweight native viewer.

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

For a full editor scene, run:

```text
cargo run --features editor --example editor
```

The headless composition workflows are demonstrated by `regular_grid_data` and `irregular_grid_data`.

## Features

- `window` (default): lightweight native `three-d` viewer.
- `image-export` (default): software-rendered 3D PNG writing.
- `flat-view`: `plotters` and `plotters-ternary` local chart rendering and flat PNG export.
- `editor`: the native scientific editor, three-d's version-matched egui layer, flat view, and OS clipboard support.

`plotters-ternary` is optional: Plotters backend types do not appear in the scientific diagram, embedding, topology, or prepared 3D geometry APIs. The current editor uses three-d's built-in egui integration rather than a separate `egui-winit` stack because its winit generation is compatible with the renderer.

`Cargo.lock` is intentionally committed for reproducible renderer/editor examples. The package MSRV is 1.92 because the optional current egui integration requires it.

## Current limitations

The editor has one docked flat view and a fixed panel layout. It does not yet offer detached views, full rectangular table keyboard interaction, filled curved regions, contours, colour maps, topology editing, undo/redo, serialization, or manual platform GUI validation in headless environments.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
