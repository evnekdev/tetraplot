use crate::ViewportError;
/// Renderer-independent Cartesian scene bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneBounds {
    min: [f64; 3],
    max: [f64; 3],
}
impl SceneBounds {
    pub fn new(min: [f64; 3], max: [f64; 3]) -> Result<Self, ViewportError> {
        if !min.iter().chain(max.iter()).all(|value| value.is_finite()) {
            return Err(ViewportError::NonFiniteBounds { min, max });
        }
        if (0..3).any(|index| min[index] >= max[index]) {
            return Err(ViewportError::InvalidBounds { min, max });
        }
        Ok(Self { min, max })
    }
    pub fn from_points<const N: usize>(points: [[f64; 3]; N]) -> Result<Self, ViewportError> {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for point in points {
            for index in 0..3 {
                min[index] = min[index].min(point[index]);
                max[index] = max[index].max(point[index]);
            }
        }
        Self::new(min, max)
    }
    pub const fn min(self) -> [f64; 3] {
        self.min
    }
    pub const fn max(self) -> [f64; 3] {
        self.max
    }
    pub fn center(self) -> [f64; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }
    pub fn extent(self) -> [f64; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }
    pub fn max_extent(self) -> f64 {
        self.extent().into_iter().fold(0.0, f64::max)
    }
    pub fn union(self, other: Self) -> Self {
        Self {
            min: [
                self.min[0].min(other.min[0]),
                self.min[1].min(other.min[1]),
                self.min[2].min(other.min[2]),
            ],
            max: [
                self.max[0].max(other.max[0]),
                self.max[1].max(other.max[1]),
                self.max[2].max(other.max[2]),
            ],
        }
    }
}
/// How scene fitting treats the output aspect ratio.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ViewportFit {
    #[default]
    PreserveAspect,
    Stretch,
}
/// Placement of a fitted scene within an output allocation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ViewportAlignment {
    #[default]
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}
