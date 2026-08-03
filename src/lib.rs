//! Renderer-independent scientific visualization in tetrahedral barycentric coordinates.
#![forbid(unsafe_code)]
mod chart;
mod coord;
mod embedded;
mod error;
pub mod prelude;
mod render;
mod series;
mod style;
pub use chart::*;
pub use coord::*;
pub use embedded::*;
pub use error::*;
pub use render::*;
pub use series::*;
pub use style::*;
