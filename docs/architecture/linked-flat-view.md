# Linked flat view

The optional `flat-view` feature adapts the same `TernaryDiagram` to the published `plotters-ternary` API. `Tetraplot::flat_chart(FlatViewTarget)` returns a renderable local chart; it never flattens world mesh positions or screenshots the 3D view.

The adapter renders local points, lines, grid, triangulation boundaries, declared break lines, and selection overlays. It uses `plotters-ternary::ViewportTransform` and `TernaryGeometry::unproject` for pointer recovery. `FlatChartImage::local_at_pixel` consequently shares the adapter's ternary coordinate mathematics.

The editor caches an RGBA texture by target, scientific/document revision, selection revision, and dimensions. Flat hover maps through the selected embedding into the linked 3D cursor.
