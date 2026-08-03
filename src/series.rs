//! Scientific series definitions and renderer-independent preparation.
use crate::{
    Color, CoordinateError, Normalization, SeriesError, TetraGeometry, TetraPoint,
    TetraPointLocation, TetraSegment, Tolerance, clip_segment_with_parameters,
};
/// Explicit response to malformed scientific input.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InvalidPointPolicy {
    #[default]
    Error,
    Skip,
}
/// Whether a series is scientifically clipped to the tetrahedron before rendering.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DomainClip {
    None,
    #[default]
    Tetrahedron,
}
/// How a polyline behaves when invalid points interrupt it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PathBreakPolicy {
    #[default]
    Break,
    Reconnect,
}
/// Renderer-independent colour, geometry, and size of a point marker.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointStyle {
    color: Color,
    size: f32,
}
impl PointStyle {
    pub const fn new(color: Color, size: f32) -> Self {
        Self { color, size }
    }
    pub const fn color(self) -> Color {
        self.color
    }
    pub const fn size(self) -> f32 {
        self.size
    }
}
impl Default for PointStyle {
    fn default() -> Self {
        Self::new(crate::BLACK, 5.0)
    }
}
/// Renderer-independent line appearance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineStyle {
    color: Color,
    width: f32,
}
impl LineStyle {
    pub const fn new(color: Color, width: f32) -> Self {
        Self { color, width }
    }
    pub const fn color(self) -> Color {
        self.color
    }
    pub const fn width(self) -> f32 {
        self.width
    }
}
impl Default for LineStyle {
    fn default() -> Self {
        Self::new(crate::BLUE, 2.0)
    }
}
/// Shading and opacity controls for an arbitrary triangulated surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceStyle {
    color: Color,
    wireframe: bool,
    two_sided: bool,
    smooth: bool,
}
impl SurfaceStyle {
    pub const fn new(color: Color) -> Self {
        Self {
            color,
            wireframe: false,
            two_sided: true,
            smooth: true,
        }
    }
    pub const fn color(self) -> Color {
        self.color
    }
    pub const fn wireframe(self) -> bool {
        self.wireframe
    }
    pub const fn two_sided(self) -> bool {
        self.two_sided
    }
    pub const fn smooth(self) -> bool {
        self.smooth
    }
    pub const fn with_wireframe(mut self, value: bool) -> Self {
        self.wireframe = value;
        self
    }
    pub const fn with_two_sided(mut self, value: bool) -> Self {
        self.two_sided = value;
        self
    }
    pub const fn with_smooth(mut self, value: bool) -> Self {
        self.smooth = value;
        self
    }
}
impl Default for SurfaceStyle {
    fn default() -> Self {
        Self::new(Color::rgb(0.2, 0.45, 0.8).with_alpha(0.65))
    }
}
/// Common preparation policy. Tolerance is deliberately supplied by the plot context.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeriesPolicy {
    normalization: Normalization,
    invalid_points: InvalidPointPolicy,
    clip: DomainClip,
}
impl SeriesPolicy {
    pub const fn new(
        normalization: Normalization,
        invalid_points: InvalidPointPolicy,
        clip: DomainClip,
    ) -> Self {
        Self {
            normalization,
            invalid_points,
            clip,
        }
    }
    pub const fn normalization(self) -> Normalization {
        self.normalization
    }
    pub const fn invalid_points(self) -> InvalidPointPolicy {
        self.invalid_points
    }
    pub const fn clip(self) -> DomainClip {
        self.clip
    }
}
impl Default for SeriesPolicy {
    fn default() -> Self {
        Self::new(
            Normalization::RequireUnitSum,
            InvalidPointPolicy::Error,
            DomainClip::Tetrahedron,
        )
    }
}
/// Context shared by all series preparation operations.
#[derive(Clone, Copy, Debug)]
pub struct PreparationContext {
    pub geometry: TetraGeometry,
    pub tolerance: Tolerance,
}
/// A prepared point retains its scientific coordinate, world coordinate, and source identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedPoint {
    pub world: [f32; 3],
    pub barycentric: [f64; 4],
    pub source_index: usize,
    pub location: TetraPointLocation,
}
/// Prepared disjoint polyline fragments. Clipping and invalid-point boundaries never reconnect fragments.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedPolyline {
    pub paths: Vec<Vec<PreparedPoint>>,
    pub style: LineStyle,
}
/// Prepared indexed surface geometry independent of the graphics backend.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedSurface {
    pub world_vertices: Vec<[f32; 3]>,
    pub barycentric_vertices: Vec<[f64; 4]>,
    pub triangles: Vec<[u32; 3]>,
    pub normals: Option<Vec<[f32; 3]>>,
    pub vertex_values: Option<Vec<f64>>,
    pub face_values: Option<Vec<f64>>,
    pub style: SurfaceStyle,
}
/// A value already prepared for a concrete rendering adapter.
#[derive(Clone, Debug, PartialEq)]
pub enum PreparedSeries {
    Points {
        points: Vec<PreparedPoint>,
        style: PointStyle,
    },
    Line(PreparedPolyline),
    Surface(PreparedSurface),
}
/// Stable identity allocated by a completed plot when a series is drawn.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SeriesId(pub(crate) u64);
impl SeriesId {
    pub const fn get(self) -> u64 {
        self.0
    }
}
/// Scientific input accepted by generic tetrahedral series. Raw arrays are always validated during preparation.
pub trait IntoTetraPoint {
    fn into_tetra_point(self) -> TetraPoint;
}
impl IntoTetraPoint for TetraPoint {
    fn into_tetra_point(self) -> TetraPoint {
        self
    }
}
impl IntoTetraPoint for [f64; 4] {
    fn into_tetra_point(self) -> TetraPoint {
        TetraPoint::from_unchecked(self)
    }
}
/// A source series that prepares itself using the plot's shared scientific context.
pub trait TetraSeries {
    fn prepare(self, context: PreparationContext) -> Result<PreparedSeries, SeriesError>;
}
/// An owned collection of scientific points.
pub struct TetraPointSeries<P> {
    points: Vec<P>,
    style: PointStyle,
    policy: SeriesPolicy,
}
impl<P> TetraPointSeries<P> {
    pub fn new<I>(points: I) -> Self
    where
        I: IntoIterator<Item = P>,
    {
        Self {
            points: points.into_iter().collect(),
            style: PointStyle::default(),
            policy: SeriesPolicy::default(),
        }
    }
    pub const fn size(mut self, size: f32) -> Self {
        self.style = PointStyle::new(self.style.color(), size);
        self
    }
    pub const fn style(mut self, style: PointStyle) -> Self {
        self.style = style;
        self
    }
    pub const fn color(mut self, color: Color) -> Self {
        self.style = PointStyle::new(color, self.style.size());
        self
    }
    pub const fn normalization(mut self, value: Normalization) -> Self {
        self.policy = SeriesPolicy::new(value, self.policy.invalid_points(), self.policy.clip());
        self
    }
    pub const fn invalid_point_policy(mut self, value: InvalidPointPolicy) -> Self {
        self.policy = SeriesPolicy::new(self.policy.normalization(), value, self.policy.clip());
        self
    }
    pub const fn clip(mut self, value: DomainClip) -> Self {
        self.policy = SeriesPolicy::new(
            self.policy.normalization(),
            self.policy.invalid_points(),
            value,
        );
        self
    }
}
impl<P: IntoTetraPoint> TetraSeries for TetraPointSeries<P> {
    fn prepare(self, context: PreparationContext) -> Result<PreparedSeries, SeriesError> {
        let mut points = Vec::with_capacity(self.points.len());
        for (index, input) in self.points.into_iter().enumerate() {
            match prepare_one(input.into_tetra_point(), index, self.policy, context) {
                Ok(Some(point)) => points.push(point),
                Ok(None) => {}
                Err(error) => match self.policy.invalid_points() {
                    InvalidPointPolicy::Error => return Err(error),
                    InvalidPointPolicy::Skip => {}
                },
            }
        }
        Ok(PreparedSeries::Points {
            points,
            style: self.style,
        })
    }
}
/// An ordered tetrahedral curve or polyline.
pub struct TetraLineSeries<P> {
    points: Vec<P>,
    style: LineStyle,
    policy: SeriesPolicy,
    break_policy: PathBreakPolicy,
    closed: bool,
}
impl<P> TetraLineSeries<P> {
    pub fn new<I>(points: I) -> Self
    where
        I: IntoIterator<Item = P>,
    {
        Self {
            points: points.into_iter().collect(),
            style: LineStyle::default(),
            policy: SeriesPolicy::default(),
            break_policy: PathBreakPolicy::Break,
            closed: false,
        }
    }
    pub const fn style(mut self, style: LineStyle) -> Self {
        self.style = style;
        self
    }
    pub const fn width(mut self, width: f32) -> Self {
        self.style = LineStyle::new(self.style.color(), width);
        self
    }
    pub const fn color(mut self, color: Color) -> Self {
        self.style = LineStyle::new(color, self.style.width());
        self
    }
    pub const fn closed(mut self, value: bool) -> Self {
        self.closed = value;
        self
    }
    pub const fn normalization(mut self, value: Normalization) -> Self {
        self.policy = SeriesPolicy::new(value, self.policy.invalid_points(), self.policy.clip());
        self
    }
    pub const fn invalid_point_policy(mut self, value: InvalidPointPolicy) -> Self {
        self.policy = SeriesPolicy::new(self.policy.normalization(), value, self.policy.clip());
        self
    }
    pub const fn path_break_policy(mut self, value: PathBreakPolicy) -> Self {
        self.break_policy = value;
        self
    }
    pub const fn clip(mut self, value: DomainClip) -> Self {
        self.policy = SeriesPolicy::new(
            self.policy.normalization(),
            self.policy.invalid_points(),
            value,
        );
        self
    }
}
impl<P: IntoTetraPoint> TetraSeries for TetraLineSeries<P> {
    fn prepare(self, context: PreparationContext) -> Result<PreparedSeries, SeriesError> {
        let mut runs = Vec::<Vec<(TetraPoint, usize)>>::new();
        let mut current = Vec::new();
        for (index, input) in self.points.into_iter().enumerate() {
            let point = input
                .into_tetra_point()
                .validate_affine(self.policy.normalization(), context.tolerance);
            match point {
                Ok(point) => current.push((point, index)),
                Err(source) => match self.policy.invalid_points() {
                    InvalidPointPolicy::Error => {
                        return Err(SeriesError::InvalidPoint { index, source });
                    }
                    InvalidPointPolicy::Skip => match self.break_policy {
                        PathBreakPolicy::Break => {
                            if !current.is_empty() {
                                runs.push(std::mem::take(&mut current));
                            }
                        }
                        PathBreakPolicy::Reconnect => {}
                    },
                },
            }
        }
        if !current.is_empty() {
            runs.push(current);
        }
        let mut paths = Vec::new();
        for mut run in runs {
            if self.closed && run.len() > 2 {
                let first = run[0];
                run.push(first);
            }
            let mut path = Vec::new();
            for pair in run.windows(2) {
                let segment = TetraSegment::new(pair[0].0, pair[1].0);
                let clipped = match self.policy.clip() {
                    DomainClip::None => Some(crate::ClippedSegment {
                        segment,
                        parameter_start: 0.0,
                        parameter_end: 1.0,
                    }),
                    DomainClip::Tetrahedron => {
                        clip_segment_with_parameters(segment, context.tolerance).map_err(
                            |source| SeriesError::InvalidPoint {
                                index: pair[0].1,
                                source,
                            },
                        )?
                    }
                };
                let Some(clipped) = clipped else {
                    finish_path(&mut path, &mut paths);
                    continue;
                };
                let start = prepared_from_tetra(clipped.segment.start, pair[0].1, context);
                let end = prepared_from_tetra(clipped.segment.end, pair[1].1, context);
                if path.last().is_some_and(|previous: &PreparedPoint| {
                    !close_world(previous.world, start.world, context.tolerance)
                }) {
                    finish_path(&mut path, &mut paths);
                }
                push_distinct(&mut path, start, context.tolerance);
                push_distinct(&mut path, end, context.tolerance);
                if clipped.parameter_end < 1.0 - context.tolerance.absolute {
                    finish_path(&mut path, &mut paths);
                }
            }
            finish_path(&mut path, &mut paths);
        }
        Ok(PreparedSeries::Line(PreparedPolyline {
            paths,
            style: self.style,
        }))
    }
}
/// An arbitrary scientific triangular surface retained for attachments and reuse.
#[derive(Clone, Debug)]
pub struct TetraSurface {
    vertices: Vec<TetraPoint>,
    triangles: Vec<[u32; 3]>,
}
impl TetraSurface {
    pub fn new<P, I>(vertices: I, triangles: Vec<[u32; 3]>) -> Result<Self, SeriesError>
    where
        I: IntoIterator<Item = P>,
        P: IntoTetraPoint,
    {
        let value = Self {
            vertices: vertices
                .into_iter()
                .map(IntoTetraPoint::into_tetra_point)
                .collect(),
            triangles,
        };
        value.validate_topology()?;
        Ok(value)
    }
    pub fn vertices(&self) -> &[TetraPoint] {
        &self.vertices
    }
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
    pub fn evaluate(
        &self,
        point: SurfacePoint,
        tolerance: Tolerance,
    ) -> Result<TetraPoint, SeriesError> {
        let triangle = *self.triangles.get(point.triangle as usize).ok_or(
            SeriesError::TriangleIndexOutOfBounds {
                triangle: point.triangle as usize,
                vertex: point.triangle,
                vertex_count: self.vertices.len(),
            },
        )?;
        let weights = point
            .validate(tolerance)
            .map_err(|source| SeriesError::InvalidPoint {
                index: point.triangle as usize,
                source,
            })?;
        Ok(TetraPoint::from_unchecked([0, 1, 2, 3].map(|index| {
            weights[0] * self.vertices[triangle[0] as usize].as_array()[index]
                + weights[1] * self.vertices[triangle[1] as usize].as_array()[index]
                + weights[2] * self.vertices[triangle[2] as usize].as_array()[index]
        })))
    }
    fn validate_topology(&self) -> Result<(), SeriesError> {
        for (triangle_index, triangle) in self.triangles.iter().copied().enumerate() {
            if triangle[0] == triangle[1]
                || triangle[1] == triangle[2]
                || triangle[0] == triangle[2]
            {
                return Err(SeriesError::RepeatedTriangleIndex {
                    triangle: triangle_index,
                });
            }
            for vertex in triangle {
                if vertex as usize >= self.vertices.len() {
                    return Err(SeriesError::TriangleIndexOutOfBounds {
                        triangle: triangle_index,
                        vertex,
                        vertex_count: self.vertices.len(),
                    });
                }
            }
        }
        Ok(())
    }
}
/// One point pinned to a triangle of a source surface by triangle-local barycentric weights.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfacePoint {
    pub triangle: u32,
    pub weights: [f64; 3],
}
impl SurfacePoint {
    pub fn new(triangle: u32, weights: [f64; 3]) -> Result<Self, CoordinateError> {
        let value = Self { triangle, weights };
        value.validate(Tolerance::default())?;
        Ok(value)
    }
    pub const fn from_unchecked(triangle: u32, weights: [f64; 3]) -> Self {
        Self { triangle, weights }
    }
    pub fn validate(self, tolerance: Tolerance) -> Result<[f64; 3], CoordinateError> {
        tolerance.validate()?;
        for (index, value) in self.weights.iter().enumerate() {
            if !value.is_finite() {
                return Err(CoordinateError::NonFiniteComponent {
                    component: index,
                    value: *value,
                });
            }
            if *value < -tolerance.absolute {
                return Err(CoordinateError::NegativeComponent {
                    component: index,
                    value: *value,
                    tolerance: tolerance.absolute,
                });
            }
        }
        let sum: f64 = self.weights.iter().sum();
        if !tolerance.is_close(sum, 1.0) {
            return Err(CoordinateError::RequiredSumMismatch {
                expected: 1.0,
                actual: sum,
                absolute: tolerance.absolute,
            });
        }
        Ok(self.weights.map(|value| {
            if tolerance.is_near_zero(value) {
                0.0
            } else {
                value / sum
            }
        }))
    }
}
/// An ordered curve retaining its attachment to a source triangulated surface.
#[derive(Clone, Debug)]
pub struct SurfaceCurve {
    pub points: Vec<SurfacePoint>,
}
impl SurfaceCurve {
    pub fn new(points: Vec<SurfacePoint>) -> Self {
        Self { points }
    }
    pub fn evaluate(
        &self,
        surface: &TetraSurface,
        tolerance: Tolerance,
    ) -> Result<Vec<TetraPoint>, SeriesError> {
        self.points
            .iter()
            .copied()
            .map(|point| surface.evaluate(point, tolerance))
            .collect()
    }
}
/// A triangulated tetrahedral surface series.
pub struct TetraSurfaceSeries {
    surface: TetraSurface,
    style: SurfaceStyle,
    policy: SeriesPolicy,
    vertex_values: Option<Vec<f64>>,
    face_values: Option<Vec<f64>>,
}
impl TetraSurfaceSeries {
    pub fn new<P, I>(vertices: I, triangles: Vec<[u32; 3]>) -> Result<Self, SeriesError>
    where
        I: IntoIterator<Item = P>,
        P: IntoTetraPoint,
    {
        Ok(Self {
            surface: TetraSurface::new(vertices, triangles)?,
            style: SurfaceStyle::default(),
            policy: SeriesPolicy::default(),
            vertex_values: None,
            face_values: None,
        })
    }
    pub const fn style(mut self, value: SurfaceStyle) -> Self {
        self.style = value;
        self
    }
    pub const fn opacity(mut self, value: f32) -> Self {
        self.style = SurfaceStyle::new(self.style.color().with_alpha(value))
            .with_wireframe(self.style.wireframe())
            .with_two_sided(self.style.two_sided())
            .with_smooth(self.style.smooth());
        self
    }
    pub const fn normalization(mut self, value: Normalization) -> Self {
        self.policy = SeriesPolicy::new(value, self.policy.invalid_points(), self.policy.clip());
        self
    }
    pub const fn invalid_point_policy(mut self, value: InvalidPointPolicy) -> Self {
        self.policy = SeriesPolicy::new(self.policy.normalization(), value, self.policy.clip());
        self
    }
    pub const fn clip(mut self, value: DomainClip) -> Self {
        self.policy = SeriesPolicy::new(
            self.policy.normalization(),
            self.policy.invalid_points(),
            value,
        );
        self
    }
    pub fn vertex_values(mut self, values: Vec<f64>) -> Result<Self, SeriesError> {
        if values.len() != self.surface.vertices.len() {
            return Err(SeriesError::ScalarValueLength {
                kind: "vertex",
                actual: values.len(),
                expected: self.surface.vertices.len(),
            });
        }
        validate_scalars(&values)?;
        self.vertex_values = Some(values);
        Ok(self)
    }
    pub fn face_values(mut self, values: Vec<f64>) -> Result<Self, SeriesError> {
        if values.len() != self.surface.triangles.len() {
            return Err(SeriesError::ScalarValueLength {
                kind: "face",
                actual: values.len(),
                expected: self.surface.triangles.len(),
            });
        }
        validate_scalars(&values)?;
        self.face_values = Some(values);
        Ok(self)
    }
}
impl TetraSeries for TetraSurfaceSeries {
    fn prepare(self, context: PreparationContext) -> Result<PreparedSeries, SeriesError> {
        let mut world_vertices = Vec::with_capacity(self.surface.vertices.len());
        let mut barycentric_vertices = Vec::with_capacity(self.surface.vertices.len());
        let mut accepted = vec![true; self.surface.vertices.len()];
        for (index, source) in self.surface.vertices.iter().copied().enumerate() {
            let point = source
                .validate_affine(self.policy.normalization(), context.tolerance)
                .map_err(|source| SeriesError::InvalidPoint { index, source })?;
            if self.policy.clip() == DomainClip::Tetrahedron
                && matches!(
                    context.geometry.classify(point, context.tolerance),
                    TetraPointLocation::Outside
                )
            {
                match self.policy.invalid_points() {
                    InvalidPointPolicy::Error => return Err(SeriesError::OutsideDomain { index }),
                    InvalidPointPolicy::Skip => accepted[index] = false,
                }
            }
            barycentric_vertices.push(point.as_array());
            world_vertices.push(context.geometry.to_world(point).map(|value| value as f32));
        }
        let mut triangles = Vec::new();
        for (index, triangle) in self.surface.triangles.iter().copied().enumerate() {
            if !accepted[triangle[0] as usize]
                || !accepted[triangle[1] as usize]
                || !accepted[triangle[2] as usize]
            {
                if self.policy.invalid_points() == InvalidPointPolicy::Error {
                    return Err(SeriesError::SurfaceClippingDeferred);
                }
                continue;
            }
            let a = world_vertices[triangle[0] as usize];
            let b = world_vertices[triangle[1] as usize];
            let c = world_vertices[triangle[2] as usize];
            if triangle_area(a, b, c) <= context.tolerance.absolute as f32 {
                return Err(SeriesError::DegenerateTriangle { triangle: index });
            }
            triangles.push(triangle);
        }
        let normals = self
            .style
            .smooth()
            .then(|| vertex_normals(&world_vertices, &triangles));
        Ok(PreparedSeries::Surface(PreparedSurface {
            world_vertices,
            barycentric_vertices,
            triangles,
            normals,
            vertex_values: self.vertex_values,
            face_values: self.face_values,
            style: self.style,
        }))
    }
}
fn prepare_one(
    point: TetraPoint,
    index: usize,
    policy: SeriesPolicy,
    context: PreparationContext,
) -> Result<Option<PreparedPoint>, SeriesError> {
    let point = point
        .validate_affine(policy.normalization(), context.tolerance)
        .map_err(|source| SeriesError::InvalidPoint { index, source })?;
    let location = context.geometry.classify(point, context.tolerance);
    if policy.clip() == DomainClip::Tetrahedron && location == TetraPointLocation::Outside {
        return match policy.invalid_points() {
            InvalidPointPolicy::Error => Err(SeriesError::OutsideDomain { index }),
            InvalidPointPolicy::Skip => Ok(None),
        };
    }
    Ok(Some(prepared_from_tetra(point, index, context)))
}
fn prepared_from_tetra(
    point: TetraPoint,
    source_index: usize,
    context: PreparationContext,
) -> PreparedPoint {
    PreparedPoint {
        world: context.geometry.to_world(point).map(|value| value as f32),
        barycentric: point.as_array(),
        source_index,
        location: context.geometry.classify(point, context.tolerance),
    }
}
fn finish_path(current: &mut Vec<PreparedPoint>, paths: &mut Vec<Vec<PreparedPoint>>) {
    if !current.is_empty() {
        paths.push(std::mem::take(current));
    }
}
fn push_distinct(path: &mut Vec<PreparedPoint>, point: PreparedPoint, tolerance: Tolerance) {
    if path
        .last()
        .is_none_or(|last| !close_world(last.world, point.world, tolerance))
    {
        path.push(point);
    }
}
fn close_world(left: [f32; 3], right: [f32; 3], tolerance: Tolerance) -> bool {
    tolerance.is_close(f64::from(left[0]), f64::from(right[0]))
        && tolerance.is_close(f64::from(left[1]), f64::from(right[1]))
        && tolerance.is_close(f64::from(left[2]), f64::from(right[2]))
}
fn validate_scalars(values: &[f64]) -> Result<(), SeriesError> {
    for (index, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(SeriesError::NonFiniteScalar {
                index,
                value: *value,
            });
        }
    }
    Ok(())
}
fn triangle_area(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let cross = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    0.5 * (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt()
}
fn vertex_normals(vertices: &[[f32; 3]], triangles: &[[u32; 3]]) -> Vec<[f32; 3]> {
    let mut result = vec![[0.0; 3]; vertices.len()];
    for triangle in triangles {
        let a = vertices[triangle[0] as usize];
        let b = vertices[triangle[1] as usize];
        let c = vertices[triangle[2] as usize];
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let normal = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for index in *triangle {
            result[index as usize][0] += normal[0];
            result[index as usize][1] += normal[1];
            result[index as usize][2] += normal[2];
        }
    }
    result
        .into_iter()
        .map(|normal| {
            let length =
                (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
            if length > f32::EPSILON {
                [normal[0] / length, normal[1] / length, normal[2] / length]
            } else {
                [0.0, 0.0, 1.0]
            }
        })
        .collect()
}
