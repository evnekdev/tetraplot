//! Renderer-independent tetrahedral coordinates, geometry, clipping, and bounds.

mod clipping;
mod geometry;
mod point;
mod tolerance;
mod validation;
mod viewport;

pub use clipping::{ClippedSegment, TetraSegment, clip_segment, clip_segment_with_parameters};
pub use geometry::{Component, Edge, Face, Handedness, TetraGeometry, TetraPointLocation};
pub use point::TetraPoint;
pub use tolerance::Tolerance;
pub use validation::Normalization;
pub use viewport::{SceneBounds, ViewportAlignment, ViewportFit};

pub(crate) use validation::{validate_affine_weights, validate_domain_weights};
