//! Renderer-independent scientific visualization in tetrahedral barycentric coordinates.
#![forbid(unsafe_code)]
mod chart;
mod coord;
mod data;
mod editor;
#[cfg(feature = "editor")]
mod editor_gui;
mod embedded;
mod error;
#[cfg(feature = "flat-view")]
mod flat;
pub mod prelude;
mod render;
mod series;
mod style;
mod table;
pub use chart::*;
pub use coord::*;
pub use data::*;
pub use editor::*;
#[cfg(feature = "editor")]
pub use editor_gui::TetraplotEditor;
pub use embedded::*;
pub use error::*;
#[cfg(feature = "flat-view")]
pub use flat::*;
pub use render::*;
pub use series::*;
pub use style::*;
pub use table::*;
