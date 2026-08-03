# Series preparation

Every tetrahedral series is prepared before it reaches a renderer. The process validates finite weights and the selected normalization policy, classifies the tetrahedral domain, applies the selected scientific clip policy, maps accepted data to world coordinates, and retains barycentric coordinates plus source indexes.

Polyline clipping is performed in barycentric space using the four linear inequalities `wi >= 0`. Surface triangles fully outside the scientific domain can be skipped, but clipping a triangle that crosses the boundary is intentionally a typed deferred error rather than a misleading partial render.