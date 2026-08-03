use super::TernaryPoint;
use crate::{Color, SectionError, SurfaceOverlayMode, TetraGeometry, TetraPoint, Tolerance};
use std::collections::BTreeMap;
/// Stable index of a vertex in a triangulated embedding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfaceVertexIndex(pub u32);
/// Stable scientific patch identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfacePatchId(u64);
impl SurfacePatchId {
    pub const fn get(self) -> u64 {
        self.0
    }
}
/// Stable declared break-line identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BreakLineId(u64);
impl BreakLineId {
    pub const fn get(self) -> u64 {
        self.0
    }
}
/// One connected region where smooth interpolation is permitted.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfacePatch {
    pub id: SurfacePatchId,
    pub triangles: Vec<u32>,
    pub name: Option<String>,
}
/// Classification of a chart-surface edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceEdgeKind {
    Smooth,
    BreakLine(BreakLineId),
    Boundary(u8),
}
/// Authoritative scientific meaning of a declared break line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BreakLineKind {
    Crease,
    Univariant,
    PhaseBoundary,
    UserDefined,
}
/// Visual style of a declared break line; all offsets are renderer-only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreakLineStyle {
    pub visible: bool,
    pub width: f32,
    pub color: Color,
    pub overlay_mode: SurfaceOverlayMode,
}
impl Default for BreakLineStyle {
    fn default() -> Self {
        Self {
            visible: true,
            width: 1.5,
            color: Color::rgb(0.15, 0.15, 0.15),
            overlay_mode: SurfaceOverlayMode::DepthBias,
        }
    }
}
/// A graph-compatible break-line arc. The initial API accepts an ordered mesh-edge chain.
#[derive(Clone, Debug, PartialEq)]
pub struct BreakLine {
    pub id: BreakLineId,
    pub vertices: Vec<SurfaceVertexIndex>,
    pub adjacent_patches: [Option<SurfacePatchId>; 2],
    pub kind: BreakLineKind,
    pub style: BreakLineStyle,
}
/// Explicit corners and boundary chains of the reference ternary chart.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceTopology {
    pub corners: [SurfaceVertexIndex; 3],
    pub boundary_chains: [Vec<SurfaceVertexIndex>; 3],
}
impl SurfaceTopology {
    pub fn new(
        corners: [SurfaceVertexIndex; 3],
        boundary_chains: [Vec<SurfaceVertexIndex>; 3],
    ) -> Self {
        Self {
            corners,
            boundary_chains,
        }
    }
}
/// Normal interpolation policy that never crosses declared breaks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SurfaceNormalMode {
    Flat,
    #[default]
    SmoothWithinPatches,
}
/// Scalar-field interpolation semantics across a scientific break line.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BreakInterpolationPolicy {
    #[default]
    Split,
    ContinuousValue,
    ExplicitJump,
}
/// Bounds for adaptive curve tessellation on an embedding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceCurveTessellation {
    pub max_chord_error: f64,
    pub max_normal_angle: f64,
    pub max_subdivision_depth: u32,
}
impl Default for SurfaceCurveTessellation {
    fn default() -> Self {
        Self {
            max_chord_error: 1.0e-3,
            max_normal_angle: 0.174_532_925_199_432_95,
            max_subdivision_depth: 10,
        }
    }
}
/// Revision counters used by caches to rebuild only affected embedded resources.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EmbeddingRevision {
    pub geometry: u64,
    pub topology: u64,
    pub breaks: u64,
    pub style: u64,
}
/// An affine map from a reference ternary triangle into the parent tetrahedron.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanarEmbedding {
    vertices: [TetraPoint; 3],
}
impl PlanarEmbedding {
    pub fn new(vertices: [TetraPoint; 3], tolerance: Tolerance) -> Result<Self, SectionError> {
        for vertex in vertices {
            vertex.validate(crate::Normalization::RequireUnitSum, tolerance)?;
        }
        if tetra_triangle_area(vertices) <= tolerance.absolute {
            return Err(SectionError::DegenerateSectionTriangle);
        }
        Ok(Self { vertices })
    }
    pub const fn vertices(self) -> [TetraPoint; 3] {
        self.vertices
    }
    pub fn map(self, local: TernaryPoint) -> Result<TetraPoint, SectionError> {
        let local = local.as_array();
        Ok(TetraPoint::from_unchecked([0, 1, 2, 3].map(|component| {
            local[0] * self.vertices[0].as_array()[component]
                + local[1] * self.vertices[1].as_array()[component]
                + local[2] * self.vertices[2].as_array()[component]
        })))
    }
    pub fn unmap(
        self,
        point: TetraPoint,
        tolerance: Tolerance,
    ) -> Result<TernaryPoint, SectionError> {
        let weights = tetra_triangle_coordinates(point, self.vertices, tolerance)?;
        TernaryPoint::new(weights).map_err(SectionError::from)
    }
    pub fn world_normal(self, geometry: &TetraGeometry) -> Result<[f64; 3], SectionError> {
        normal3(
            sub3(
                geometry.to_world(self.vertices[1]),
                geometry.to_world(self.vertices[0]),
            ),
            sub3(
                geometry.to_world(self.vertices[2]),
                geometry.to_world(self.vertices[0]),
            ),
        )
        .ok_or(SectionError::DegenerateSectionTriangle)
    }
}
/// A piecewise-linear, topologically triangular surface embedding.
#[derive(Clone, Debug)]
pub struct TriangulatedEmbedding {
    local_vertices: Vec<TernaryPoint>,
    tetra_vertices: Vec<TetraPoint>,
    triangles: Vec<[u32; 3]>,
    topology: SurfaceTopology,
    patches: Vec<SurfacePatch>,
    edge_kinds: BTreeMap<(u32, u32), SurfaceEdgeKind>,
    break_lines: Vec<BreakLine>,
    revision: EmbeddingRevision,
    next_patch_id: u64,
    next_break_id: u64,
}
impl TriangulatedEmbedding {
    pub fn new(
        local_vertices: Vec<TernaryPoint>,
        tetra_vertices: Vec<TetraPoint>,
        triangles: Vec<[u32; 3]>,
        topology: SurfaceTopology,
        tolerance: Tolerance,
    ) -> Result<Self, SectionError> {
        let mut value = Self {
            local_vertices,
            tetra_vertices,
            triangles,
            topology,
            patches: Vec::new(),
            edge_kinds: BTreeMap::new(),
            break_lines: Vec::new(),
            revision: EmbeddingRevision::default(),
            next_patch_id: 1,
            next_break_id: 1,
        };
        value.validate(tolerance)?;
        value.patches.push(SurfacePatch {
            id: SurfacePatchId(0),
            triangles: (0..value.triangles.len() as u32).collect(),
            name: None,
        });
        value.classify_boundaries()?;
        Ok(value)
    }
    pub fn local_vertices(&self) -> &[TernaryPoint] {
        &self.local_vertices
    }
    pub fn tetra_vertices(&self) -> &[TetraPoint] {
        &self.tetra_vertices
    }
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
    pub fn topology(&self) -> &SurfaceTopology {
        &self.topology
    }
    pub fn patches(&self) -> &[SurfacePatch] {
        &self.patches
    }
    pub fn break_lines(&self) -> &[BreakLine] {
        &self.break_lines
    }
    pub const fn revision(&self) -> EmbeddingRevision {
        self.revision
    }
    pub fn edge_kind(
        &self,
        first: SurfaceVertexIndex,
        second: SurfaceVertexIndex,
    ) -> Option<SurfaceEdgeKind> {
        self.edge_kinds.get(&edge_key(first.0, second.0)).copied()
    }
    pub fn map(
        &self,
        local: TernaryPoint,
        tolerance: Tolerance,
    ) -> Result<TetraPoint, SectionError> {
        let (_, weights) = self
            .locate_local(local, tolerance)
            .ok_or(SectionError::PointOutsideSectionPlane)?;
        let triangle = self.triangles[self
            .locate_local(local, tolerance)
            .expect("located triangle")
            .0];
        Ok(TetraPoint::from_unchecked([0, 1, 2, 3].map(|component| {
            weights[0] * self.tetra_vertices[triangle[0] as usize].as_array()[component]
                + weights[1] * self.tetra_vertices[triangle[1] as usize].as_array()[component]
                + weights[2] * self.tetra_vertices[triangle[2] as usize].as_array()[component]
        })))
    }
    pub fn unmap(
        &self,
        point: TetraPoint,
        tolerance: Tolerance,
    ) -> Result<TernaryPoint, SectionError> {
        for (index, triangle) in self.triangles.iter().copied().enumerate() {
            let vertices = triangle.map(|vertex| self.tetra_vertices[vertex as usize]);
            if let Ok(weights) = tetra_triangle_coordinates(point, vertices, tolerance) {
                return Ok(TernaryPoint::from_unchecked([0, 1, 2].map(|corner| {
                    weights[0] * self.local_vertices[triangle[0] as usize].as_array()[corner]
                        + weights[1] * self.local_vertices[triangle[1] as usize].as_array()[corner]
                        + weights[2] * self.local_vertices[triangle[2] as usize].as_array()[corner]
                })));
            }
            let _ = index;
        }
        Err(SectionError::PointOutsideSectionPlane)
    }
    pub fn world_normal(
        &self,
        local: TernaryPoint,
        geometry: &TetraGeometry,
        tolerance: Tolerance,
    ) -> Result<[f64; 3], SectionError> {
        let (index, _) = self
            .locate_local(local, tolerance)
            .ok_or(SectionError::PointOutsideSectionPlane)?;
        let triangle = self.triangles[index];
        normal3(
            sub3(
                geometry.to_world(self.tetra_vertices[triangle[1] as usize]),
                geometry.to_world(self.tetra_vertices[triangle[0] as usize]),
            ),
            sub3(
                geometry.to_world(self.tetra_vertices[triangle[2] as usize]),
                geometry.to_world(self.tetra_vertices[triangle[0] as usize]),
            ),
        )
        .ok_or(SectionError::DegenerateSectionTriangle)
    }
    pub fn add_break_line(
        &mut self,
        vertices: Vec<SurfaceVertexIndex>,
        adjacent_patches: [Option<SurfacePatchId>; 2],
        kind: BreakLineKind,
        style: BreakLineStyle,
    ) -> Result<BreakLineId, SectionError> {
        if vertices.len() < 2
            || vertices
                .iter()
                .any(|vertex| vertex.0 as usize >= self.local_vertices.len())
        {
            return Err(SectionError::DegenerateSectionTriangle);
        }
        for pair in vertices.windows(2) {
            let key = edge_key(pair[0].0, pair[1].0);
            if !self.edge_kinds.contains_key(&key)
                || matches!(
                    self.edge_kinds.get(&key),
                    Some(SurfaceEdgeKind::Boundary(_))
                )
            {
                return Err(SectionError::DegenerateSectionTriangle);
            }
            if matches!(
                self.edge_kinds.get(&key),
                Some(SurfaceEdgeKind::BreakLine(_))
            ) {
                return Err(SectionError::DegenerateSectionTriangle);
            }
        }
        let id = BreakLineId(self.next_break_id);
        self.next_break_id += 1;
        for pair in vertices.windows(2) {
            self.edge_kinds.insert(
                edge_key(pair[0].0, pair[1].0),
                SurfaceEdgeKind::BreakLine(id),
            );
        }
        self.break_lines.push(BreakLine {
            id,
            vertices,
            adjacent_patches,
            kind,
            style,
        });
        self.revision.breaks += 1;
        Ok(id)
    }
    pub fn remove_break_line(&mut self, id: BreakLineId) -> Option<BreakLine> {
        let index = self.break_lines.iter().position(|line| line.id == id)?;
        let line = self.break_lines.remove(index);
        for pair in line.vertices.windows(2) {
            self.edge_kinds
                .insert(edge_key(pair[0].0, pair[1].0), SurfaceEdgeKind::Smooth);
        }
        self.revision.breaks += 1;
        Some(line)
    }
    pub fn add_patch(
        &mut self,
        triangles: Vec<u32>,
        name: Option<String>,
    ) -> Result<SurfacePatchId, SectionError> {
        if triangles
            .iter()
            .any(|triangle| *triangle as usize >= self.triangles.len())
        {
            return Err(SectionError::DegenerateSectionTriangle);
        }
        let id = SurfacePatchId(self.next_patch_id);
        self.next_patch_id += 1;
        self.patches.push(SurfacePatch {
            id,
            triangles,
            name,
        });
        self.revision.topology += 1;
        Ok(id)
    }
    pub fn validate(&self, tolerance: Tolerance) -> Result<(), SectionError> {
        if self.local_vertices.len() != self.tetra_vertices.len() || self.local_vertices.is_empty()
        {
            return Err(SectionError::DegenerateSectionTriangle);
        }
        for point in &self.tetra_vertices {
            point.validate(crate::Normalization::RequireUnitSum, tolerance)?;
        }
        let mut orientation = None;
        let mut edges = BTreeMap::<(u32, u32), usize>::new();
        for (index, triangle) in self.triangles.iter().copied().enumerate() {
            if triangle[0] == triangle[1]
                || triangle[1] == triangle[2]
                || triangle[0] == triangle[2]
                || triangle
                    .iter()
                    .any(|vertex| *vertex as usize >= self.local_vertices.len())
            {
                return Err(SectionError::DegenerateSectionTriangle);
            }
            let local = triangle.map(|vertex| self.local_vertices[vertex as usize]);
            let area = ternary_orientation(local);
            if area.abs() <= tolerance.absolute
                || tetra_triangle_area(triangle.map(|vertex| self.tetra_vertices[vertex as usize]))
                    <= tolerance.absolute
            {
                return Err(SectionError::DegenerateSectionTriangle);
            }
            let sign = area.signum();
            if let Some(expected) = orientation {
                if sign != expected {
                    return Err(SectionError::DegenerateSectionTriangle);
                }
            } else {
                orientation = Some(sign);
            }
            for [a, b] in [
                [triangle[0], triangle[1]],
                [triangle[1], triangle[2]],
                [triangle[2], triangle[0]],
            ] {
                *edges.entry(edge_key(a, b)).or_default() += 1;
            }
            let _ = index;
        }
        if edges.values().any(|count| *count > 2) {
            return Err(SectionError::DegenerateSectionTriangle);
        }
        let mut all = self
            .topology
            .corners
            .into_iter()
            .chain(self.topology.boundary_chains.iter().flatten().copied());
        if all.any(|vertex| vertex.0 as usize >= self.local_vertices.len()) {
            return Err(SectionError::DegenerateSectionTriangle);
        }
        Ok(())
    }
    fn classify_boundaries(&mut self) -> Result<(), SectionError> {
        let mut counts = BTreeMap::<(u32, u32), usize>::new();
        for triangle in &self.triangles {
            for [a, b] in [
                [triangle[0], triangle[1]],
                [triangle[1], triangle[2]],
                [triangle[2], triangle[0]],
            ] {
                *counts.entry(edge_key(a, b)).or_default() += 1;
            }
        }
        for (edge, count) in counts {
            self.edge_kinds.insert(
                edge,
                if count == 1 {
                    SurfaceEdgeKind::Boundary(boundary_index(edge, &self.topology).unwrap_or(255))
                } else {
                    SurfaceEdgeKind::Smooth
                },
            );
        }
        Ok(())
    }
    fn locate_local(&self, local: TernaryPoint, tolerance: Tolerance) -> Option<(usize, [f64; 3])> {
        self.triangles
            .iter()
            .copied()
            .enumerate()
            .find_map(|(index, triangle)| {
                let weights = ternary_triangle_coordinates(
                    local,
                    triangle.map(|vertex| self.local_vertices[vertex as usize]),
                );
                weights
                    .filter(|weights| weights.iter().all(|weight| *weight >= -tolerance.absolute))
                    .map(|weights| (index, weights))
            })
    }
}
/// A concrete embedding avoids per-point trait-object dispatch while retaining one local chart model.
#[derive(Clone, Debug)]
pub enum ChartEmbedding {
    Planar(PlanarEmbedding),
    Triangulated(TriangulatedEmbedding),
}
impl ChartEmbedding {
    pub fn map(
        &self,
        local: TernaryPoint,
        tolerance: Tolerance,
    ) -> Result<TetraPoint, SectionError> {
        match self {
            Self::Planar(value) => value.map(local),
            Self::Triangulated(value) => value.map(local, tolerance),
        }
    }
    pub fn unmap(
        &self,
        point: TetraPoint,
        tolerance: Tolerance,
    ) -> Result<TernaryPoint, SectionError> {
        match self {
            Self::Planar(value) => value.unmap(point, tolerance),
            Self::Triangulated(value) => value.unmap(point, tolerance),
        }
    }
    pub fn world_normal(
        &self,
        local: TernaryPoint,
        geometry: &TetraGeometry,
        tolerance: Tolerance,
    ) -> Result<[f64; 3], SectionError> {
        match self {
            Self::Planar(value) => value.world_normal(geometry),
            Self::Triangulated(value) => value.world_normal(local, geometry, tolerance),
        }
    }
    pub fn revision(&self) -> EmbeddingRevision {
        match self {
            Self::Planar(_) => EmbeddingRevision::default(),
            Self::Triangulated(value) => value.revision(),
        }
    }
}
fn edge_key(first: u32, second: u32) -> (u32, u32) {
    (first.min(second), first.max(second))
}
fn boundary_index(edge: (u32, u32), topology: &SurfaceTopology) -> Option<u8> {
    topology
        .boundary_chains
        .iter()
        .enumerate()
        .find_map(|(index, chain)| {
            chain
                .windows(2)
                .any(|pair| edge_key(pair[0].0, pair[1].0) == edge)
                .then_some(index as u8)
        })
}
fn ternary_orientation(vertices: [TernaryPoint; 3]) -> f64 {
    let a = vertices[0].as_array();
    let b = vertices[1].as_array();
    let c = vertices[2].as_array();
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn ternary_triangle_coordinates(
    point: TernaryPoint,
    vertices: [TernaryPoint; 3],
) -> Option<[f64; 3]> {
    let a = vertices[0].as_array();
    let b = vertices[1].as_array();
    let c = vertices[2].as_array();
    let p = point.as_array();
    let determinant = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    (determinant.abs() > f64::EPSILON).then(|| {
        let u = ((p[0] - a[0]) * (c[1] - a[1]) - (p[1] - a[1]) * (c[0] - a[0])) / determinant;
        let v = ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])) / determinant;
        [1.0 - u - v, u, v]
    })
}
fn tetra_triangle_area(vertices: [TetraPoint; 3]) -> f64 {
    let a = vertices[0].as_array();
    let b = vertices[1].as_array();
    let c = vertices[2].as_array();
    let u = [0, 1, 2, 3].map(|index| b[index] - a[index]);
    let v = [0, 1, 2, 3].map(|index| c[index] - a[index]);
    let uu: f64 = u.into_iter().map(|value| value * value).sum();
    let vv: f64 = v.into_iter().map(|value| value * value).sum();
    let uv: f64 = u.into_iter().zip(v).map(|(left, right)| left * right).sum();
    (uu * vv - uv * uv).max(0.0).sqrt() * 0.5
}
fn tetra_triangle_coordinates(
    point: TetraPoint,
    vertices: [TetraPoint; 3],
    tolerance: Tolerance,
) -> Result<[f64; 3], SectionError> {
    let a = vertices[0].as_array();
    let b = vertices[1].as_array();
    let c = vertices[2].as_array();
    let p = point.as_array();
    let u = [0, 1, 2, 3].map(|index| b[index] - a[index]);
    let v = [0, 1, 2, 3].map(|index| c[index] - a[index]);
    let q = [0, 1, 2, 3].map(|index| p[index] - a[index]);
    let uu: f64 = u.into_iter().map(|value| value * value).sum();
    let vv: f64 = v.into_iter().map(|value| value * value).sum();
    let uv: f64 = u.into_iter().zip(v).map(|(left, right)| left * right).sum();
    let uq: f64 = u.into_iter().zip(q).map(|(left, right)| left * right).sum();
    let vq: f64 = v.into_iter().zip(q).map(|(left, right)| left * right).sum();
    let determinant = uu * vv - uv * uv;
    if determinant.abs() <= tolerance.absolute {
        return Err(SectionError::DegenerateSectionTriangle);
    }
    let beta = (uq * vv - vq * uv) / determinant;
    let gamma = (vq * uu - uq * uv) / determinant;
    let result = [1.0 - beta - gamma, beta, gamma];
    let recovered = [0, 1, 2, 3]
        .map(|index| result[0] * a[index] + result[1] * b[index] + result[2] * c[index]);
    if recovered
        .into_iter()
        .zip(p)
        .any(|(left, right)| !tolerance.is_close(left, right))
        || result.iter().any(|weight| *weight < -tolerance.absolute)
    {
        return Err(SectionError::PointOutsideSectionPlane);
    }
    Ok(result.map(|weight| {
        if tolerance.is_near_zero(weight) {
            0.0
        } else {
            weight
        }
    }))
}
fn sub3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}
fn normal3(left: [f64; 3], right: [f64; 3]) -> Option<[f64; 3]> {
    let normal = [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ];
    let length = normal
        .into_iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    (length > f64::EPSILON).then_some(normal.map(|value| value / length))
}
