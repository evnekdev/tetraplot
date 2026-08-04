use super::TernaryPoint;
use crate::{
    Color, Normalization, SectionError, SurfaceOverlayMode, TetraGeometry, TetraPoint, Tolerance,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

type EdgeKey = (u32, u32);

/// Stable index of a vertex in a triangulated embedding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfaceVertexIndex(pub u32);

/// Stable scientific patch identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfacePatchId(u64);
impl SurfacePatchId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
    /// Returns the stable numeric representation for diagnostics.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable declared break-line identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BreakLineId(u64);
impl BreakLineId {
    /// Returns the stable numeric representation for diagnostics.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One connected region where smooth interpolation is permitted.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfacePatch {
    id: SurfacePatchId,
    triangles: Vec<u32>,
    name: Option<String>,
}
impl SurfacePatch {
    /// Describes an unnamed patch before [`TriangulatedEmbedding::set_patches`].
    pub fn new(triangles: Vec<u32>) -> Self {
        Self {
            id: SurfacePatchId(0),
            triangles,
            name: None,
        }
    }
    /// Describes a named patch before [`TriangulatedEmbedding::set_patches`].
    pub fn named(triangles: Vec<u32>, name: impl Into<String>) -> Self {
        Self {
            id: SurfacePatchId(0),
            triangles,
            name: Some(name.into()),
        }
    }
    /// Returns the ID assigned by the embedding.
    pub const fn id(&self) -> SurfacePatchId {
        self.id
    }
    /// Returns owned surface triangles.
    pub fn triangles(&self) -> &[u32] {
        &self.triangles
    }
    /// Returns the optional scientific name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

/// Classification of a chart-surface edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceEdgeKind {
    /// An internal edge inside one patch.
    Smooth,
    /// An explicit scientific discontinuity.
    BreakLine(BreakLineId),
    /// An edge on a chart boundary chain.
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

/// Visual style for a break-line overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreakLineStyle {
    /// Whether the line is drawn.
    pub visible: bool,
    /// Renderer line width.
    pub width: f32,
    /// Renderer line colour.
    pub color: Color,
    /// Renderer-only anti-z-fighting policy.
    pub overlay_mode: SurfaceOverlayMode,
}
impl Default for BreakLineStyle {
    fn default() -> Self {
        Self {
            visible: true,
            width: 2.5,
            color: Color::rgb(0.12, 0.08, 0.08),
            overlay_mode: SurfaceOverlayMode::DepthBias,
        }
    }
}

/// A graph-compatible break-line arc represented by an ordered mesh-edge chain.
#[derive(Clone, Debug, PartialEq)]
pub struct BreakLine {
    id: BreakLineId,
    vertices: Vec<SurfaceVertexIndex>,
    adjacent_patches: [SurfacePatchId; 2],
    kind: BreakLineKind,
    style: BreakLineStyle,
}
impl BreakLine {
    /// Returns the stable break-line ID.
    pub const fn id(&self) -> BreakLineId {
        self.id
    }
    /// Returns the ordered scientific mesh vertices.
    pub fn vertices(&self) -> &[SurfaceVertexIndex] {
        &self.vertices
    }
    /// Returns the patch pair derived from mesh adjacency.
    pub const fn adjacent_patches(&self) -> [SurfacePatchId; 2] {
        self.adjacent_patches
    }
    /// Returns the scientific classification.
    pub const fn kind(&self) -> BreakLineKind {
        self.kind
    }
    /// Returns the visual overlay style.
    pub const fn style(&self) -> BreakLineStyle {
        self.style
    }

    /// Updates the scientific classification while preserving topology.
    pub fn set_kind(&mut self, kind: BreakLineKind) {
        self.kind = kind;
    }
    /// Updates visual overlay style without changing topology.
    pub fn set_style(&mut self, style: BreakLineStyle) {
        self.style = style;
    }
}
/// Explicit corners and boundary chains of the reference ternary chart.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceTopology {
    corners: [SurfaceVertexIndex; 3],
    boundary_chains: [Vec<SurfaceVertexIndex>; 3],
}
impl SurfaceTopology {
    /// Constructs explicit chart boundary topology.
    pub fn new(
        corners: [SurfaceVertexIndex; 3],
        boundary_chains: [Vec<SurfaceVertexIndex>; 3],
    ) -> Self {
        Self {
            corners,
            boundary_chains,
        }
    }
    /// Returns the three distinguished corners.
    pub const fn corners(&self) -> [SurfaceVertexIndex; 3] {
        self.corners
    }
    /// Returns the ordered boundary chains.
    pub fn boundary_chains(&self) -> &[Vec<SurfaceVertexIndex>; 3] {
        &self.boundary_chains
    }
}

