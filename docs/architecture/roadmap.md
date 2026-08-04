# Roadmap

The current milestone supplies a document/editor foundation: regular and irregular composition grids, TSV/clipboard model, selection, prepared-surface picking, one cached linked `plotters-ternary` view, and a fixed native editor shell.

Next milestone:

1. detachable and multiple flat ternary windows;
2. scalar colour maps and patch-aware contour extraction;
3. richer legends and colour bars;
4. undo/redo and deeper interactive property/series editing;
5. explicit grid-point rendering/highlighting in the native 3D viewport.

Filled curved polygons, full interactive surface-topology editing, document serialization, VTK, WebAssembly, arbitrary mesh import, and high-performance indexing remain intentionally outside the current scope.

## Completed alpha milestone

The alpha editor now includes a true central viewport, spreadsheet-style rectangular interaction, regular scalar rectangles, irregular component/scalar rows, dependent entry controls, structured clipboard summaries, prepared grid glyphs, stable bidirectional table/flat/3D selection, renderer-only highlights, and command-routed basic properties.

Deferred work remains detached/multiple flat windows, contours and filled contour bands, arbitrary analytic surfaces, topology or break-line dragging, undo/redo, project serialization, VTK, WebAssembly, plugin/background-job systems, and advanced transparency.
