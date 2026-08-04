# Selection and picking

Selection is shared editor state and uses stable identities for ordinary series, sections, embedded charts, diagram series, patches, break lines, grids, fields, rows, and cells.

Renderer-independent picking supports ordinary point/line series, embedded diagram points/lines, grid points, planar/curved supporting surfaces, and break-line proximity. Surface hits interpolate exact local ternary and tetrahedral coordinates and retain source triangle and patch metadata. Grid hits carry their original CompositionGridId and GridRowId.

select_best_pick applies explicit precedence:

1. point, grid-point, or diagram/ordinary series;
2. declared break line;
3. section, embedded surface, or patch;
4. frame/fallback.

Distance along the ray resolves candidates within one tier. Stable IDs are taken from prepared geometry rather than reconstructed by approximate position.

The native adapter derives rays only for unhandled clicks inside EditorViewport. Selection overlays are renderer-only: larger magenta point glyphs, wider series/break overlays, subtle patch emphasis, and boundary emphasis. A cyan linked cursor remains distinct from selection and never alters scientific styles.