/// Normal interpolation that never crosses scientific breaks or chart boundaries.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SurfaceNormalMode {
    /// Emits one normal per triangle corner.
    Flat,
    /// Averages area-weighted normals only within one patch.
    #[default]
    SmoothWithinPatches,
}

/// Scalar-field semantics across break lines, reserved for future contours.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BreakInterpolationPolicy {
    #[default]
    Split,
    ContinuousValue,
    ExplicitJump,
}

/// Bounds reserved for future adaptive smooth-surface tessellation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceCurveTessellation {
    /// Maximum world-space midpoint/chord disagreement.
    pub max_chord_error: f64,
    /// Maximum normal-angle disagreement in radians.
    pub max_normal_angle: f64,
    /// Maximum recursive subdivision depth.
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

/// Revisions consumed by prepared embedded-chart caches.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EmbeddingRevision {
    /// Parent tetrahedral vertices changed.
    pub geometry: u64,
    /// Parameter mesh or patch ownership changed.
    pub topology: u64,
    /// Break-line topology changed.
    pub breaks: u64,
    /// Break-line display style changed.
    pub style: u64,
}

/// A local point located on a concrete embedding triangle.
#[derive(Clone, Debug, PartialEq)]
pub struct LocatedSurfacePoint {
    /// Triangulated-surface triangle index, or zero for affine embeddings.
    pub triangle: u32,
    /// Authoritative scientific patch.
    pub patch: SurfacePatchId,
    /// Barycentric weights in `triangle`.
    pub triangle_weights: [f64; 3],
    /// Canonical chart-local coordinate.
    pub local: TernaryPoint,
    /// Parent tetrahedral coordinate.
    pub tetrahedral: TetraPoint,
}

/// An affine map from a local ternary simplex into the parent tetrahedron.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanarEmbedding {
    vertices: [TetraPoint; 3],
}
impl PlanarEmbedding {
    /// Constructs a non-degenerate affine embedding.
    pub fn new(vertices: [TetraPoint; 3], tolerance: Tolerance) -> Result<Self, SectionError> {
        for vertex in vertices {
            vertex.validate(Normalization::RequireUnitSum, tolerance)?;
        }
        if tetra_triangle_area(vertices) <= tolerance.absolute {
            return Err(SectionError::DegenerateSectionTriangle);
        }
        Ok(Self { vertices })
    }
    /// Returns the parent tetrahedral corners.
    pub const fn vertices(self) -> [TetraPoint; 3] {
        self.vertices
    }
    /// Maps a valid local coordinate into parent tetrahedral space.
    pub fn map(self, local: TernaryPoint) -> Result<TetraPoint, SectionError> {
        let local = TernaryPoint::new(local.as_array())?.as_array();
        Ok(TetraPoint::from_unchecked([0, 1, 2, 3].map(|component| {
            local[0] * self.vertices[0].as_array()[component]
                + local[1] * self.vertices[1].as_array()[component]
                + local[2] * self.vertices[2].as_array()[component]
        })))
    }
    /// Recovers local coordinates only for a point on the affine section.
    pub fn unmap(
        self,
        point: TetraPoint,
        tolerance: Tolerance,
    ) -> Result<TernaryPoint, SectionError> {
        TernaryPoint::with_policy(
            tetra_triangle_coordinates(point, self.vertices, tolerance)?,
            Normalization::RequireUnitSum,
            tolerance,
        )
        .map_err(SectionError::from)
    }
    /// Returns the consistently wound world-space normal.
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

#[derive(Clone, Debug)]
struct EdgeAdjacency {
    incidents: Vec<(u32, u8)>,
    boundary_chain: Option<u8>,
    break_line: Option<BreakLineId>,
}
#[derive(Clone, Debug)]
struct MeshAdjacency {
    edges: BTreeMap<EdgeKey, EdgeAdjacency>,
    neighbours: Vec<[Option<u32>; 3]>,
}
impl MeshAdjacency {
    fn build(triangles: &[[u32; 3]]) -> Result<Self, SectionError> {
        let mut edges = BTreeMap::<EdgeKey, EdgeAdjacency>::new();
        for (triangle_index, triangle) in triangles.iter().copied().enumerate() {
            for (slot, [first, second]) in [
                [triangle[0], triangle[1]],
                [triangle[1], triangle[2]],
                [triangle[2], triangle[0]],
            ]
            .into_iter()
            .enumerate()
            {
                edges
                    .entry(edge_key(first, second))
                    .or_insert_with(|| EdgeAdjacency {
                        incidents: Vec::new(),
                        boundary_chain: None,
                        break_line: None,
                    })
                    .incidents
                    .push((triangle_index as u32, slot as u8));
            }
        }
        if edges.values().any(|edge| edge.incidents.len() > 2) {
            return Err(SectionError::FoldedParameterization);
        }
        let mut neighbours = vec![[None; 3]; triangles.len()];
        for edge in edges.values() {
            if let [(a, a_slot), (b, b_slot)] = edge.incidents.as_slice() {
                neighbours[*a as usize][*a_slot as usize] = Some(*b);
                neighbours[*b as usize][*b_slot as usize] = Some(*a);
            }
        }
        Ok(Self { edges, neighbours })
    }
}
/// A piecewise-linear, topologically triangular surface embedding.
#[derive(Clone, Debug)]
pub struct TriangulatedEmbedding {
    local_vertices: Vec<TernaryPoint>,
    tetra_vertices: Vec<TetraPoint>,
    triangles: Vec<[u32; 3]>,
    topology: SurfaceTopology,
    adjacency: MeshAdjacency,
    patches: Vec<SurfacePatch>,
    patch_by_triangle: Vec<SurfacePatchId>,
    break_lines: Vec<BreakLine>,
    revision: EmbeddingRevision,
    next_patch_id: u64,
    next_break_id: u64,
}

impl TriangulatedEmbedding {
    /// Constructs and validates a complete reference-triangle parameter mesh.
    pub fn new(
        local_vertices: Vec<TernaryPoint>,
        tetra_vertices: Vec<TetraPoint>,
        triangles: Vec<[u32; 3]>,
        topology: SurfaceTopology,
        tolerance: Tolerance,
    ) -> Result<Self, SectionError> {
        let adjacency = MeshAdjacency::build(&triangles)?;
        let mut value = Self {
            local_vertices,
            tetra_vertices,
            triangles,
            topology,
            adjacency,
            patches: Vec::new(),
            patch_by_triangle: Vec::new(),
            break_lines: Vec::new(),
            revision: EmbeddingRevision::default(),
            next_patch_id: 1,
            next_break_id: 1,
        };
        value.validate_geometry(tolerance)?;
        value.assign_boundary_chains()?;
        value.patches.push(SurfacePatch {
            id: SurfacePatchId(0),
            triangles: (0..value.triangles.len() as u32).collect(),
            name: None,
        });
        value.patch_by_triangle = vec![SurfacePatchId(0); value.triangles.len()];
        Ok(value)
    }

