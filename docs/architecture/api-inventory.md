# Public API inventory

- Coordinates: TetraPoint, TetraGeometry, TernaryPoint, Tolerance, Normalization, and DomainClip.
- Scene: TetraplotBuilder, Tetraplot, Camera, and TetraFrameConfig.
- Ordinary series: TetraPointSeries, TetraLineSeries, TetraSurfaceSeries, and SeriesId.
- Local diagrams: TernaryDiagram, SectionSeriesId, and DiagramSeries.
- Embeddings: PlanarEmbedding, TriangulatedEmbedding, ChartEmbedding, and LocatedSurfacePoint.
- Piecewise topology: SurfaceTopology, SurfacePatch, SurfaceEdgeKind, and BreakLine.
- Prepared geometry: PreparedEmbeddedChart, its surface/point/line/grid/break types, and PreparedGridPoint.
- Composition data: regular/irregular CompositionGrid, stable grid/row/field IDs, ScalarField, entry/duplicate/redefinition policies, validation, ClipboardTable, and GridExportOptions.
- Table model: GridColumn, GridCellAddress, CellRange, CellEditBuffer, DataTableState, TableMove, and TableSort.
- Editor-neutral state: TetraplotDocument, Selection, EditorState, EditorCommand, EditorViewport, Ray, EditorPickResult, and picking helpers.
- Rendering: RenderedImage, Tetraplot::render, save_png, and feature-gated show.
- Optional flat view: FlatChart, FlatChartImage, FlatRenderOptions, FlatGridPoint, FlatViewTarget, and Tetraplot::flat_chart.
- Optional editor: TetraplotEditor and Tetraplot::show_editor.

Scientific containers expose read-only topology/data accessors and invariant-preserving mutation methods. Patches and break adjacency are validated from the surface mesh. Table sorting and selection operate on stable IDs, so vector positions are never persistent identities.
