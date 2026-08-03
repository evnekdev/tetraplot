use super::{
    Component, Normalization, Tolerance, validate_affine_weights, validate_domain_weights,
};
use crate::CoordinateError;
/// A four-component tetrahedral barycentric coordinate in stable A/B/C/D order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TetraPoint {
    weights: [f64; 4],
}
impl TetraPoint {
    /// Construct a strict, unit-sum in-domain point using the default tolerance.
    pub fn new(weights: [f64; 4]) -> Result<Self, CoordinateError> {
        Self::with_policy(weights, Normalization::RequireUnitSum, Tolerance::default())
    }
    /// Construct a point after explicit domain validation and normalization policy.
    pub fn with_policy(
        weights: [f64; 4],
        normalization: Normalization,
        tolerance: Tolerance,
    ) -> Result<Self, CoordinateError> {
        Ok(Self {
            weights: validate_domain_weights(weights, normalization, tolerance)?,
        })
    }
    /// Construct an unrestricted point for input preparation or internal interpolation.
    pub const fn from_unchecked(weights: [f64; 4]) -> Self {
        Self { weights }
    }
    pub const fn as_array(self) -> [f64; 4] {
        self.weights
    }
    pub const fn weight(self, component: Component) -> f64 {
        self.weights[component.index()]
    }
    pub fn component(self, index: usize) -> Result<f64, CoordinateError> {
        self.weights
            .get(index)
            .copied()
            .ok_or(CoordinateError::InvalidComponentIndex { index })
    }
    pub fn sum(self) -> f64 {
        self.weights.iter().sum()
    }
    pub fn is_finite(self) -> bool {
        self.weights.iter().all(|weight| weight.is_finite())
    }
    pub fn validate(
        self,
        normalization: Normalization,
        tolerance: Tolerance,
    ) -> Result<Self, CoordinateError> {
        Self::with_policy(self.weights, normalization, tolerance)
    }
    pub fn validate_affine(
        self,
        normalization: Normalization,
        tolerance: Tolerance,
    ) -> Result<Self, CoordinateError> {
        Ok(Self {
            weights: validate_affine_weights(self.weights, normalization, tolerance)?,
        })
    }
    pub fn normalized(self, tolerance: Tolerance) -> Result<Self, CoordinateError> {
        Ok(Self {
            weights: validate_affine_weights(self.weights, Normalization::Normalize, tolerance)?,
        })
    }
    pub fn interpolate(self, other: Self, amount: f64) -> Result<Self, CoordinateError> {
        if !amount.is_finite() {
            return Err(CoordinateError::NonFiniteInterpolationParameter { value: amount });
        }
        Ok(Self::from_unchecked(std::array::from_fn(|index| {
            self.weights[index] + (other.weights[index] - self.weights[index]) * amount
        })))
    }
}
