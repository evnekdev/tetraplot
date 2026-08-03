# Editor architecture

The optional `editor` feature adds `TetraplotEditor`, an application shell around a `TetraplotDocument`. It deliberately separates four layers:

1. `TetraplotDocument` owns the plot and composition grids.
2. `EditorState` owns selection, active data grid, flat-view target, linked cursor, and status text.
3. prepared geometry remains owned by the scientific/render preparation pipeline.
4. the `three-d` + built-in egui implementation owns native windows, textures, and input.

`Tetraplot::show()` remains the lightweight viewer. `TetraplotEditor::new(plot).run()` opens the fuller interface. The editor feature enables three-d's matching `egui-gui` integration; it intentionally does not depend on `egui-winit`, whose current winit line differs from three-d's.

The first shell has fixed resizable scene, property, central-view, lower-view and status regions. It does not provide arbitrary docking or document serialization.
