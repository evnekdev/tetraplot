# Editor architecture

The optional editor feature wraps TetraplotDocument in a TetraplotEditor application without changing the lightweight Tetraplot::show() viewer.

Four layers remain distinct:

1. TetraplotDocument owns the plot and composition grids.
2. EditorState owns stable selection, table states, flat target, linked cursor, and status.
3. Renderer-independent preparation owns scientific geometry and picking metadata.
4. The three-d/egui adapter owns native resources, input, textures, and visual overlays.

The egui shell creates the top, scene, property, lower, and status panels before the central panel. The resulting central logical rectangle is converted with device-pixel ratio and top-left/bottom-left correction into EditorViewport::physical_rect. The three-d camera uses that viewport and the scene is rendered with a scissor rectangle. Empty or tiny rectangles are rejected safely.

Mouse events outside the viewport are marked handled before orbit control. Picking uses only unhandled left presses inside EditorViewport, so side-panel interaction cannot orbit or select 3D objects. Camera motion changes camera state but does not rebuild scientific geometry.

EditorCommand routes document mutations. Widget-local selection and focus remain in EditorState/DataTableState; scientific values change through invariant-preserving plot, chart, section, and grid methods. Errors are returned to the status area or retained as cell/row validation.
