# Embedded ternary charts

A `TernaryDiagram` is independent from its `ChartEmbedding`. `PlanarEmbedding` maps local ternary weights affinely through the three triangular section vertices. `TriangulatedEmbedding` maps with piecewise barycentric interpolation over a local parameter mesh and validates orientation, indices, nondegeneracy, edge manifold counts, and declared boundaries.

Planar sections calculate explicit plane–tetrahedron intersections. Empty, point, segment, triangle, and quadrilateral outcomes are represented distinctly. Quadrilateral sections are never silently diagonalized into a ternary chart.

Triangulated embeddings preserve surface patches, authoritative mesh-edge break lines, break-line styles, and separate revisions. Renderer vertices may be duplicated at a break later, but the scientific mesh remains position-continuous. Patch-aware normal generation, adaptive curve splitting at breaks, contours, and picking are the next milestones.