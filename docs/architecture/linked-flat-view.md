# Linked flat view

The optional `flat-view` feature adapts the same `TernaryDiagram` to the published `plotters-ternary` API. `Tetraplot::flat_chart(FlatViewTarget)` returns a renderable local chart; it never flattens world mesh positions or screenshots the 3D view.

The adapter renders local points, lines, grid, triangulation boundaries, declared break lines, and selection overlays. It uses `plotters-ternary::ViewportTransform` and `TernaryGeometry::unproject` for pointer recovery. `FlatChartImage::local_at_pixel` consequently shares the adapter's ternary coordinate mathematics.

The editor caches an RGBA texture by target, scientific/document revision, selection revision, and dimensions. Flat hover maps through the selected embedding into the linked 3D cursor.

## Grid and cursor overlays

The cached Plotters ternary image renders attached valid grid points. Selected rows, selected diagram series, and selected break lines are part of the relevant selection cache key. The linked cursor is drawn as an egui overlay so ordinary hover motion does not regenerate the bitmap.

Flat hover converts pixels through FlatChartImage to a local TernaryPoint, maps through the owning section or embedded chart, and updates local, tetrahedral, world, triangle, and patch cursor state. Clicking within a twelve-pixel grid-glyph threshold selects the exact CompositionGridId and GridRowId rather than reconstructing identity from coordinates.

The flat cache combines target revisions, matching grid coordinate/structure revisions, selection revision, and image size. Native camera orbit is deliberately absent from this key.
