use super::{ChartEmbedding, PlanarEmbedding, TernaryPoint};
use crate::{Component, SectionError, TetraGeometry, TetraPoint, Tolerance};
/// Stable identity for a planar section managed by a [`Tetraplot`](crate::Tetraplot).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SectionId(u64);
impl SectionId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}
/// Provenance of a plane-intersection vertex.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntersectionProvenance {
    TetrahedronVertex(Component),
    TetrahedronEdge(crate::Edge),
    Derived,
}
/// One plane-intersection vertex retaining scientific and world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionVertex {
    pub tetrahedral: TetraPoint,
    pub world: [f64; 3],
    pub provenance: IntersectionProvenance,
}
/// A triangular plane?tetrahedron intersection in consistently ordered winding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionTriangle {
    pub vertices: [SectionVertex; 3],
}
/// A quadrilateral plane?tetrahedron intersection in consistently ordered winding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionQuadrilateral {
    pub vertices: [SectionVertex; 4],
}
/// The complete, explicit topology of a plane?tetrahedron intersection.
#[derive(Clone, Debug, PartialEq)]
pub enum SectionIntersection {
    Empty,
    Point(SectionVertex),
    Segment([SectionVertex; 2]),
    Triangle(SectionTriangle),
    Quadrilateral(SectionQuadrilateral),
}
/// User-facing plane definitions. They are converted to one local evaluation form for intersection calculations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SectionPlane {
    ConstantComponent { component: Component, value: f64 },
    Barycentric { coefficients: [f64; 4], value: f64 },
    Cartesian { normal: [f64; 3], offset: f64 },
}
impl SectionPlane {
    pub fn constant_component(component: usize, value: f64) -> Result<Self, SectionError> {
        Ok(Self::ConstantComponent {
            component: Component::from_index(component)?,
            value,
        })
    }
    pub fn validate(
        self,
        geometry: &TetraGeometry,
        tolerance: Tolerance,
    ) -> Result<(), SectionError> {
        tolerance.validate()?;
        match self {
            Self::ConstantComponent { value, .. } => {
                if !value.is_finite()
                    || value < -tolerance.absolute
                    || value > 1.0 + tolerance.absolute
                {
                    return Err(SectionError::InvalidConstantComponent { value });
                }
            }
            Self::Barycentric {
                coefficients,
                value,
            } => {
                if !value.is_finite() {
                    return Err(SectionError::NonFinitePlaneCoefficient { index: 4, value });
                }
                for (index, coefficient) in coefficients.iter().enumerate() {
                    if !coefficient.is_finite() {
                        return Err(SectionError::NonFinitePlaneCoefficient {
                            index,
                            value: *coefficient,
                        });
                    }
                }
                if coefficients
                    .iter()
                    .all(|coefficient| tolerance.is_near_zero(*coefficient))
                {
                    return Err(SectionError::DegeneratePlane);
                }
            }
            Self::Cartesian { normal, offset } => {
                if !offset.is_finite() {
                    return Err(SectionError::NonFinitePlaneCoefficient {
                        index: 3,
                        value: offset,
                    });
                }
                if !normal.iter().all(|value| value.is_finite()) {
                    return Err(SectionError::NonFinitePlaneCoefficient {
                        index: 0,
                        value: normal[0],
                    });
                }
                if normal
                    .into_iter()
                    .map(|value| value * value)
                    .sum::<f64>()
                    .sqrt()
                    <= tolerance.absolute
                {
                    return Err(SectionError::DegeneratePlane);
                }
            }
        };
        let values = Component::ALL.map(|component| {
            self.evaluate(
                TetraPoint::from_unchecked(unit_component(component)),
                geometry,
            )
        });
        if values.iter().all(|value| tolerance.is_near_zero(*value)) {
            return Err(SectionError::DegeneratePlane);
        }
        Ok(())
    }
    pub fn intersection(
        self,
        geometry: &TetraGeometry,
        tolerance: Tolerance,
    ) -> Result<SectionIntersection, SectionError> {
        self.validate(geometry, tolerance)?;
        let mut vertices = Vec::<SectionVertex>::new();
        for component in Component::ALL {
            let point = TetraPoint::from_unchecked(unit_component(component));
            if tolerance.is_near_zero(self.evaluate(point, geometry)) {
                push_unique(
                    &mut vertices,
                    SectionVertex {
                        tetrahedral: point,
                        world: geometry.to_world(point),
                        provenance: IntersectionProvenance::TetrahedronVertex(component),
                    },
                    tolerance,
                );
            }
        }
        for edge in crate::Edge::ALL {
            let [first, second] = edge.endpoints();
            let start = TetraPoint::from_unchecked(unit_component(first));
            let end = TetraPoint::from_unchecked(unit_component(second));
            let left = self.evaluate(start, geometry);
            let right = self.evaluate(end, geometry);
            if left * right < -tolerance.absolute * tolerance.absolute {
                let amount = left / (left - right);
                let point = start.interpolate(end, amount)?;
                push_unique(
                    &mut vertices,
                    SectionVertex {
                        tetrahedral: point,
                        world: geometry.to_world(point),
                        provenance: IntersectionProvenance::TetrahedronEdge(edge),
                    },
                    tolerance,
                );
            }
        }
        order_vertices(&mut vertices);
        match vertices.len() {
            0 => Ok(SectionIntersection::Empty),
            1 => Ok(SectionIntersection::Point(vertices[0])),
            2 => Ok(SectionIntersection::Segment([vertices[0], vertices[1]])),
            3 => Ok(SectionIntersection::Triangle(SectionTriangle {
                vertices: [vertices[0], vertices[1], vertices[2]],
            })),
            4 => Ok(SectionIntersection::Quadrilateral(SectionQuadrilateral {
                vertices: [vertices[0], vertices[1], vertices[2], vertices[3]],
            })),
            _ => Err(SectionError::DegeneratePlane),
        }
    }
    fn evaluate(self, point: TetraPoint, geometry: &TetraGeometry) -> f64 {
        match self {
            Self::ConstantComponent { component, value } => point.weight(component) - value,
            Self::Barycentric {
                coefficients,
                value,
            } => {
                point
                    .as_array()
                    .into_iter()
                    .zip(coefficients)
                    .map(|(left, right)| left * right)
                    .sum::<f64>()
                    - value
            }
            Self::Cartesian { normal, offset } => {
                geometry
                    .to_world(point)
                    .into_iter()
                    .zip(normal)
                    .map(|(left, right)| left * right)
                    .sum::<f64>()
                    - offset
            }
        }
    }
}
/// Local-to-parent transform for a triangular planar section.
pub type SectionTransform = PlanarEmbedding;
/// A stable local series identity within an embedded ternary diagram.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SectionSeriesId(u64);
/// Local chart contents independent from its planar or curved embedding.
#[derive(Clone, Debug, Default)]
pub struct TernaryDiagram {
    series: Vec<DiagramSeries>,
    next_id: u64,
    revision: u64,
}
#[derive(Clone, Debug)]
pub enum DiagramSeries {
    Points {
        id: SectionSeriesId,
        points: Vec<TernaryPoint>,
    },
    Line {
        id: SectionSeriesId,
        points: Vec<TernaryPoint>,
        closed: bool,
    },
}
impl TernaryDiagram {
    pub fn add_points<P, I>(&mut self, points: I) -> SectionSeriesId
    where
        I: IntoIterator<Item = P>,
        P: super::IntoTernaryPoint,
    {
        let id = SectionSeriesId(self.next_id);
        self.next_id += 1;
        self.series.push(DiagramSeries::Points {
            id,
            points: points
                .into_iter()
                .map(super::IntoTernaryPoint::into_ternary_point)
                .collect(),
        });
        self.revision += 1;
        id
    }
    pub fn add_line<P, I>(&mut self, points: I, closed: bool) -> SectionSeriesId
    where
        I: IntoIterator<Item = P>,
        P: super::IntoTernaryPoint,
    {
        let id = SectionSeriesId(self.next_id);
        self.next_id += 1;
        self.series.push(DiagramSeries::Line {
            id,
            points: points
                .into_iter()
                .map(super::IntoTernaryPoint::into_ternary_point)
                .collect(),
            closed,
        });
        self.revision += 1;
        id
    }
    pub fn series(&self) -> &[DiagramSeries] {
        &self.series
    }
    pub fn remove_series(&mut self, id: SectionSeriesId) -> Option<DiagramSeries> {
        let index = self.series.iter().position(|series| match series {
            DiagramSeries::Points { id: current, .. } | DiagramSeries::Line { id: current, .. } => {
                *current == id
            }
        })?;
        self.revision += 1;
        Some(self.series.remove(index))
    }
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}
/// Appearance and overlay controls shared by planar and curved embedded charts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmbeddedChartStyle {
    pub fill_opacity: f32,
    pub boundary_visible: bool,
    pub grid_visible: bool,
    pub overlay_depth_bias: f32,
    pub two_sided: bool,
}
impl Default for EmbeddedChartStyle {
    fn default() -> Self {
        Self {
            fill_opacity: 0.2,
            boundary_visible: true,
            grid_visible: false,
            overlay_depth_bias: 1.0e-4,
            two_sided: true,
        }
    }
}
/// Editable planar section state. Its intersection and affine embedding are always refreshed together.
#[derive(Clone, Debug)]
pub struct PlanarSection {
    id: Option<SectionId>,
    name: Option<String>,
    plane: SectionPlane,
    intersection: SectionIntersection,
    embedding: Option<PlanarEmbedding>,
    chart: TernaryDiagram,
    style: EmbeddedChartStyle,
    visible: bool,
    geometry_revision: u64,
    chart_revision: u64,
    style_revision: u64,
}
impl PlanarSection {
    pub fn constant_component(component: usize, value: f64) -> Result<Self, SectionError> {
        Ok(Self {
            id: None,
            name: None,
            plane: SectionPlane::constant_component(component, value)?,
            intersection: SectionIntersection::Empty,
            embedding: None,
            chart: TernaryDiagram::default(),
            style: EmbeddedChartStyle::default(),
            visible: true,
            geometry_revision: 0,
            chart_revision: 0,
            style_revision: 0,
        })
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
    pub fn fill_opacity(mut self, opacity: f32) -> Self {
        self.style.fill_opacity = opacity;
        self.style_revision += 1;
        self
    }
    pub fn show_grid(mut self, visible: bool) -> Self {
        self.style.grid_visible = visible;
        self.style_revision += 1;
        self
    }
    pub fn id(&self) -> Option<SectionId> {
        self.id
    }
    pub fn plane(&self) -> SectionPlane {
        self.plane
    }
    pub fn intersection(&self) -> &SectionIntersection {
        &self.intersection
    }
    pub fn chart(&self) -> &TernaryDiagram {
        &self.chart
    }
    pub fn chart_mut(&mut self) -> &mut TernaryDiagram {
        self.chart_revision += 1;
        &mut self.chart
    }
    pub fn style(&self) -> EmbeddedChartStyle {
        self.style
    }
    pub fn visible(&self) -> bool {
        self.visible
    }
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        self.style_revision += 1;
    }
    pub fn set_plane(
        &mut self,
        plane: SectionPlane,
        geometry: &TetraGeometry,
        tolerance: Tolerance,
    ) -> Result<(), SectionError> {
        self.plane = plane;
        self.refresh(geometry, tolerance)
    }
    pub fn set_constant_component_value(
        &mut self,
        value: f64,
        geometry: &TetraGeometry,
        tolerance: Tolerance,
    ) -> Result<(), SectionError> {
        let component = match self.plane {
            SectionPlane::ConstantComponent { component, .. } => component,
            _ => return Err(SectionError::ChartRequiresTriangle),
        };
        self.plane = SectionPlane::ConstantComponent { component, value };
        self.refresh(geometry, tolerance)
    }
    pub fn refresh(
        &mut self,
        geometry: &TetraGeometry,
        tolerance: Tolerance,
    ) -> Result<(), SectionError> {
        self.intersection = self.plane.intersection(geometry, tolerance)?;
        self.embedding = match self.intersection {
            SectionIntersection::Triangle(triangle) => Some(PlanarEmbedding::new(
                triangle.vertices.map(|vertex| vertex.tetrahedral),
                tolerance,
            )?),
            _ => None,
        };
        self.geometry_revision += 1;
        Ok(())
    }
    pub fn embedding(&self) -> Result<ChartEmbedding, SectionError> {
        self.embedding
            .map(ChartEmbedding::Planar)
            .ok_or(SectionError::ChartRequiresTriangle)
    }
    pub(crate) fn assign_id(&mut self, id: SectionId) {
        self.id = Some(id);
    }
}
fn unit_component(component: Component) -> [f64; 4] {
    let mut value = [0.0; 4];
    value[component.index()] = 1.0;
    value
}
fn push_unique(vertices: &mut Vec<SectionVertex>, vertex: SectionVertex, tolerance: Tolerance) {
    if vertices.iter().all(|current| {
        current
            .tetrahedral
            .as_array()
            .into_iter()
            .zip(vertex.tetrahedral.as_array())
            .any(|(left, right)| !tolerance.is_close(left, right))
    }) {
        vertices.push(vertex);
    }
}
fn order_vertices(vertices: &mut [SectionVertex]) {
    if vertices.len() < 3 {
        return;
    }
    let center = vertices
        .iter()
        .fold([0.0; 3], |sum, vertex| {
            [
                sum[0] + vertex.world[0],
                sum[1] + vertex.world[1],
                sum[2] + vertex.world[2],
            ]
        })
        .map(|value| value / vertices.len() as f64);
    let normal = cross(
        sub(vertices[1].world, vertices[0].world),
        sub(vertices[2].world, vertices[0].world),
    );
    let axis = normalize(sub(vertices[0].world, center)).unwrap_or([1.0, 0.0, 0.0]);
    let other = cross(normal, axis);
    vertices.sort_by(|left, right| {
        let l = sub(left.world, center);
        let r = sub(right.world, center);
        let la = dot(l, other).atan2(dot(l, axis));
        let ra = dot(r, other).atan2(dot(r, axis));
        la.total_cmp(&ra)
    });
}
fn sub(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}
fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}
fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}
fn normalize(value: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(value, value).sqrt();
    (length > f64::EPSILON).then_some(value.map(|part| part / length))
}
