# Editor architecture

The optional `editor` feature adds `TetraplotEditor`, an application shell around a `TetraplotDocument`. It deliberately separates four layers:

1. `TetraplotDocument` owns the plot and composition grids.
2. `EditorState` owns selection, active data grid, flat-view target, linked cursor, and status text.
3. prepared geometry remains owned by the scientific/render preparation pipeline.
4. the `three-d` + built-in egui implementation owns native windows, textures, and input.

`Tetraplot::show()` remains the lightweight viewer. `TetraplotEditor::new(plot).run()` opens the fuller interface. The editor feature enables three-d's matching `egui-gui` integration; it intentionally does not depend on `egui-winit`, whose current winit line differs from three-d's.

The first shell has fixed resizable scene, property, central-view, lower-view and status regions. It does not provide arbitrary docking or document serialization.

## Alpha editor shell

The native editor reserves the real central-panel rectangle for 3D rendering. EditorViewport converts egui logical top-left coordinates to a clamped, bottom-left physical viewport. The three-d camera uses that viewport, and the render target clears and renders it through a matching scissor box. Orbit, wheel, gesture, and picking events are cloned into the scene input stream only when their physical position is inside this rectangle and egui has not consumed them.

The fixed shell retains scene, properties, data/flat, and status panels. Fit scene rebuilds the native camera from the document camera. The properties panel routes frame, section, embedded-chart, break-line, grid, and scalar mutations through EditorCommand. Recoverable command, clipboard, export, and render failures are displayed in editor status.

Native GUI behaviour is intended for Windows verification. Automated tests cover viewport conversion and outside-viewport rejection logic; this document does not claim a completed human GUI session.
