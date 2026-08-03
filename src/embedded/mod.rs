//! Reusable local ternary diagrams embedded on affine sections or triangulated surfaces.
mod embedding;
mod local;
mod section;
pub use embedding::{
    BreakInterpolationPolicy, BreakLine, BreakLineId, BreakLineKind, BreakLineStyle,
    ChartEmbedding, EmbeddingRevision, PlanarEmbedding, SurfaceCurveTessellation, SurfaceEdgeKind,
    SurfaceNormalMode, SurfacePatch, SurfacePatchId, SurfaceTopology, SurfaceVertexIndex,
    TriangulatedEmbedding,
};
pub use local::{IntoTernaryPoint, TernaryPoint};
pub use section::{
    DiagramSeries, EmbeddedChartStyle, IntersectionProvenance, PlanarSection, SectionId,
    SectionIntersection, SectionPlane, SectionQuadrilateral, SectionSeriesId, SectionTransform,
    SectionTriangle, SectionVertex, TernaryDiagram,
};
/// Stable identity for a non-planar or independently managed embedded chart.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EmbeddedChartId(u64);
impl EmbeddedChartId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}
/// A reusable ternary diagram paired with either affine or curved embedding geometry.
#[derive(Clone, Debug)]
pub struct EmbeddedTernaryChart {
    id: Option<EmbeddedChartId>,
    embedding: ChartEmbedding,
    diagram: TernaryDiagram,
    style: EmbeddedChartStyle,
    visible: bool,
    embedding_revision: u64,
    chart_revision: u64,
    style_revision: u64,
}
impl EmbeddedTernaryChart {
    pub fn new(embedding: ChartEmbedding) -> Self {
        Self {
            id: None,
            embedding,
            diagram: TernaryDiagram::default(),
            style: EmbeddedChartStyle::default(),
            visible: true,
            embedding_revision: 0,
            chart_revision: 0,
            style_revision: 0,
        }
    }
    pub fn id(&self) -> Option<EmbeddedChartId> {
        self.id
    }
    pub fn embedding(&self) -> &ChartEmbedding {
        &self.embedding
    }
    pub fn replace_embedding(&mut self, embedding: ChartEmbedding) {
        self.embedding = embedding;
        self.embedding_revision += 1;
    }
    pub fn diagram(&self) -> &TernaryDiagram {
        &self.diagram
    }
    pub fn diagram_mut(&mut self) -> &mut TernaryDiagram {
        self.chart_revision += 1;
        &mut self.diagram
    }
    pub fn style(&self) -> EmbeddedChartStyle {
        self.style
    }
    pub fn set_style(&mut self, style: EmbeddedChartStyle) {
        self.style = style;
        self.style_revision += 1;
    }
    pub fn visible(&self) -> bool {
        self.visible
    }
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        self.style_revision += 1;
    }
    pub(crate) fn assign_id(&mut self, id: EmbeddedChartId) {
        self.id = Some(id);
    }
}
/// Coordinate-rich result shape reserved for renderer-backed embedded-chart picking.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddedChartPickResult {
    pub chart_id: EmbeddedChartId,
    pub series_id: Option<SectionSeriesId>,
    pub primitive_index: Option<usize>,
    pub local_position: TernaryPoint,
    pub tetrahedral_position: crate::TetraPoint,
    pub world_position: [f32; 3],
    pub surface_triangle: Option<u32>,
    pub scalar_value: Option<f64>,
}
