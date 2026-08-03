//! Structured failures reported by tetraplot.

use thiserror::Error;

/// Errors in scientific coordinate validation and conversion.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum CoordinateError {
    #[error(
        "invalid tolerance: absolute={absolute:?}, relative={relative:?}; both must be finite and positive"
    )]
    InvalidTolerance { absolute: f64, relative: f64 },
    #[error("component {component} is not finite: {value:?}")]
    NonFiniteComponent { component: usize, value: f64 },
    #[error("component {component} is negative beyond tolerance {tolerance:?}: {value:?}")]
    NegativeComponent {
        component: usize,
        value: f64,
        tolerance: f64,
    },
    #[error("barycentric sum must be finite and greater than {minimum:?}: {sum:?}")]
    InvalidSum { sum: f64, minimum: f64 },
    #[error("required sum must be finite and greater than {minimum:?}: {required_sum:?}")]
    InvalidRequiredSum { required_sum: f64, minimum: f64 },
    #[error(
        "barycentric sum {actual:?} does not match {expected:?} within {absolute:?} absolute tolerance"
    )]
    RequiredSumMismatch {
        expected: f64,
        actual: f64,
        absolute: f64,
    },
    #[error("component index {index} is outside 0..4")]
    InvalidComponentIndex { index: usize },
    #[error("interpolation parameter is not finite: {value:?}")]
    NonFiniteInterpolationParameter { value: f64 },
    #[error("Cartesian point {point:?} is outside the tetrahedron")]
    CartesianOutsideTetrahedron { point: [f64; 3] },
    #[error("section-local ternary coordinate is invalid: {message}")]
    InvalidSectionCoordinate { message: &'static str },
}

/// Errors in tetrahedron construction and geometric operations.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum GeometryError {
    #[error("tetrahedron vertex {vertex} is not finite: {value:?}")]
    NonFiniteVertex { vertex: usize, value: [f64; 3] },
    #[error(
        "tetrahedron is degenerate: signed volume {signed_volume:?} is too small for tolerance {minimum_volume:?}"
    )]
    DegenerateTetrahedron {
        signed_volume: f64,
        minimum_volume: f64,
    },
    #[error("geometry operation needs a non-degenerate tetrahedron")]
    DegenerateOperation,
    #[error("face {face} has no stable normal")]
    DegenerateFace { face: usize },
}

/// Errors in camera, bounds, and output allocation.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ViewportError {
    #[error("scene bounds must be finite: min={min:?}, max={max:?}")]
    NonFiniteBounds { min: [f64; 3], max: [f64; 3] },
    #[error("scene bounds are empty or reversed: min={min:?}, max={max:?}")]
    InvalidBounds { min: [f64; 3], max: [f64; 3] },
    #[error("camera value {field} is not finite: {value:?}")]
    NonFiniteCamera {
        field: &'static str,
        value: [f64; 3],
    },
    #[error("camera position and target cannot coincide")]
    CoincidentCameraTarget,
    #[error("output dimensions must both be non-zero: {width}x{height}")]
    InvalidImageDimensions { width: u32, height: u32 },
}

/// Errors preparing a series into renderer-independent primitives.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum SeriesError {
    #[error("invalid point at source index {index}: {source}")]
    InvalidPoint {
        index: usize,
        #[source]
        source: CoordinateError,
    },
    #[error("point at source index {index} is outside the tetrahedral domain")]
    OutsideDomain { index: usize },
    #[error(
        "triangle {triangle} references vertex {vertex}, but only {vertex_count} vertices exist"
    )]
    TriangleIndexOutOfBounds {
        triangle: usize,
        vertex: u32,
        vertex_count: usize,
    },
    #[error("triangle {triangle} repeats one or more vertex indices")]
    RepeatedTriangleIndex { triangle: usize },
    #[error("triangle {triangle} is geometrically degenerate")]
    DegenerateTriangle { triangle: usize },
    #[error("{kind} scalar values have length {actual}; expected {expected}")]
    ScalarValueLength {
        kind: &'static str,
        actual: usize,
        expected: usize,
    },
    #[error("scalar value {index} is not finite: {value:?}")]
    NonFiniteScalar { index: usize, value: f64 },
    #[error("surface clipping is not implemented for a triangle crossing the tetrahedron boundary")]
    SurfaceClippingDeferred,
}

/// Errors in planar-section construction and mutation.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum SectionError {
    #[error("section plane has a non-finite coefficient at index {index}: {value:?}")]
    NonFinitePlaneCoefficient { index: usize, value: f64 },
    #[error("section plane has a zero or numerically degenerate normal")]
    DegeneratePlane,
    #[error("constant component value must lie in [0, 1] within tolerance: {value:?}")]
    InvalidConstantComponent { value: f64 },
    #[error("plane coincides with tetrahedron face opposite component {component}")]
    CoincidentFace { component: usize },
    #[error("section {id} does not exist")]
    UnknownSection { id: u64 },
    #[error(
        "section intersection is quadrilateral; ternary chart content requires a triangular section"
    )]
    ChartRequiresTriangle,
    #[error("point is not on this section plane within tolerance")]
    PointOutsideSectionPlane,
    #[error("section-local triangle is degenerate")]
    DegenerateSectionTriangle,
    #[error("section chart series {id} does not exist")]
    UnknownSectionSeries { id: u64 },
    #[error(transparent)]
    Coordinate(#[from] CoordinateError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error(transparent)]
    Series(#[from] SeriesError),
}

/// Errors during interactive rendering or image export.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RenderError {
    #[error("the `window` feature is disabled")]
    WindowFeatureDisabled,
    #[error("the `image-export` feature is disabled")]
    ImageExportFeatureDisabled,
    #[error("failed to create or use the graphics backend: {message}")]
    Backend { message: String },
    #[error("failed to encode PNG: {message}")]
    ImageEncoding { message: String },
}

/// The crate-wide error type.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TetraplotError {
    #[error(transparent)]
    Coordinate(#[from] CoordinateError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error(transparent)]
    Viewport(#[from] ViewportError),
    #[error(transparent)]
    Series(#[from] SeriesError),
    #[error(transparent)]
    Section(#[from] SectionError),
    #[error(transparent)]
    Render(#[from] RenderError),
}

/// A convenient result using [`TetraplotError`].
pub type Result<T> = std::result::Result<T, TetraplotError>;
