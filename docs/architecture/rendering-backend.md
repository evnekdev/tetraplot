# Rendering backend

Prepared points, polyline fragments, and indexed surfaces are backend-neutral. Static output uses a deterministic CPU rasterizer and returns top-to-bottom RGBA bytes; PNG encoding is an optional final step. The native adapter translates the same prepared scene to `three-d`, with orbit control, zoom, resize handling, depth testing, tube lines, and octahedral point glyphs.

Software and window renderers are separately testable. CI compiles the window backend but does not open a GUI window.