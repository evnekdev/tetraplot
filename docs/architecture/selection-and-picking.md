# Selection and picking

`Selection` is editor-wide and uses stable scientific identities: scene series, sections, embedded charts, patches, break lines, grids, fields and rows. A view never stores an independent selection.

`Ray`, `pick_prepared_surface`, and `pick_embedded_chart` are renderer-independent. They intersect the prepared support triangles, interpolate local ternary and tetrahedral barycentric weights, preserve source triangle/patch identity, and detect a nearby declared break line. A miss is `None`, not an error.

The native editor derives a ray from the three-d camera on an unhandled left click, performs this scientific recovery, and updates the shared selection/cursor. Rendering highlights are UI state and never mutate source styles.
