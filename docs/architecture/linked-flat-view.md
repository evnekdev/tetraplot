# Linked flat view

The optional flat-view feature adapts the same TernaryDiagram to plotters-ternary; it does not flatten world mesh positions or rasterize the 3D view. Tetraplot::flat_chart renders local diagram points/lines, ternary mesh, triangulation/break overlays, attached grid points, selected series/row, and linked cursor.

FlatChartImage owns the exact ViewportTransform used for rendering. local_at_pixel recovers a local ternary point and rejects pixels outside the simplex; pixel_at_local supports stable grid-point hit testing.

The editor cache key uses target chart/section revisions, only attached grid coordinate/scalar/structure revisions, highlight revisions, and output dimensions. Ordinary camera changes do not invalidate it.

Hover maps the recovered local coordinate through the current embedding to tetrahedral/world coordinates and triangle/patch metadata. A nearby rendered grid point wins within the flat-view pixel threshold and selects its stable row, which opens the table and requests scrolling to that row. Table/cell selection highlights the same row in both flat and 3D views.
