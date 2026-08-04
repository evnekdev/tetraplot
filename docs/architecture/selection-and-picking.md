# Selection and picking

`Selection` is editor-wide and uses stable scientific identities: scene series, sections, embedded charts, patches, break lines, grids, fields and rows. A view never stores an independent selection.

`Ray`, `pick_prepared_surface`, and `pick_embedded_chart` are renderer-independent. They intersect the prepared support triangles, interpolate local ternary and tetrahedral barycentric weights, preserve source triangle/patch identity, and detect a nearby declared break line. A miss is `None`, not an error.

The native editor derives a ray from the three-d camera on an unhandled left click, performs this scientific recovery, and updates the shared selection/cursor. Rendering highlights are UI state and never mutate source styles.

## Hit precedence and linkage

EditorPickResult carries stable source metadata for grid rows, series, break lines, patches, surface triangles, local and tetrahedral coordinates, world position, and scalar context. Native hit precedence is grid point, embedded diagram point, break line, diagram line, supporting surface, then frame fallback. Ray proximity thresholds are applied before surface intersection so foreground glyphs do not collapse into a surface selection.

Selection::GridRow is shared by the table, flat view, and 3D overlay. A graphical grid hit calls EditorState::select_grid_row, activates the owning grid, restores the row in any sorted order, and updates first_visible_row. Removed rows and grids clear stale selection safely.

Renderer-only overlays include grid halos, linked cursor markers, chart or patch material emphasis, break-line widening, and point/line-series emphasis. Source scientific styles are not mutated by selection.
