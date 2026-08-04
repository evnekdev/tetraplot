# State and revisions

Prepared embedded charts retain their existing embedding/diagram/style revision cache. Composition grids separately track coordinate, scalar, and row-structure revisions. Scalar edits do not regenerate regular coordinates; coordinate edits do.

`TetraplotDocument` has one modified flag/revision for editor invalidation. `EditorState` has its own revision for selection, flat target, and cursor changes. The flat texture cache includes both revision domains plus dimensions, so camera movement does not regenerate a flat bitmap.

This milestone centralizes mutations in `EditorCommand` and document/grid methods. It intentionally does not yet record undo/redo transactions, but these command boundaries are suitable future undo units.

## Revision responsibilities

TetraplotDocument revision marks persistent document mutation and native base-model changes. Composition grids additionally separate coordinate, scalar, and structure revisions; each ScalarField has its own revision. PlanarSection and EmbeddedTernaryChart expose revision fingerprints for flat and prepared caches.

Native camera movement is held by three-d and does not mutate TetraplotDocument or rebuild scientific geometry. Grid base glyphs rebuild on document mutation; selection and linked-cursor models rebuild from EditorState revision only. Flat images use target, relevant grid, selection, and size revisions rather than camera state.

Invalid scalar keystrokes update DataTableState staging only. They do not change coordinate or scalar revisions and therefore do not trigger scientific preparation.
