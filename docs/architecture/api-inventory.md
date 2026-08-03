# Public API inventory

- Coordinates: `TetraPoint`, `TetraGeometry`, `Tolerance`, `Normalization`, `DomainClip`.
- Chart scene: `TetraplotBuilder`, `Tetraplot`, `Camera`, `TetraFrameConfig`.
- Ordinary series: `TetraPointSeries`, `TetraLineSeries`, `TetraSurfaceSeries`.
- Local diagram: `TernaryPoint`, `TernaryDiagram`, `SectionSeriesId`.
- Embeddings: `PlanarEmbedding`, `TriangulatedEmbedding`, `ChartEmbedding`, `LocatedSurfacePoint`.
- Piecewise topology: `SurfaceTopology`, `SurfacePatch`, `SurfaceEdgeKind`, `BreakLine`.
- Prepared geometry: `PreparedEmbeddedChart`, `PreparedEmbeddedSurface`, `PreparedEmbeddedLine`.
- Rendering: `RenderedImage`, `Tetraplot::render`, `save_png`, and feature-gated `show`.

`TriangulatedEmbedding` exposes read-only mesh data. Patches are installed with `set_patches`; break lines are added with `add_break_line`; their adjacency is derived from the mesh rather than caller metadata.