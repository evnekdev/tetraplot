use crate::CoordinateError;

/// Central absolute and relative numerical tolerances.
///
/// Absolute tolerance is used for simplex boundaries and near-zero quantities;
/// relative tolerance is used when comparing values away from zero. Operations
/// document which term is relevant, rather than scattering numeric constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    /// Absolute tolerance used near zero.
    pub absolute: f64,
    /// Relative tolerance scaled by the compared magnitudes.
    pub relative: f64,
}

impl Tolerance {
    /// Construct a usable tolerance.
    pub fn new(absolute: f64, relative: f64) -> Result<Self, CoordinateError> {
        let tolerance = Self { absolute, relative };
        tolerance.validate()?;
        Ok(tolerance)
    }

    /// Validate values, including those made with public fields.
    pub fn validate(self) -> Result<(), CoordinateError> {
        if !self.absolute.is_finite()
            || !self.relative.is_finite()
            || self.absolute <= 0.0
            || self.relative <= 0.0
        {
            return Err(CoordinateError::InvalidTolerance {
                absolute: self.absolute,
                relative: self.relative,
            });
        }
        Ok(())
    }

    /// Compare two finite values with combined absolute and relative tolerance.
    pub fn is_close(self, left: f64, right: f64) -> bool {
        (left - right).abs() <= self.absolute + self.relative * left.abs().max(right.abs())
    }

    /// Return whether a value is within absolute tolerance of zero.
    pub fn is_near_zero(self, value: f64) -> bool {
        value.abs() <= self.absolute
    }
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            absolute: 1.0e-12,
            relative: 1.0e-12,
        }
    }
}