    /// Returns local reference-triangle vertices.
    pub fn local_vertices(&self) -> &[TernaryPoint] {
        &self.local_vertices
    }
    /// Returns parent tetrahedral vertices.
    pub fn tetra_vertices(&self) -> &[TetraPoint] {
        &self.tetra_vertices
    }
    /// Returns indexed scientific surface triangles.
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
    /// Returns parameter-domain topology.
    pub fn topology(&self) -> &SurfaceTopology {
        &self.topology
    }
    /// Returns authoritative, non-overlapping scientific patches.
    pub fn patches(&self) -> &[SurfacePatch] {
        &self.patches
    }
    /// Returns declared scientific break lines.
    pub fn break_lines(&self) -> &[BreakLine] {
        &self.break_lines
    }
    /// Returns embedding cache revisions.
    pub const fn revision(&self) -> EmbeddingRevision {
        self.revision
    }

    /// Returns incident triangle indexes for a mesh edge.
    pub fn edge_incident_triangles(
        &self,
        first: SurfaceVertexIndex,
        second: SurfaceVertexIndex,
    ) -> Option<Vec<u32>> {
        self.adjacency
            .edges
            .get(&edge_key(first.0, second.0))
            .map(|edge| {
                edge.incidents
                    .iter()
                    .map(|(triangle, _)| *triangle)
                    .collect()
            })
    }
    /// Returns the outer boundary chain containing an edge.
    pub fn boundary_chain(
        &self,
        first: SurfaceVertexIndex,
        second: SurfaceVertexIndex,
    ) -> Option<u8> {
        self.adjacency
            .edges
            .get(&edge_key(first.0, second.0))
            .and_then(|edge| edge.boundary_chain)
    }
    /// Returns authoritative edge classification.
    pub fn edge_kind(
        &self,
        first: SurfaceVertexIndex,
        second: SurfaceVertexIndex,
    ) -> Option<SurfaceEdgeKind> {
        self.adjacency
            .edges
            .get(&edge_key(first.0, second.0))
            .map(|edge| match (edge.break_line, edge.boundary_chain) {
                (Some(id), _) => SurfaceEdgeKind::BreakLine(id),
                (None, Some(chain)) => SurfaceEdgeKind::Boundary(chain),
                (None, None) => SurfaceEdgeKind::Smooth,
            })
    }

