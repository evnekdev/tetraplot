# State and revisions

Prepared embedded charts retain embedding geometry, topology, break, diagram, and style revision keys. Composition grids independently track coordinate, scalar, and structure revisions; a scalar edit cannot regenerate regular-grid coordinates.

TetraplotDocument has a structural/modified revision. EditorState separately tracks selection, cursor, and flat-target revisions plus per-grid DataTableState. The native GPU model key combines scientific document state with only highlight state that changes renderer resources; camera movement updates the camera directly.

TetraplotDocument::flat_revision hashes one target section/chart and only grids attached to it. It deliberately excludes camera and unrelated-document edits. The texture cache adds selection/cursor revisions and panel dimensions.

Plane, embedding, topology, break, diagram, style, grid, and scalar mutations use invariant-preserving methods or EditorCommand. Removing grids, fields, sections, or charts calls EditorState::clear_removed, which removes stale selections, cursors, flat targets, and table state. Undo/redo is not implemented, but command boundaries are suitable future transaction units.
