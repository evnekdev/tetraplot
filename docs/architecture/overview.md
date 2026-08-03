# Architecture overview

`TernaryDiagram` owns local ternary scientific data. `ChartEmbedding` maps that one diagram into parent tetrahedral coordinates. `EmbeddedTernaryChart` owns one diagram, one embedding, style/state, and a revision-keyed prepared cache.

```text
TernaryDiagram + ChartEmbedding -> PreparedEmbeddedChart -> renderer adapter
```

`PlanarSection` remains an ergonomic editable plane/intersection builder. For triangular intersections it constructs an affine `PlanarEmbedding` and feeds the same prepared embedded-chart path used by `TriangulatedEmbedding`; it has no independent section-series renderer.

A triangulated embedding owns compact mesh adjacency, a complete non-overlapping patch partition, and authoritative break-edge chains. Scientific coordinates remain shared at a break. Render vertices are duplicated only during preparation where patch-specific normals require it.