    /// Replaces the default patch with a complete, connected, non-overlapping partition.
    ///
    /// All internal patch boundaries must be declared as breaks before the embedding is rendered.
    pub fn set_patches(&mut self, mut patches: Vec<SurfacePatch>) -> Result<(), SectionError> {
        if patches.is_empty() {
            return Err(SectionError::LocalDomainNotCovered);
        }
        for patch in &mut patches {
            patch.id = SurfacePatchId(self.next_patch_id);
            self.next_patch_id += 1;
        }
        let membership = self.validate_patch_partition(&patches)?;
        self.validate_existing_breaks_against(&membership)?;
        self.patches = patches;
        self.patch_by_triangle = membership;
        self.revision.topology += 1;
        Ok(())
    }

    /// Locates a local chart position and maps it to parent tetrahedral coordinates.
    pub fn locate(
        &self,
        local: TernaryPoint,
        tolerance: Tolerance,
    ) -> Result<LocatedSurfacePoint, SectionError> {
        let local =
            TernaryPoint::with_policy(local.as_array(), Normalization::RequireUnitSum, tolerance)?;
        let (triangle, triangle_weights) = self
            .locate_local(local, tolerance)
            .ok_or(SectionError::PointOutsideSectionPlane)?;
        Ok(LocatedSurfacePoint {
            triangle: triangle as u32,
            patch: self.patch_by_triangle[triangle],
            triangle_weights,
            local,
            tetrahedral: self.map_located(triangle, triangle_weights),
        })
    }
    /// Maps a local chart position to parent tetrahedral coordinates.
    pub fn map(
        &self,
        local: TernaryPoint,
        tolerance: Tolerance,
    ) -> Result<TetraPoint, SectionError> {
        Ok(self.locate(local, tolerance)?.tetrahedral)
    }
    /// Recovers local coordinates only for a point on this surface.
    pub fn unmap(
        &self,
        point: TetraPoint,
        tolerance: Tolerance,
    ) -> Result<TernaryPoint, SectionError> {
        Ok(self.locate_tetra(point, tolerance)?.local)
    }
    /// Locates a tetrahedral position only when it lies on this surface.
    pub fn locate_tetra(
        &self,
        point: TetraPoint,
        tolerance: Tolerance,
    ) -> Result<LocatedSurfacePoint, SectionError> {
        for (triangle_index, triangle) in self.triangles.iter().copied().enumerate() {
            let vertices = triangle.map(|vertex| self.tetra_vertices[vertex as usize]);
            if let Ok(weights) = tetra_triangle_coordinates(point, vertices, tolerance) {
                let local = TernaryPoint::from_unchecked([0, 1, 2].map(|corner| {
                    weights[0] * self.local_vertices[triangle[0] as usize].as_array()[corner]
                        + weights[1] * self.local_vertices[triangle[1] as usize].as_array()[corner]
                        + weights[2] * self.local_vertices[triangle[2] as usize].as_array()[corner]
                }));
                return Ok(LocatedSurfacePoint {
                    triangle: triangle_index as u32,
                    patch: self.patch_by_triangle[triangle_index],
                    triangle_weights: weights,
                    local,
                    tetrahedral: point,
                });
            }
        }
        Err(SectionError::PointOutsideSectionPlane)
    }
    /// Returns a consistently wound face normal at a local point.
    pub fn world_normal(
        &self,
        local: TernaryPoint,
        geometry: &TetraGeometry,
        tolerance: Tolerance,
    ) -> Result<[f64; 3], SectionError> {
        let located = self.locate(local, tolerance)?;
        let triangle = self.triangles[located.triangle as usize];
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
        .ok_or(SectionError::DegenerateWorldTriangle {
            triangle: located.triangle as usize,
        })
    }

