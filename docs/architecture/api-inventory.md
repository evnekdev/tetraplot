# Public API inventory

- Coordinates: `TetraPoint`, `TetraGeometry`, `Tolerance`, `Normalization`, `DomainClip`.
- Scene: `TetraplotBuilder`, `Tetraplot`, `Camera`, `TetraFrameConfig`.
- Ordinary series: `TetraPointSeries`, `TetraLineSeries`, `TetraSurfaceSeries`.
- Local diagrams: `TernaryPoint`, `TernaryDiagram`, `SectionSeriesId`.
- Embeddings: `PlanarEmbedding`, `TriangulatedEmbedding`, `ChartEmbedding`, `LocatedSurfacePoint`.
- Piecewise topology: `SurfaceTopology`, `SurfacePatch`, `SurfaceEdgeKind`, `BreakLine`.
- Prepared geometry: `PreparedEmbeddedChart`, `PreparedEmbeddedSurface`, `PreparedEmbeddedLine`.
- Composition data: `CompositionGrid`, `RegularCompositionGrid`, `IrregularCompositionGrid`, `ScalarField`, `ClipboardTable`, `GridExportOptions`.
- Editor-neutral state: `TetraplotDocument`, `Selection`, `EditorState`, `EditorCommand`, `Ray`, `EditorPickResult`.
- Rendering: `RenderedImage`, `Tetraplot::render`, `save_png`, and feature-gated `show`.
- Optional flat view: `FlatChart`, `FlatChartImage`, `FlatViewTarget`, and `Tetraplot::flat_chart`.
- Optional editor: `TetraplotEditor` and `Tetraplot::show_editor`.

`TriangulatedEmbedding` exposes read-only mesh data. Patches are installed with `set_patches`; break lines are added with `add_break_line`; their adjacency is derived from the mesh rather than caller metadata. Composition rows and fields retain stable identities even when irregular table rows are reordered.

## Added editor APIs

- Table model: GridColumnKind, GridColumn, GridCellAddress, CellRange, DataTableState, CellEditBuffer, and ParsedCellValue.
- Viewport: EditorViewport and PhysicalViewport with logical/physical containment and conversion.
- Data workflow: CompositionGrid column/cell access, selected_table, component edit, row insert/delete, field metadata, and independent coordinate/scalar/structure revisions.
- Commands: cell/component edit, rectangular and append paste, entry and duplicate modes, row operations, field properties, frame/chart/break properties, removal, fit, and reset.
- Picking: pick_grid_points plus point, break-line, diagram-line, and supporting-surface precedence in pick_embedded_chart.
- Flat view: grid_points, selected_points, linked_cursor options, local_at_pixel, and pixel_at_local.
