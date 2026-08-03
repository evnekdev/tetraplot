use super::Tolerance;
use crate::CoordinateError;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Normalization {
    #[default]
    RequireUnitSum,
    Normalize,
    RequireSum(f64),
}
pub(crate) fn validate_domain_weights(
    weights: [f64; 4],
    normalization: Normalization,
    tolerance: Tolerance,
) -> Result<[f64; 4], CoordinateError> {
    validate_weights(weights, normalization, tolerance, true)
}
pub(crate) fn validate_affine_weights(
    weights: [f64; 4],
    normalization: Normalization,
    tolerance: Tolerance,
) -> Result<[f64; 4], CoordinateError> {
    validate_weights(weights, normalization, tolerance, false)
}
fn validate_weights(
    mut weights: [f64; 4],
    normalization: Normalization,
    tolerance: Tolerance,
    require_nonnegative: bool,
) -> Result<[f64; 4], CoordinateError> {
    tolerance.validate()?;
    if let Normalization::RequireSum(required_sum) = normalization
        && (!required_sum.is_finite() || required_sum <= tolerance.absolute)
    {
        return Err(CoordinateError::InvalidRequiredSum {
            required_sum,
            minimum: tolerance.absolute,
        });
    }
    for (index, weight) in weights.iter_mut().enumerate() {
        if !weight.is_finite() {
            return Err(CoordinateError::NonFiniteComponent {
                component: index,
                value: *weight,
            });
        }
        if require_nonnegative && *weight < -tolerance.absolute {
            return Err(CoordinateError::NegativeComponent {
                component: index,
                value: *weight,
                tolerance: tolerance.absolute,
            });
        }
        if require_nonnegative && *weight < 0.0 {
            *weight = 0.0;
        }
    }
    let sum: f64 = weights.iter().sum();
    if !sum.is_finite() || sum.abs() <= tolerance.absolute {
        return Err(CoordinateError::InvalidSum {
            sum,
            minimum: tolerance.absolute,
        });
    }
    match normalization {
        Normalization::Normalize => Ok(weights.map(|weight| weight / sum)),
        Normalization::RequireUnitSum => {
            if !tolerance.is_close(sum, 1.0) {
                return Err(CoordinateError::RequiredSumMismatch {
                    expected: 1.0,
                    actual: sum,
                    absolute: tolerance.absolute,
                });
            }
            Ok(weights.map(|weight| weight / sum))
        }
        Normalization::RequireSum(required_sum) => {
            if !tolerance.is_close(sum, required_sum) {
                return Err(CoordinateError::RequiredSumMismatch {
                    expected: required_sum,
                    actual: sum,
                    absolute: tolerance.absolute,
                });
            }
            Ok(weights.map(|weight| weight / sum))
        }
    }
}
