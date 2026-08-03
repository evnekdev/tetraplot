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
