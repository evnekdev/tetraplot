//! One local ternary diagram model embedded either affinely or on a triangulated surface.
mod embedding;
mod local;
mod prepared;
mod section;

pub use embedding::{
    BreakInterpolationPolicy, BreakLine, BreakLineId, BreakLineKind, BreakLineStyle,
    ChartEmbedding, EmbeddingRevision, LocatedSurfacePoint, PlanarEmbedding,
    SurfaceCurveTessellation, SurfaceEdgeKind, SurfaceNormalMode, SurfacePatch, SurfacePatchId,
    SurfaceTopology, SurfaceVertexIndex, TriangulatedEmbedding,
};
pub use local::{IntoTernaryPoint, TernaryPoint};
pub use prepared::{
    PreparedBreakLine, PreparedEmbeddedChart, PreparedEmbeddedGrid, PreparedEmbeddedLine,
    PreparedEmbeddedLineSegment, PreparedEmbeddedPoint, PreparedEmbeddedPointSeries,
    PreparedEmbeddedSurface, PreparedEmbeddedTriangle, PreparedEmbeddedVertex,
};
pub use section::{
    DiagramSeries, EmbeddedChartStyle, EmbeddedGridStyle, IntersectionProvenance, PlanarSection,
    SectionId, SectionIntersection, SectionPlane, SectionQuadrilateral, SectionSeriesId,
    SectionTransform, SectionTriangle, SectionVertex, TernaryDiagram,
};

use std::{cell::RefCell, sync::Arc};

/// Stable identity for a non-planar or independently managed embedded chart.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EmbeddedChartId(u64);
impl EmbeddedChartId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
    /// Returns the stable numeric representation.
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug)]
struct EmbeddedPreparedCache {
    owner_embedding: u64,
    embedding: EmbeddingRevision,
    chart: u64,
    diagram: u64,
    style: u64,
    prepared: Arc<PreparedEmbeddedChart>,
}

/// One local ternary diagram plus one affine or curved embedding and rendering state.
#[derive(Clone, Debug)]
pub struct EmbeddedTernaryChart {
    id: Option<EmbeddedChartId>,
    name: Option<String>,
    embedding: ChartEmbedding,
    diagram: TernaryDiagram,
    style: EmbeddedChartStyle,
    visible: bool,
    embedding_revision: u64,
    chart_revision: u64,
    style_revision: u64,
    prepared_cache: RefCell<Option<EmbeddedPreparedCache>>,
}
impl EmbeddedTernaryChart {
    /// Creates a visible chart with an empty local diagram.
    pub fn new(embedding: ChartEmbedding) -> Self {
        Self {
            id: None,
            name: None,
            embedding,
            diagram: TernaryDiagram::default(),
            style: EmbeddedChartStyle::default(),
            visible: true,
            embedding_revision: 0,
            chart_revision: 0,
            style_revision: 0,
            prepared_cache: RefCell::new(None),
        }
    }
    /// Assigns a human-readable chart label before insertion into a scene.
    pub fn name(mut self, value: impl Into<String>) -> Self {
        self.name = Some(value.into());
        self
    }
    /// Returns the optional chart label.
    pub fn label(&self) -> Option<&str> {
        self.name.as_deref()
    }
    /// Replaces the chart label without changing its scientific geometry.
    pub fn set_name(&mut self, value: Option<impl Into<String>>) {
        self.name = value.map(Into::into);
        self.style_revision += 1;
    }
    /// Returns the optional scene identity.
    pub fn id(&self) -> Option<EmbeddedChartId> {
        self.id
    }
    /// Returns the embedding.
    pub fn embedding(&self) -> &ChartEmbedding {
        &self.embedding
    }
    /// Replaces the full embedding and invalidates prepared geometry.
    pub fn replace_embedding(&mut self, embedding: ChartEmbedding) {
        self.embedding = embedding;
        self.embedding_revision += 1;
        self.prepared_cache.get_mut().take();
    }
    /// Returns the reusable local diagram.
    pub fn diagram(&self) -> &TernaryDiagram {
        &self.diagram
    }
    /// Returns the local diagram and marks prepared chart content stale.
    pub fn diagram_mut(&mut self) -> &mut TernaryDiagram {
        self.chart_revision += 1;
        self.prepared_cache.get_mut().take();
        &mut self.diagram
    }
    /// Returns chart rendering style.
    pub fn style(&self) -> EmbeddedChartStyle {
        self.style
    }
    /// Replaces chart rendering style without changing scientific topology.
    pub fn set_style(&mut self, style: EmbeddedChartStyle) {
        self.style = style;
        self.style_revision += 1;
        self.prepared_cache.get_mut().take();
    }
    /// Returns visibility state.
    pub fn visible(&self) -> bool {
        self.visible
    }
    /// Updates visibility without changing chart geometry.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
    /// Returns a shared cached prepared result, rebuilding only when revisions changed.
    pub fn prepared(
        &self,
        geometry: &crate::TetraGeometry,
        tolerance: crate::Tolerance,
    ) -> Result<Arc<PreparedEmbeddedChart>, crate::SectionError> {
        let owner_embedding = self.embedding_revision;
        let embedding = self.embedding.revision();
        let chart = self.chart_revision;
        let diagram = self.diagram.revision();
        let style = self.style_revision;
        if let Some(cache) = self.prepared_cache.borrow().as_ref()
            && cache.owner_embedding == owner_embedding
            && cache.embedding == embedding
            && cache.chart == chart
            && cache.diagram == diagram
            && cache.style == style
        {
            return Ok(Arc::clone(&cache.prepared));
        }
        let prepared = Arc::new(prepared::prepare_embedded_chart(
            &self.embedding,
            &self.diagram,
            self.style,
            geometry,
            tolerance,
        )?);
        *self.prepared_cache.borrow_mut() = Some(EmbeddedPreparedCache {
            owner_embedding,
            embedding,
            chart,
            diagram,
            style,
            prepared: Arc::clone(&prepared),
        });
        Ok(prepared)
    }
    pub(crate) fn assign_id(&mut self, id: EmbeddedChartId) {
        self.id = Some(id);
    }
}

/// Coordinate-rich result shape reserved for renderer-backed embedded-chart picking.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddedChartPickResult {
    /// Scene chart identity.
    pub chart_id: EmbeddedChartId,
    /// Optional local series identity.
    pub series_id: Option<SectionSeriesId>,
    /// Optional primitive index.
    pub primitive_index: Option<usize>,
    /// Local ternary coordinate.
    pub local_position: TernaryPoint,
    /// Parent tetrahedral coordinate.
    pub tetrahedral_position: crate::TetraPoint,
    /// World position.
    pub world_position: [f32; 3],
    /// Hit triangulated-surface triangle where available.
    pub surface_triangle: Option<u32>,
    /// Optional sampled scalar value.
    pub scalar_value: Option<f64>,
}
