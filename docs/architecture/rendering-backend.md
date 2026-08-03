# Rendering backend

The deterministic CPU renderer and the optional `three-d` window adapter consume `PreparedEmbeddedChart`. They render supporting indexed surface geometry, patch-aware normals, break-line overlays, chart boundaries, local grid lines, mapped curves, and mapped points.

The CPU renderer uses a documented renderer-only depth bias for overlays and simple deterministic directional shading. The native adapter converts the same prepared vertices, normals, indexed triangles, tubes, and point glyphs into `three-d` resources. CI compiles the native path; it does not open a native window for visual inspection.