use super::{Normalization, TetraPoint, Tolerance};
use crate::CoordinateError;
/// A directed segment expressed in scientific tetrahedral barycentric coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TetraSegment {
    pub start: TetraPoint,
    pub end: TetraPoint,
}
impl TetraSegment {
    pub const fn new(start: TetraPoint, end: TetraPoint) -> Self {
        Self { start, end }
    }
    pub fn point_at(self, parameter: f64) -> Result<TetraPoint, CoordinateError> {
        self.start.interpolate(self.end, parameter)
    }
}
/// A clipped segment retaining source parameter values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClippedSegment {
    pub segment: TetraSegment,
    pub parameter_start: f64,
    pub parameter_end: f64,
}
/// Clip a segment to the closed tetrahedron using its four linear barycentric inequalities.
pub fn clip_segment(
    segment: TetraSegment,
    tolerance: Tolerance,
) -> Result<Option<TetraSegment>, CoordinateError> {
    Ok(clip_segment_with_parameters(segment, tolerance)?.map(|result| result.segment))
}
/// Clip a segment and preserve its parameter interval in the original directed segment.
pub fn clip_segment_with_parameters(
    segment: TetraSegment,
    tolerance: Tolerance,
) -> Result<Option<ClippedSegment>, CoordinateError> {
    let start = segment
        .start
        .validate_affine(Normalization::RequireUnitSum, tolerance)?;
    let end = segment
        .end
        .validate_affine(Normalization::RequireUnitSum, tolerance)?;
    let mut lower = 0.0_f64;
    let mut upper = 1.0_f64;
    for index in 0..4 {
        let a = start.as_array()[index];
        let delta = end.as_array()[index] - a;
        if delta.abs() <= tolerance.absolute {
            if a < -tolerance.absolute {
                return Ok(None);
            }
            continue;
        }
        let crossing = (-tolerance.absolute - a) / delta;
        if delta > 0.0 {
            lower = lower.max(crossing);
        } else {
            upper = upper.min(crossing);
        }
    }
    lower = lower.max(0.0);
    upper = upper.min(1.0);
    if lower > upper + tolerance.absolute {
        return Ok(None);
    }
    let lower = lower.clamp(0.0, 1.0);
    let upper = upper.clamp(0.0, 1.0);
    let snap = |point: TetraPoint| {
        TetraPoint::from_unchecked(point.as_array().map(|weight| {
            if tolerance.is_near_zero(weight) {
                0.0
            } else {
                weight
            }
        }))
    };
    Ok(Some(ClippedSegment {
        segment: TetraSegment::new(
            snap(start.interpolate(end, lower)?),
            snap(start.interpolate(end, upper)?),
        ),
        parameter_start: lower,
        parameter_end: upper,
    }))
}
