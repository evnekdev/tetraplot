# Camera and viewport

Scientific tetrahedral domain, Cartesian scene bounds, renderer-independent camera, and output allocation remain separate. `TetraplotBuilder` resolves implicit bounds and camera only at `build`: later selected geometry affects implicit defaults, while an explicit camera or bounds remains fixed. `ViewportFit` and `ViewportAlignment` are retained as output policy even though the initial software rasterizer centres the projected camera image.
## Editor viewport

EditorViewport records the central egui rectangle, device-pixel ratio, and the matching bottom-left physical renderer rectangle. The native camera viewport and render scissor use the physical rectangle. Pointer and gesture events must be both unconsumed and inside that rectangle before orbit, zoom, or picking can observe them. Rectangles smaller than two physical pixels are ignored safely.
