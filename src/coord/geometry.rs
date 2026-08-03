use super::{SceneBounds, TetraPoint, Tolerance};
use crate::{CoordinateError, GeometryError};
/// A semantic tetrahedron component in stable scientific order.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(usize)]
pub enum Component {
    A = 0,
    B = 1,
    C = 2,
    D = 3,
}
impl Component {
    pub const ALL: [Self; 4] = [Self::A, Self::B, Self::C, Self::D];
    pub const fn index(self) -> usize {
        self as usize
    }
    pub fn from_index(index: usize) -> Result<Self, CoordinateError> {
        match index {
            0 => Ok(Self::A),
            1 => Ok(Self::B),
            2 => Ok(Self::C),
            3 => Ok(Self::D),
            _ => Err(CoordinateError::InvalidComponentIndex { index }),
        }
    }
    pub const fn others(self) -> [Self; 3] {
        match self {
            Self::A => [Self::B, Self::C, Self::D],
            Self::B => [Self::A, Self::C, Self::D],
            Self::C => [Self::A, Self::B, Self::D],
            Self::D => [Self::A, Self::B, Self::C],
        }
    }
}
/// One of the six canonical tetrahedron edges.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Edge {
    AB,
    AC,
    AD,
    BC,
    BD,
    CD,
}
impl Edge {
    pub const ALL: [Self; 6] = [Self::AB, Self::AC, Self::AD, Self::BC, Self::BD, Self::CD];
    pub const fn endpoints(self) -> [Component; 2] {
        match self {
            Self::AB => [Component::A, Component::B],
            Self::AC => [Component::A, Component::C],
            Self::AD => [Component::A, Component::D],
            Self::BC => [Component::B, Component::C],
            Self::BD => [Component::B, Component::D],
            Self::CD => [Component::C, Component::D],
        }
    }
    pub const fn from_components(left: Component, right: Component) -> Self {
        match (left, right) {
            (Component::A, Component::B) | (Component::B, Component::A) => Self::AB,
            (Component::A, Component::C) | (Component::C, Component::A) => Self::AC,
            (Component::A, Component::D) | (Component::D, Component::A) => Self::AD,
            (Component::B, Component::C) | (Component::C, Component::B) => Self::BC,
            (Component::B, Component::D) | (Component::D, Component::B) => Self::BD,
            (Component::C, Component::D) | (Component::D, Component::C) => Self::CD,
            _ => unreachable!(),
        }
    }
}
/// A face identified by its opposite tetrahedron component.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Face {
    OppositeA,
    OppositeB,
    OppositeC,
    OppositeD,
}
impl Face {
    pub const ALL: [Self; 4] = [
        Self::OppositeA,
        Self::OppositeB,
        Self::OppositeC,
        Self::OppositeD,
    ];
    pub const fn opposite(self) -> Component {
        match self {
            Self::OppositeA => Component::A,
            Self::OppositeB => Component::B,
            Self::OppositeC => Component::C,
            Self::OppositeD => Component::D,
        }
    }
    pub const fn from_opposite(component: Component) -> Self {
        match component {
            Component::A => Self::OppositeA,
            Component::B => Self::OppositeB,
            Component::C => Self::OppositeC,
            Component::D => Self::OppositeD,
        }
    }
}
/// Orientation derived from the signed tetrahedron volume.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Handedness {
    Right,
    Left,
}
/// A point's location relative to the scientific tetrahedral domain.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TetraPointLocation {
    Interior,
    Face(Face),
    Edge(Edge),
    Vertex(Component),
    Outside,
}
/// A renderer-independent tetrahedron with vertices in A/B/C/D order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TetraGeometry {
    vertices: [[f64; 3]; 4],
}
impl Default for TetraGeometry {
    fn default() -> Self {
        Self {
            vertices: [
                [1.0, 1.0, 1.0],
                [1.0, -1.0, -1.0],
                [-1.0, 1.0, -1.0],
                [-1.0, -1.0, 1.0],
            ],
        }
    }
}
impl TetraGeometry {
    pub fn new(vertices: [[f64; 3]; 4], tolerance: Tolerance) -> Result<Self, GeometryError> {
        tolerance
            .validate()
            .map_err(|_| GeometryError::DegenerateOperation)?;
        for (vertex, value) in vertices.iter().enumerate() {
            if !value.iter().all(|value| value.is_finite()) {
                return Err(GeometryError::NonFiniteVertex {
                    vertex,
                    value: *value,
                });
            }
        }
        let geometry = Self { vertices };
        let signed_volume = geometry.signed_volume();
        let extent = geometry.bounds_unchecked().max_extent().max(1.0);
        let minimum_volume = tolerance.absolute * extent.powi(3);
        if signed_volume.abs() <= minimum_volume {
            return Err(GeometryError::DegenerateTetrahedron {
                signed_volume,
                minimum_volume,
            });
        }
        Ok(geometry)
    }
    pub const fn vertices(self) -> [[f64; 3]; 4] {
        self.vertices
    }
    pub const fn vertex(self, component: Component) -> [f64; 3] {
        self.vertices[component.index()]
    }
    pub fn signed_volume(self) -> f64 {
        determinant(
            sub(self.vertices[0], self.vertices[3]),
            sub(self.vertices[1], self.vertices[3]),
            sub(self.vertices[2], self.vertices[3]),
        ) / 6.0
    }
    pub fn handedness(self) -> Handedness {
        if self.signed_volume() >= 0.0 {
            Handedness::Right
        } else {
            Handedness::Left
        }
    }
    pub fn center(self) -> [f64; 3] {
        scale(self.vertices.into_iter().fold([0.0; 3], add), 0.25)
    }
    pub fn bounds(self) -> SceneBounds {
        self.bounds_unchecked()
    }
    fn bounds_unchecked(self) -> SceneBounds {
        SceneBounds::from_points(self.vertices).expect("finite default geometry")
    }
    pub fn edge(self, edge: Edge) -> [[f64; 3]; 2] {
        let [first, second] = edge.endpoints();
        [self.vertex(first), self.vertex(second)]
    }
    /// Return vertices in outward winding. `face[0] -> face[1] -> face[2]` follows its normal.
    pub fn face(self, face: Face) -> [usize; 3] {
        let other = face.opposite().others();
        let mut result = [other[0].index(), other[1].index(), other[2].index()];
        let a = self.vertices[result[0]];
        let b = self.vertices[result[1]];
        let c = self.vertices[result[2]];
        let inward = dot(
            cross(sub(b, a), sub(c, a)),
            sub(self.vertex(face.opposite()), a),
        );
        if inward > 0.0 {
            result.swap(1, 2);
        }
        result
    }
    pub fn face_normal(self, face: Face) -> Result<[f64; 3], GeometryError> {
        let [a, b, c] = self.face(face);
        normalize(cross(
            sub(self.vertices[b], self.vertices[a]),
            sub(self.vertices[c], self.vertices[a]),
        ))
        .ok_or(GeometryError::DegenerateFace {
            face: face.opposite().index(),
        })
    }
    pub fn to_world(self, point: TetraPoint) -> [f64; 3] {
        point
            .as_array()
            .into_iter()
            .zip(self.vertices)
            .fold([0.0; 3], |sum, (weight, vertex)| {
                add(sum, scale(vertex, weight))
            })
    }
    pub fn from_world(
        self,
        point: [f64; 3],
        tolerance: Tolerance,
    ) -> Result<TetraPoint, CoordinateError> {
        if !point.iter().all(|value| value.is_finite()) {
            return Err(CoordinateError::CartesianOutsideTetrahedron { point });
        }
        let base = self.vertices[3];
        let a = sub(self.vertices[0], base);
        let b = sub(self.vertices[1], base);
        let c = sub(self.vertices[2], base);
        let volume_determinant = determinant(a, b, c);
        if volume_determinant.abs() <= tolerance.absolute {
            return Err(CoordinateError::CartesianOutsideTetrahedron { point });
        }
        let q = sub(point, base);
        let weights = [
            determinant(q, b, c) / volume_determinant,
            determinant(a, q, c) / volume_determinant,
            determinant(a, b, q) / volume_determinant,
            0.0,
        ];
        let weights = [
            weights[0],
            weights[1],
            weights[2],
            1.0 - weights[0] - weights[1] - weights[2],
        ];
        if self.classify_weights(weights, tolerance) == TetraPointLocation::Outside {
            return Err(CoordinateError::CartesianOutsideTetrahedron { point });
        }
        let cleaned = weights.map(|weight| {
            if tolerance.is_near_zero(weight) {
                0.0
            } else {
                weight
            }
        });
        TetraPoint::from_unchecked(cleaned)
            .validate(super::Normalization::RequireUnitSum, tolerance)
    }
    pub fn classify(self, point: TetraPoint, tolerance: Tolerance) -> TetraPointLocation {
        self.classify_weights(point.as_array(), tolerance)
    }
    pub fn classify_weights(self, weights: [f64; 4], tolerance: Tolerance) -> TetraPointLocation {
        if !weights.iter().all(|weight| weight.is_finite())
            || weights.iter().any(|weight| *weight < -tolerance.absolute)
        {
            return TetraPointLocation::Outside;
        }
        let zero: Vec<_> = weights
            .iter()
            .enumerate()
            .filter_map(|(index, weight)| tolerance.is_near_zero(*weight).then_some(index))
            .collect();
        match zero.as_slice() {
            [] => TetraPointLocation::Interior,
            [face] => TetraPointLocation::Face(Face::from_opposite(
                Component::from_index(*face).expect("fixed index"),
            )),
            [first, second] => {
                let remaining: Vec<_> = (0..4)
                    .filter(|index| *index != *first && *index != *second)
                    .collect();
                TetraPointLocation::Edge(Edge::from_components(
                    Component::from_index(remaining[0]).expect("fixed index"),
                    Component::from_index(remaining[1]).expect("fixed index"),
                ))
            }
            [first, second, third] => {
                let vertex = (0..4)
                    .find(|index| *index != *first && *index != *second && *index != *third)
                    .expect("unit-sum point has a vertex");
                TetraPointLocation::Vertex(Component::from_index(vertex).expect("fixed index"))
            }
            _ => TetraPointLocation::Outside,
        }
    }
}
pub(crate) fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(crate) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn scale(a: [f64; 3], factor: f64) -> [f64; 3] {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}
pub(crate) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub(crate) fn determinant(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    dot(a, cross(b, c))
}
pub(crate) fn normalize(vector: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(vector, vector).sqrt();
    (length > f64::EPSILON).then_some(scale(vector, 1.0 / length))
}
