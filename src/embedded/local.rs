use crate::{CoordinateError, Normalization, Tolerance};
/// A point in the reference ternary simplex of an embedded chart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TernaryPoint {
    weights: [f64; 3],
}
impl TernaryPoint {
    pub fn new(weights: [f64; 3]) -> Result<Self, CoordinateError> {
        Self::with_policy(weights, Normalization::RequireUnitSum, Tolerance::default())
    }
    pub fn with_policy(
        mut weights: [f64; 3],
        normalization: Normalization,
        tolerance: Tolerance,
    ) -> Result<Self, CoordinateError> {
        tolerance.validate()?;
        for (index, weight) in weights.iter_mut().enumerate() {
            if !weight.is_finite() {
                return Err(CoordinateError::NonFiniteComponent {
                    component: index,
                    value: *weight,
                });
            }
            if *weight < -tolerance.absolute {
                return Err(CoordinateError::NegativeComponent {
                    component: index,
                    value: *weight,
                    tolerance: tolerance.absolute,
                });
            }
            if *weight < 0.0 {
                *weight = 0.0;
            }
        }
        let sum: f64 = weights.iter().sum();
        if sum <= tolerance.absolute {
            return Err(CoordinateError::InvalidSum {
                sum,
                minimum: tolerance.absolute,
            });
        }
        let expected = match normalization {
            Normalization::Normalize => sum,
            Normalization::RequireUnitSum => 1.0,
            Normalization::RequireSum(value) => value,
        };
        if !expected.is_finite() || expected <= tolerance.absolute {
            return Err(CoordinateError::InvalidRequiredSum {
                required_sum: expected,
                minimum: tolerance.absolute,
            });
        }
        if !matches!(normalization, Normalization::Normalize) && !tolerance.is_close(sum, expected)
        {
            return Err(CoordinateError::RequiredSumMismatch {
                expected,
                actual: sum,
                absolute: tolerance.absolute,
            });
        }
        Ok(Self {
            weights: weights.map(|value| value / sum),
        })
    }
    pub const fn from_unchecked(weights: [f64; 3]) -> Self {
        Self { weights }
    }
    pub const fn as_array(self) -> [f64; 3] {
        self.weights
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
    pub fn interpolate(self, other: Self, amount: f64) -> Result<Self, CoordinateError> {
        if !amount.is_finite() {
            return Err(CoordinateError::NonFiniteInterpolationParameter { value: amount });
        }
        Ok(Self::from_unchecked(std::array::from_fn(|index| {
            self.weights[index] + (other.weights[index] - self.weights[index]) * amount
        })))
    }
}
impl TryFrom<[f64; 3]> for TernaryPoint {
    type Error = CoordinateError;
    fn try_from(value: [f64; 3]) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
/// Input accepted by embedded-chart series. Arrays are deliberately treated as raw scientific input and are validated during preparation.
pub trait IntoTernaryPoint {
    fn into_ternary_point(self) -> TernaryPoint;
}
impl IntoTernaryPoint for TernaryPoint {
    fn into_ternary_point(self) -> TernaryPoint {
        self
    }
}
impl IntoTernaryPoint for [f64; 3] {
    fn into_ternary_point(self) -> TernaryPoint {
        TernaryPoint::from_unchecked(self)
    }
}
