//! Plot construction, operational scene ownership, and tetrahedral frame configuration.
use crate::{
    Color, EmbeddedChartId, EmbeddedTernaryChart, PlanarSection, PreparationContext,
    PreparedSeries, Result, SceneBounds, SectionId, SeriesId, TetraGeometry, TetraSeries,
    TetraplotError, Tolerance, ViewportAlignment, ViewportError, ViewportFit,
};
/// Projection used by the renderer-independent camera model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    Perspective { vertical_fov_radians: f64 },
    Orthographic { vertical_span: f64 },
}
/// A scientific-camera description independent of graphics or windowing APIs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    position: [f64; 3],
    target: [f64; 3],
    up: [f64; 3],
    projection: Projection,
    near: f64,
    far: f64,
}
impl Camera {
    pub fn new(
        position: [f64; 3],
        target: [f64; 3],
        up: [f64; 3],
        projection: Projection,
    ) -> std::result::Result<Self, ViewportError> {
        for (field, value) in [("position", position), ("target", target), ("up", up)] {
            if !value.iter().all(|value| value.is_finite()) {
                return Err(ViewportError::NonFiniteCamera { field, value });
            }
        }
        if distance(position, target) <= f64::EPSILON {
            return Err(ViewportError::CoincidentCameraTarget);
        }
        Ok(Self {
            position,
            target,
            up,
            projection,
            near: 0.01,
            far: 10_000.0,
        })
    }
    pub fn fit(bounds: SceneBounds) -> Self {
        let center = bounds.center();
        let radius = bounds.max_extent().max(1.0);
        Self::new(
            [
                center[0] + radius * 1.8,
                center[1] + radius * 1.4,
                center[2] + radius * 1.8,
            ],
            center,
            [0.0, 1.0, 0.0],
            Projection::Perspective {
                vertical_fov_radians: std::f64::consts::FRAC_PI_4,
            },
        )
        .expect("finite scene bounds")
    }
    pub const fn position(self) -> [f64; 3] {
        self.position
    }
    pub const fn target(self) -> [f64; 3] {
        self.target
    }
    pub const fn up(self) -> [f64; 3] {
        self.up
    }
    pub const fn projection(self) -> Projection {
        self.projection
    }
    pub const fn near(self) -> f64 {
        self.near
    }
    pub const fn far(self) -> f64 {
        self.far
    }
    pub fn with_clip_planes(
        mut self,
        near: f64,
        far: f64,
    ) -> std::result::Result<Self, ViewportError> {
        if !near.is_finite() || !far.is_finite() || near <= 0.0 || far <= near {
            return Err(ViewportError::InvalidBounds {
                min: [near, 0.0, 0.0],
                max: [far, 0.0, 0.0],
            });
        }
        self.near = near;
        self.far = far;
        Ok(self)
    }
}
/// Visible tetrahedral frame controls. Faces are intentionally disabled by default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameStyle {
    edge_color: Color,
    edge_width: f32,
    faces: bool,
    face_color: Color,
    vertex_labels: bool,
}
impl Default for FrameStyle {
    fn default() -> Self {
        Self {
            edge_color: crate::BLACK,
            edge_width: 1.5,
            faces: false,
            face_color: Color::rgb(0.7, 0.75, 0.85).with_alpha(0.15),
            vertex_labels: true,
        }
    }
}
impl FrameStyle {
    pub const fn edge_color(self) -> Color {
        self.edge_color
    }
    pub const fn edge_width(self) -> f32 {
        self.edge_width
    }
    pub const fn faces(self) -> bool {
        self.faces
    }
    pub const fn face_color(self) -> Color {
        self.face_color
    }
    pub const fn vertex_labels(self) -> bool {
        self.vertex_labels
    }
}
/// Commit-style builder for the visible tetrahedral frame.
pub struct TetraFrameConfig<'a> {
    plot: &'a mut Tetraplot,
    draft: FrameStyle,
}
impl<'a> TetraFrameConfig<'a> {
    pub const fn edge_style(mut self, color: Color) -> Self {
        self.draft.edge_color = color;
        self
    }
    pub const fn edge_width(mut self, width: f32) -> Self {
        self.draft.edge_width = width;
        self
    }
    pub const fn faces(mut self, visible: bool) -> Self {
        self.draft.faces = visible;
        self
    }
    pub const fn face_color(mut self, color: Color) -> Self {
        self.draft.face_color = color;
        self
    }
    pub const fn vertex_labels(mut self, visible: bool) -> Self {
        self.draft.vertex_labels = visible;
        self
    }
    pub fn draw(self) -> Result<()> {
        self.plot.frame = self.draft;
        self.plot.frame_revision += 1;
        Ok(())
    }
}
/// Builder resolving implicit camera and scene bounds only when [`Self::build`] is called.
pub struct TetraplotBuilder {
    geometry: TetraGeometry,
    camera: Option<Camera>,
    scene_bounds: Option<SceneBounds>,
    fit: ViewportFit,
    alignment: ViewportAlignment,
    caption: Option<String>,
    vertex_labels: [Option<String>; 4],
    background: Color,
    tolerance: Tolerance,
}
impl Default for TetraplotBuilder {
    fn default() -> Self {
        Self::new()
    }
}
impl TetraplotBuilder {
    pub fn new() -> Self {
        Self {
            geometry: TetraGeometry::default(),
            camera: None,
            scene_bounds: None,
            fit: ViewportFit::PreserveAspect,
            alignment: ViewportAlignment::Center,
            caption: None,
            vertex_labels: std::array::from_fn(|_| None),
            background: Color::rgb(0.98, 0.98, 0.99),
            tolerance: Tolerance::default(),
        }
    }
    pub const fn geometry(mut self, value: TetraGeometry) -> Self {
        self.geometry = value;
        self
    }
    pub const fn camera(mut self, value: Camera) -> Self {
        self.camera = Some(value);
        self
    }
    pub const fn scene_bounds(mut self, value: SceneBounds) -> Self {
        self.scene_bounds = Some(value);
        self
    }
    pub const fn viewport_fit(mut self, value: ViewportFit) -> Self {
        self.fit = value;
        self
    }
    pub const fn viewport_alignment(mut self, value: ViewportAlignment) -> Self {
        self.alignment = value;
        self
    }
    pub fn caption(mut self, value: impl Into<String>) -> Self {
        self.caption = Some(value.into());
        self
    }
    pub fn vertex_labels<S: AsRef<str>>(mut self, labels: [S; 4]) -> Self {
        self.vertex_labels = labels.map(|label| Some(label.as_ref().to_owned()));
        self
    }
    pub const fn background(mut self, value: Color) -> Self {
        self.background = value;
        self
    }
    pub const fn tolerance(mut self, value: Tolerance) -> Self {
        self.tolerance = value;
        self
    }
    pub fn build(self) -> Result<Tetraplot> {
        self.tolerance.validate()?;
        let bounds = self.scene_bounds.unwrap_or_else(|| self.geometry.bounds());
        Ok(Tetraplot {
            geometry: self.geometry,
            camera: self.camera.unwrap_or_else(|| Camera::fit(bounds)),
            scene_bounds: bounds,
            fit: self.fit,
            alignment: self.alignment,
            caption: self.caption,
            vertex_labels: self.vertex_labels,
            background: self.background,
            tolerance: self.tolerance,
            frame: FrameStyle::default(),
            frame_revision: 0,
            series: Vec::new(),
            next_series_id: 0,
            sections: Vec::new(),
            next_section_id: 0,
            embedded_charts: Vec::new(),
            next_embedded_id: 0,
        })
    }
    pub fn resolved_scene_bounds(&self) -> SceneBounds {
        self.scene_bounds.unwrap_or_else(|| self.geometry.bounds())
    }
    pub fn resolved_camera(&self) -> Camera {
        self.camera
            .unwrap_or_else(|| Camera::fit(self.resolved_scene_bounds()))
    }
}
/// A completed mutable tetrahedral plot and its prepared renderer-neutral scene.
pub struct Tetraplot {
    geometry: TetraGeometry,
    camera: Camera,
    scene_bounds: SceneBounds,
    fit: ViewportFit,
    alignment: ViewportAlignment,
    caption: Option<String>,
    vertex_labels: [Option<String>; 4],
    background: Color,
    tolerance: Tolerance,
    frame: FrameStyle,
    frame_revision: u64,
    series: Vec<(SeriesId, PreparedSeries)>,
    next_series_id: u64,
    sections: Vec<Option<PlanarSection>>,
    next_section_id: u64,
    embedded_charts: Vec<Option<EmbeddedTernaryChart>>,
    next_embedded_id: u64,
}
impl Tetraplot {
    pub fn builder() -> TetraplotBuilder {
        TetraplotBuilder::new()
    }
    pub fn geometry(&self) -> TetraGeometry {
        self.geometry
    }
    pub fn camera(&self) -> Camera {
        self.camera
    }
    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
    }
    pub fn scene_bounds(&self) -> SceneBounds {
        self.scene_bounds
    }
    pub fn tolerance(&self) -> Tolerance {
        self.tolerance
    }
    pub fn viewport_fit(&self) -> ViewportFit {
        self.fit
    }
    pub fn viewport_alignment(&self) -> ViewportAlignment {
        self.alignment
    }
    pub fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }
    pub fn vertex_labels(&self) -> &[Option<String>; 4] {
        &self.vertex_labels
    }
    pub fn background(&self) -> Color {
        self.background
    }
    pub fn set_background(&mut self, color: Color) {
        self.background = color.clamped();
    }
    pub fn configure_frame(&mut self) -> TetraFrameConfig<'_> {
        TetraFrameConfig {
            draft: self.frame,
            plot: self,
        }
    }
    pub fn draw_series<S: TetraSeries>(
        &mut self,
        series: S,
    ) -> std::result::Result<SeriesId, TetraplotError> {
        let prepared = series.prepare(PreparationContext {
            geometry: self.geometry,
            tolerance: self.tolerance,
        })?;
        let id = SeriesId(self.next_series_id);
        self.next_series_id += 1;
        self.series.push((id, prepared));
        Ok(id)
    }
    pub fn prepared_series(&self) -> &[(SeriesId, PreparedSeries)] {
        &self.series
    }
    pub fn add_section(&mut self, mut section: PlanarSection) -> Result<SectionId> {
        section.refresh(&self.geometry, self.tolerance)?;
        let id = SectionId::new(self.next_section_id);
        self.next_section_id += 1;
        section.assign_id(id);
        self.sections.push(Some(section));
        Ok(id)
    }
    pub fn section(&self, id: SectionId) -> Option<&PlanarSection> {
        self.sections
            .iter()
            .flatten()
            .find(|section| section.id() == Some(id))
    }
    pub fn section_mut(&mut self, id: SectionId) -> Option<&mut PlanarSection> {
        self.sections
            .iter_mut()
            .flatten()
            .find(|section| section.id() == Some(id))
    }
    pub fn remove_section(&mut self, id: SectionId) -> Option<PlanarSection> {
        let index = self.sections.iter().position(|section| {
            section
                .as_ref()
                .is_some_and(|section| section.id() == Some(id))
        })?;
        self.sections[index].take()
    }
    pub fn sections(&self) -> impl Iterator<Item = &PlanarSection> {
        self.sections.iter().flatten()
    }
    pub fn add_embedded_chart(
        &mut self,
        mut chart: EmbeddedTernaryChart,
    ) -> Result<EmbeddedChartId> {
        chart.prepared(&self.geometry, self.tolerance)?;
        let id = EmbeddedChartId::new(self.next_embedded_id);
        self.next_embedded_id += 1;
        chart.assign_id(id);
        self.embedded_charts.push(Some(chart));
        Ok(id)
    }
    pub fn embedded_chart(&self, id: EmbeddedChartId) -> Option<&EmbeddedTernaryChart> {
        self.embedded_charts
            .iter()
            .flatten()
            .find(|chart| chart.id() == Some(id))
    }
    pub fn embedded_chart_mut(&mut self, id: EmbeddedChartId) -> Option<&mut EmbeddedTernaryChart> {
        self.embedded_charts
            .iter_mut()
            .flatten()
            .find(|chart| chart.id() == Some(id))
    }
    pub fn remove_embedded_chart(&mut self, id: EmbeddedChartId) -> Option<EmbeddedTernaryChart> {
        let index = self
            .embedded_charts
            .iter()
            .position(|chart| chart.as_ref().is_some_and(|chart| chart.id() == Some(id)))?;
        self.embedded_charts[index].take()
    }
    pub fn embedded_charts(&self) -> impl Iterator<Item = &EmbeddedTernaryChart> {
        self.embedded_charts.iter().flatten()
    }
    pub(crate) fn frame(&self) -> FrameStyle {
        self.frame
    }
}
impl Default for Tetraplot {
    fn default() -> Self {
        TetraplotBuilder::new()
            .build()
            .expect("default geometry is valid")
    }
}
fn distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    ((left[0] - right[0]).powi(2) + (left[1] - right[1]).powi(2) + (left[2] - right[2]).powi(2))
        .sqrt()
}
