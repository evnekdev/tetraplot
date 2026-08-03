# Series preparation

Ordinary tetrahedral series validate and prepare before rendering. Embedded charts use the corresponding local pipeline:

```text
TernaryDiagram -> local validation and triangle clipping -> mesh-edge splitting
-> patch association -> tetrahedral/world mapping -> PreparedEmbeddedChart
```

For piecewise-linear surfaces, every local line segment is split exactly at parameter-mesh edges. A segment that crosses a declared break is represented as two patch-associated fragments sharing the same scientific crossing coordinate. Grid lines use this identical pipeline.

The prepared cache is keyed by embedding geometry/topology/break revisions, diagram revision, and style revision. Renderers consume prepared world geometry; they do not remap the local diagram on each frame.