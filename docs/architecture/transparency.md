# Transparency

The outer tetrahedron defaults to wireframe-only because translucent faces can obscure internal data. Surface and section fill opacity use ordinary alpha blending; order-independent transparency and depth peeling are not implemented. Depth bias and normal offsets are renderer-only options reserved for section and break-line overlays and never modify scientific coordinates.