    pub(crate) fn locate_local(
        &self,
        local: TernaryPoint,
        tolerance: Tolerance,
    ) -> Option<(usize, [f64; 3])> {
        self.triangles
            .iter()
            .copied()
            .enumerate()
            .find_map(|(index, triangle)| {
                ternary_triangle_coordinates(
                    local,
                    triangle.map(|vertex| self.local_vertices[vertex as usize]),
                )
                .filter(|weights| weights.iter().all(|weight| *weight >= -tolerance.absolute))
                .map(|weights| (index, weights))
            })
    }
    pub(crate) fn patch_for_triangle(&self, triangle: usize) -> SurfacePatchId {
        self.patch_by_triangle[triangle]
    }
    pub(crate) fn map_located(&self, triangle_index: usize, weights: [f64; 3]) -> TetraPoint {
        let triangle = self.triangles[triangle_index];
        TetraPoint::from_unchecked([0, 1, 2, 3].map(|component| {
            weights[0] * self.tetra_vertices[triangle[0] as usize].as_array()[component]
                + weights[1] * self.tetra_vertices[triangle[1] as usize].as_array()[component]
                + weights[2] * self.tetra_vertices[triangle[2] as usize].as_array()[component]
        }))
    }
}
impl TriangulatedEmbedding {
    /// Declares an existing internal mesh-edge chain as an authoritative scientific break line.
    ///
    /// `[None, None]` derives the patch pair. Any supplied patch IDs are checked against mesh
    /// adjacency rather than trusted.
    pub fn add_break_line(
        &mut self,
        vertices: Vec<SurfaceVertexIndex>,
        declared_patches: [Option<SurfacePatchId>; 2],
        kind: BreakLineKind,
        style: BreakLineStyle,
    ) -> Result<BreakLineId, SectionError> {
        if vertices.len() < 2 {
            return Err(SectionError::InvalidBreakLineVertex { vertex: u32::MAX });
        }
        for vertex in &vertices {
            if vertex.0 as usize >= self.local_vertices.len() {
                return Err(SectionError::InvalidBreakLineVertex { vertex: vertex.0 });
            }
        }
        let id = BreakLineId(self.next_break_id);
        let mut expected_patches = None;
        let mut used_edges = BTreeSet::new();
        for pair in vertices.windows(2) {
            let key = edge_key(pair[0].0, pair[1].0);
            if !used_edges.insert(key) {
                return Err(SectionError::EdgeAlreadyAssignedToBreakLine {
                    from: pair[0].0,
                    to: pair[1].0,
                });
            }
            let edge = self.adjacency.edges.get(&key).ok_or(
                SectionError::BreakLineSegmentIsNotMeshEdge {
                    from: pair[0].0,
                    to: pair[1].0,
                },
            )?;
            if edge.boundary_chain.is_some() {
                return Err(SectionError::BreakLineUsesBoundaryEdge {
                    from: pair[0].0,
                    to: pair[1].0,
                });
            }
            if edge.break_line.is_some() {
                return Err(SectionError::EdgeAlreadyAssignedToBreakLine {
                    from: pair[0].0,
                    to: pair[1].0,
                });
            }
            let patches = self.edge_patches(key, &self.patch_by_triangle).ok_or(
                SectionError::BreakLinePatchMismatch {
                    from: pair[0].0,
                    to: pair[1].0,
                },
            )?;
            if expected_patches.is_some_and(|expected| expected != patches) {
                return Err(SectionError::BreakLinePatchMismatch {
                    from: pair[0].0,
                    to: pair[1].0,
                });
            }
            expected_patches = Some(patches);
        }
        let adjacent_patches =
            expected_patches.ok_or(SectionError::InvalidBreakLineVertex { vertex: u32::MAX })?;
        if declared_patches != [None, None] && !same_patch_pair(declared_patches, adjacent_patches)
        {
            return Err(SectionError::BreakLinePatchMismatch {
                from: vertices[0].0,
                to: vertices[1].0,
            });
        }
        for pair in vertices.windows(2) {
            if let Some(edge) = self
                .adjacency
                .edges
                .get_mut(&edge_key(pair[0].0, pair[1].0))
            {
                edge.break_line = Some(id);
            }
        }
        self.next_break_id += 1;
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

    /// Removes a break line and restores its edges to smooth classification.
    /// Updates break-line classification and visual style without changing its path.
    pub fn update_break_line(
        &mut self,
        id: BreakLineId,
        kind: BreakLineKind,
        style: BreakLineStyle,
    ) -> bool {
        let Some(line) = self.break_lines.iter_mut().find(|line| line.id == id) else {
            return false;
        };
        line.set_kind(kind);
        line.set_style(style);
        self.revision.style += 1;
        true
    }

    pub fn remove_break_line(&mut self, id: BreakLineId) -> Option<BreakLine> {
        let index = self.break_lines.iter().position(|line| line.id == id)?;
        let line = self.break_lines.remove(index);
        for pair in line.vertices.windows(2) {
            if let Some(edge) = self
                .adjacency
                .edges
                .get_mut(&edge_key(pair[0].0, pair[1].0))
            {
                edge.break_line = None;
            }
        }
        self.revision.breaks += 1;
        Some(line)
    }

    /// Changes only break-line display style; geometry and topology remain cached.
    pub fn set_break_line_style(
        &mut self,
        id: BreakLineId,
        style: BreakLineStyle,
    ) -> Result<(), SectionError> {
        let line = self
            .break_lines
            .iter_mut()
            .find(|line| line.id == id)
            .ok_or(SectionError::UnknownSurfacePatch { patch: id.get() })?;
        line.style = style;
        self.revision.style += 1;
        Ok(())
    }

    /// Validates geometry, patches, and complete scientific break topology.
    pub fn validate(&self, tolerance: Tolerance) -> Result<(), SectionError> {
        self.validate_geometry(tolerance)?;
        self.validate_patch_partition(&self.patches)?;
        self.validate_existing_breaks_against(&self.patch_by_triangle)?;
        self.validate_patch_boundaries()
    }

    fn validate_geometry(&self, tolerance: Tolerance) -> Result<(), SectionError> {
        tolerance.validate()?;
        if self.local_vertices.len() != self.tetra_vertices.len() {
            return Err(SectionError::MismatchedEmbeddingVertexCounts {
                local: self.local_vertices.len(),
                tetrahedral: self.tetra_vertices.len(),
            });
        }
        if self.local_vertices.is_empty() || self.triangles.is_empty() {
            return Err(SectionError::EmptyEmbedding);
        }
        for point in &self.local_vertices {
            TernaryPoint::with_policy(point.as_array(), Normalization::RequireUnitSum, tolerance)?;
        }
        for point in &self.tetra_vertices {
            point.validate(Normalization::RequireUnitSum, tolerance)?;
        }
        let mut orientation = None;
        let mut local_area = 0.0;
        for (index, triangle) in self.triangles.iter().copied().enumerate() {
            if triangle[0] == triangle[1]
                || triangle[1] == triangle[2]
                || triangle[0] == triangle[2]
            {
                return Err(SectionError::RepeatedSurfaceTriangleVertex { triangle: index });
            }
            for vertex in triangle {
                if vertex as usize >= self.local_vertices.len() {
                    return Err(SectionError::InvalidSurfaceTriangleIndex {
                        triangle: index,
                        vertex,
                        vertex_count: self.local_vertices.len(),
                    });
                }
            }
            let signed_area =
                ternary_orientation(triangle.map(|vertex| self.local_vertices[vertex as usize]));
            if signed_area.abs() <= tolerance.absolute {
                return Err(SectionError::DegenerateLocalTriangle { triangle: index });
            }
            if let Some(expected) = orientation {
                if signed_area.signum() != expected {
                    return Err(SectionError::InconsistentLocalOrientation { triangle: index });
                }
            } else {
                orientation = Some(signed_area.signum());
            }
            if tetra_triangle_area(triangle.map(|vertex| self.tetra_vertices[vertex as usize]))
                <= tolerance.absolute
            {
                return Err(SectionError::DegenerateWorldTriangle { triangle: index });
            }
            local_area += signed_area.abs();
        }
        if !tolerance.is_close(local_area, 1.0) {
            return Err(SectionError::FoldedParameterization);
        }
        self.validate_topology_chains()?;
        self.validate_triangle_connectivity()
    }

    fn validate_topology_chains(&self) -> Result<(), SectionError> {
        let count = self.local_vertices.len();
        let corners = self.topology.corners;
        if corners.iter().any(|corner| corner.0 as usize >= count)
            || corners[0] == corners[1]
            || corners[1] == corners[2]
            || corners[0] == corners[2]
        {
            return Err(SectionError::InvalidTopologyCorner { corner: 0 });
        }
        let mut declared = BTreeSet::new();
        for (chain_index, chain) in self.topology.boundary_chains.iter().enumerate() {
            if chain.len() < 2
                || chain.first() != Some(&corners[chain_index])
                || chain.last() != Some(&corners[(chain_index + 1) % 3])
            {
                return Err(SectionError::InvalidBoundaryChain { chain: chain_index });
            }
            for pair in chain.windows(2) {
                if pair[0].0 as usize >= count || pair[1].0 as usize >= count {
                    return Err(SectionError::InvalidBoundaryChain { chain: chain_index });
                }
                let key = edge_key(pair[0].0, pair[1].0);
                let edge = self
                    .adjacency
                    .edges
                    .get(&key)
                    .ok_or(SectionError::InvalidBoundaryChain { chain: chain_index })?;
                if edge.incidents.len() != 1 {
                    return Err(SectionError::InvalidBoundaryChain { chain: chain_index });
                }
                if !declared.insert(key) {
                    return Err(SectionError::DisconnectedBoundaryChain { chain: chain_index });
                }
            }
        }
        let actual: BTreeSet<_> = self
            .adjacency
            .edges
            .iter()
            .filter_map(|(key, edge)| (edge.incidents.len() == 1).then_some(*key))
            .collect();
        if declared != actual {
            return Err(SectionError::LocalDomainNotCovered);
        }
        let expected = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        if corners.into_iter().zip(expected).any(|(corner, expected)| {
            self.local_vertices[corner.0 as usize]
                .as_array()
                .into_iter()
                .zip(expected)
                .any(|(actual, expected)| (actual - expected).abs() > 1.0e-9)
        }) {
            return Err(SectionError::LocalDomainNotCovered);
        }
        Ok(())
    }

    fn validate_triangle_connectivity(&self) -> Result<(), SectionError> {
        let mut visited = vec![false; self.triangles.len()];
        let mut queue = VecDeque::from([0usize]);
        visited[0] = true;
        while let Some(triangle) = queue.pop_front() {
            for neighbour in self.adjacency.neighbours[triangle].iter().flatten() {
                let neighbour = *neighbour as usize;
                if !visited[neighbour] {
                    visited[neighbour] = true;
                    queue.push_back(neighbour);
                }
            }
        }
        if visited.iter().all(|visited| *visited) {
            Ok(())
        } else {
            Err(SectionError::LocalDomainNotCovered)
        }
    }

    fn assign_boundary_chains(&mut self) -> Result<(), SectionError> {
        self.validate_topology_chains()?;
        for (chain_index, chain) in self.topology.boundary_chains.iter().enumerate() {
            for pair in chain.windows(2) {
                self.adjacency
                    .edges
                    .get_mut(&edge_key(pair[0].0, pair[1].0))
                    .ok_or(SectionError::InvalidBoundaryChain { chain: chain_index })?
                    .boundary_chain = Some(chain_index as u8);
            }
        }
        Ok(())
    }
    fn validate_patch_partition(
        &self,
        patches: &[SurfacePatch],
    ) -> Result<Vec<SurfacePatchId>, SectionError> {
        if patches.is_empty() {
            return Err(SectionError::LocalDomainNotCovered);
        }
        let mut membership = vec![None; self.triangles.len()];
        for patch in patches {
            if patch.triangles.is_empty() {
                return Err(SectionError::DisconnectedPatch {
                    patch: patch.id.get(),
                });
            }
            let mut unique = BTreeSet::new();
            for triangle in &patch.triangles {
                if *triangle as usize >= self.triangles.len() {
                    return Err(SectionError::InvalidSurfaceTriangleIndex {
                        triangle: *triangle as usize,
                        vertex: 0,
                        vertex_count: self.local_vertices.len(),
                    });
                }
                if !unique.insert(*triangle)
                    || membership[*triangle as usize].replace(patch.id).is_some()
                {
                    return Err(SectionError::OverlappingPatchMembership {
                        triangle: *triangle,
                    });
                }
            }
            let start = patch.triangles[0] as usize;
            let mut visited = BTreeSet::from([start]);
            let mut queue = VecDeque::from([start]);
            while let Some(triangle) = queue.pop_front() {
                for neighbour in self.adjacency.neighbours[triangle].iter().flatten() {
                    let neighbour = *neighbour as usize;
                    if membership[neighbour] == Some(patch.id) && visited.insert(neighbour) {
                        queue.push_back(neighbour);
                    }
                }
            }
            if visited.len() != unique.len() {
                return Err(SectionError::DisconnectedPatch {
                    patch: patch.id.get(),
                });
            }
        }
        membership
            .into_iter()
            .enumerate()
            .map(|(triangle, patch)| {
                patch.ok_or(SectionError::UnknownSurfacePatch {
                    patch: triangle as u64,
                })
            })
            .collect()
    }

    fn edge_patches(
        &self,
        key: EdgeKey,
        membership: &[SurfacePatchId],
    ) -> Option<[SurfacePatchId; 2]> {
        let edge = self.adjacency.edges.get(&key)?;
        let [(first, _), (second, _)] = edge.incidents.as_slice() else {
            return None;
        };
        let first = membership[*first as usize];
        let second = membership[*second as usize];
        (first != second).then_some(if first < second {
            [first, second]
        } else {
            [second, first]
        })
    }

    fn validate_existing_breaks_against(
        &self,
        membership: &[SurfacePatchId],
    ) -> Result<(), SectionError> {
        for line in &self.break_lines {
            for pair in line.vertices.windows(2) {
                let key = edge_key(pair[0].0, pair[1].0);
                let edge = self.adjacency.edges.get(&key).ok_or(
                    SectionError::BreakLineSegmentIsNotMeshEdge {
                        from: pair[0].0,
                        to: pair[1].0,
                    },
                )?;
                if edge.break_line != Some(line.id) {
                    return Err(SectionError::EdgeAlreadyAssignedToBreakLine {
                        from: pair[0].0,
                        to: pair[1].0,
                    });
                }
                let actual = self.edge_patches(key, membership).ok_or(
                    SectionError::BreakLinePatchMismatch {
                        from: pair[0].0,
                        to: pair[1].0,
                    },
                )?;
                if actual != line.adjacent_patches {
                    return Err(SectionError::BreakLinePatchMismatch {
                        from: pair[0].0,
                        to: pair[1].0,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_patch_boundaries(&self) -> Result<(), SectionError> {
        for (key, edge) in &self.adjacency.edges {
            if self.edge_patches(*key, &self.patch_by_triangle).is_some()
                && edge.break_line.is_none()
            {
                return Err(SectionError::BreakLinePatchMismatch {
                    from: key.0,
                    to: key.1,
                });
            }
        }
        Ok(())
    }
}

/// A concrete embedding avoids per-point trait-object dispatch while retaining one local chart model.
///
/// The concrete enum intentionally stores both variants inline: the hot preparation path avoids
/// an allocation per embedding, and the public model remains straightforward to edit.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum ChartEmbedding {
    /// An affine planar section or tetrahedron face.
    Planar(PlanarEmbedding),
    /// A validated piecewise-linear triangular surface.
    Triangulated(TriangulatedEmbedding),
}
impl ChartEmbedding {
    /// Borrows a triangulated embedding when this chart is curved.
    pub const fn as_triangulated(&self) -> Option<&TriangulatedEmbedding> {
        match self {
            Self::Triangulated(value) => Some(value),
            Self::Planar(_) => None,
        }
    }
    /// Locates and maps a local chart coordinate.
    pub fn locate(
        &self,
        local: TernaryPoint,
        tolerance: Tolerance,
    ) -> Result<LocatedSurfacePoint, SectionError> {
        match self {
            Self::Planar(value) => {
                let local = TernaryPoint::with_policy(
                    local.as_array(),
                    Normalization::RequireUnitSum,
                    tolerance,
                )?;
                Ok(LocatedSurfacePoint {
                    triangle: 0,
                    patch: SurfacePatchId(0),
                    triangle_weights: local.as_array(),
                    local,
                    tetrahedral: value.map(local)?,
                })
            }
            Self::Triangulated(value) => value.locate(local, tolerance),
        }
    }
    /// Maps a local coordinate to the parent tetrahedral simplex.
    pub fn map(
        &self,
        local: TernaryPoint,
        tolerance: Tolerance,
    ) -> Result<TetraPoint, SectionError> {
        Ok(self.locate(local, tolerance)?.tetrahedral)
    }
    /// Recovers a local coordinate only for a point on the embedding surface.
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
    /// Returns the local surface normal.
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
    /// Returns cache revisions for the underlying embedding.
    pub fn revision(&self) -> EmbeddingRevision {
        match self {
            Self::Planar(_) => EmbeddingRevision::default(),
            Self::Triangulated(value) => value.revision(),
        }
    }
    pub(crate) fn validate_for_preparation(
        &self,
        tolerance: Tolerance,
    ) -> Result<(), SectionError> {
        if let Self::Triangulated(value) = self {
            value.validate(tolerance)?;
        }
        Ok(())
    }
}

fn edge_key(first: u32, second: u32) -> EdgeKey {
    (first.min(second), first.max(second))
}
fn same_patch_pair(declared: [Option<SurfacePatchId>; 2], actual: [SurfacePatchId; 2]) -> bool {
    matches!(declared, [Some(first), Some(second)] if (first == actual[0] && second == actual[1]) || (first == actual[1] && second == actual[0]))
}
fn ternary_orientation(vertices: [TernaryPoint; 3]) -> f64 {
    let a = vertices[0].as_array();
    let b = vertices[1].as_array();
    let c = vertices[2].as_array();
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
pub(crate) fn ternary_triangle_coordinates(
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
