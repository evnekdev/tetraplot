# State and revisions

Prepared embedded charts retain their existing embedding/diagram/style revision cache. Composition grids separately track coordinate, scalar, and row-structure revisions. Scalar edits do not regenerate regular coordinates; coordinate edits do.

`TetraplotDocument` has one modified flag/revision for editor invalidation. `EditorState` has its own revision for selection, flat target, and cursor changes. The flat texture cache includes both revision domains plus dimensions, so camera movement does not regenerate a flat bitmap.

This milestone centralizes mutations in `EditorCommand` and document/grid methods. It intentionally does not yet record undo/redo transactions, but these command boundaries are suitable future undo units.
