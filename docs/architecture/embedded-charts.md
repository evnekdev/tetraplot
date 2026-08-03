# Embedded ternary charts

`TernaryDiagram` is the single owner of local ternary points and lines. `ChartEmbedding` supplies the mapping into tetrahedral coordinates. `EmbeddedTernaryChart` binds the pair and caches the renderer-neutral `PreparedEmbeddedChart`.

`PlanarEmbedding` is the affine special case. `PlanarSection` computes its explicit plane–tetrahedron intersection, refuses chart content for quadrilateral cuts, and converts triangular cuts through this same prepared representation. The software and native renderers therefore do not have separate planar-section and curved-chart series paths.

`TriangulatedEmbedding` represents a piecewise-linear parameterization of the reference ternary domain. It validates matching vertex arrays, nondegenerate consistently oriented local/world triangles, a connected complete boundary, and local-domain coverage. Its compact adjacency table drives patch connectivity validation, edge classification, mapped-line splitting, and break-line checks.

Patches form a complete, non-overlapping partition of triangles. Every internal edge between patches must be explicitly declared as a break before preparation. Break lines follow existing internal mesh edges; their adjacent patch IDs are derived from incident triangles and caller-supplied IDs are checked. Scientific vertices remain shared while prepared rendering vertices are duplicated per patch where normals differ.

Mapped lines and grids split at parameter-triangle edges and declared breaks. The prepared fragments retain local, tetrahedral, world, triangle, patch, and break-crossing data. Contours, filled curved regions, picking, and arbitrary smooth parametric embeddings remain future work.