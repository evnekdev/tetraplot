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

## Editor data workflow

Run:

```text
cargo run --features editor --example editor
```

The central panel is a physical 3D viewport: its camera aspect ratio, scissor rectangle,
orbit/zoom input, and scientific picking all follow the resizable central panel. Side and
bottom panels do not pass pointer input to the camera. Grid points are prepared from stable
row IDs and rendered in 3D; rows selected in the table or flat view receive a contrasting
halo, while the linked flat cursor uses a separate red marker.

The docked data table supports one active cell, shift-extended rectangular ranges, arrow,
Tab, and Enter navigation, Ctrl+A, Ctrl+C, Ctrl+V, Delete/Backspace, copying with or without
headers, transposed paste, clear, and fill down. Regular compositions are generated,
read-only, and directly copyable to Excel. Scalar cells accept single values, columns, and
multi-field rectangles. Invalid scalar text remains visibly staged in its cell rather than
becoming a missing value.

Irregular grids support component and scalar editing, insert/append/delete, and
composition-plus-scalar TSV paste. Choose all-component entry or a dependent tetrahedral or
local component above the table; dependent values update after every independent edit.
Incomplete and invalid rows remain visible, keep their stable IDs, and are excluded from
scientific point preparation until valid. Header-aware paste recognizes canonical A/B/C/D
or u/v/w component names, scalar names, and scalar names with units. Headerless paste maps
positionally from the active cell, preserves empty cells, and reports ragged or unknown
input in the status area.

Selecting a valid grid row links the table, flat ternary chart, and 3D view. Picking a grid
glyph in either graphical view activates its grid, selects its stable row ID, and scrolls
the table without losing identity after row sorting.

## Current limitations

The alpha editor intentionally keeps one docked flat view and a fixed panel layout. Detached views, contours and filled contour bands, topology editing, draggable break lines, undo/redo, project serialization, VTK export, WebAssembly, plugins, and advanced order-independent transparency remain deferred. Invalid scalar text is editor staging state and is not written into the scientific scalar array until corrected.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
