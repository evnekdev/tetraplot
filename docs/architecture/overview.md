# Architecture overview

Tetraplot keeps dependency direction strictly downward: coordinate and domain model; validation and preparation; topology and embedded charts; prepared primitives; rendering adapters. `three-d`, image encoding, and window types are isolated in `render`.

The current single-crate layout deliberately preserves a future split into `tetraplot-core`, `tetraplot-three-d`, and a public facade without creating premature workspace overhead.