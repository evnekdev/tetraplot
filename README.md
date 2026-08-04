# tetraplot

tetraplot is a Rust library for scientific visualization and editing in tetrahedral barycentric coordinates. Scientific coordinates, validation, topology, prepared geometry, editor state, and rendering remain separate.

    local ternary diagram -> validation and clipping -> embedding-aware splitting
    -> tetrahedral coordinates -> Cartesian world geometry -> software / three-d rendering

    composition grid + scalar fields -> document -> table / TSV / linked grid points

## Current functional slice

The library renders local ternary diagrams on planar triangular sections and piecewise-planar curved surfaces inside a tetrahedron. TriangulatedEmbedding owns validated mesh topology, patches, adjacency, and authoritative break lines. TernaryDiagram is the shared local dataset for embedded 3D geometry and the optional flat plotters-ternary adapter.

The optional editor provides:

- a real central three-d viewport clipped to the egui central panel;
- viewport-only orbit, zoom, and picking;
- a stable-ID table model with rectangular selection and keyboard navigation;
- direct regular-grid scalar and irregular-grid composition/scalar editing;
- dependent-component modes with validation;
- Excel-compatible TSV copy, header-aware or positional paste, transposition, clear, and fill-down;
- linked grid-row selection and cursor markers in the table, flat chart, and 3D scene;
- properties for frames, sections, embedded charts, break lines, grids, and scalar fields.

plot.show() remains the lightweight native viewer.

## Quick start

    use tetraplot::prelude::*;

    let mut plot = TetraplotBuilder::new().build()?;
    plot.draw_series(
        TetraPointSeries::new([[0.25, 0.25, 0.25, 0.25]])
            .color(RED)
            .size(7.0),
    )?;
    plot.save_png("composition.png", (1200, 900))?;

Run the scientific editor:

    cargo run --features editor --example editor

In the Data table tab, select generated compositions and choose **Copy compositions** before pasting into Excel. Select a scalar cell to paste one or several scalar columns back. Irregular grids accept component-plus-scalar blocks, retain malformed cells for correction, and expose the dependent-component selector. A valid row selected in any supported view is resolved through its stable GridRowId.

Headless workflows are demonstrated by regular_grid_data, irregular_grid_data, and flat_chart_export.

## Features

- window (default): lightweight native three-d viewer.
- image-export (default): software-rendered 3D PNG writing.
- flat-view: plotters and plotters-ternary local chart rendering and flat PNG export.
- editor: native scientific editor, three-d's version-matched egui layer, flat view, and OS clipboard support.

plotters-ternary is optional and does not appear in the scientific diagram, embedding, topology, or prepared 3D geometry APIs. Cargo.lock is intentionally committed for reproducible renderer/editor examples. The package MSRV is 1.92.

## Current limitations

The alpha editor has one docked flat view and a fixed resizable panel layout. It has no detached views, contours, filled curved regions, scalar colour maps, topology editing, undo/redo, or project serialization. Sorting is available in the renderer-independent table model but is not yet exposed as a header-click GUI control. Native visual and Excel interoperability checks are platform/manual checks rather than headless unit tests.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
