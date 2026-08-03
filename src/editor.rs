//! Editor-neutral document, selection, linked cursor, commands, and scientific picking.

use crate::{BreakLineId, EmbeddedChartId, SeriesId};
use crate::{
    ChartEmbedding, CompositionGrid, CompositionGridId, GridColumnId, GridCoordinate, GridError,
    GridRowId, PasteSummary, PreparedEmbeddedChart, ScalarFieldId, SectionId, SectionSeriesId,
    SurfacePatchId, TernaryPoint, TetraGeometry, TetraPoint, Tetraplot, Tolerance,
};

pub struct TetraplotDocument {
    plot: Tetraplot,
    grids: Vec<Option<CompositionGrid>>,
    next_grid_id: u64,
    modified: bool,
    revision: u64,
}
impl TetraplotDocument {
    pub fn new(plot: Tetraplot) -> Self {
        Self {
            plot,
            grids: Vec::new(),
            next_grid_id: 0,
            modified: false,
            revision: 0,
        }
    }
    pub fn plot(&self) -> &Tetraplot {
        &self.plot
    }
    pub fn plot_mut(&mut self) -> &mut Tetraplot {
        self.mark_modified();
        &mut self.plot
    }
    pub fn into_plot(self) -> Tetraplot {
        self.plot
    }
    pub const fn modified(&self) -> bool {
        self.modified
    }
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub fn mark_saved(&mut self) {
        self.modified = false;
    }
    pub fn add_grid(&mut self, mut grid: CompositionGrid) -> CompositionGridId {
        let id = CompositionGridId::new(self.next_grid_id);
        self.next_grid_id += 1;
        grid.assign_id(id);
        self.grids.push(Some(grid));
        self.mark_modified();
        id
    }
    pub fn grid(&self, id: CompositionGridId) -> Option<&CompositionGrid> {
        self.grids
            .iter()
            .flatten()
            .find(|grid| grid.id() == Some(id))
    }
    pub fn grid_mut(&mut self, id: CompositionGridId) -> Option<&mut CompositionGrid> {
        let index = self
            .grids
            .iter()
            .position(|grid| grid.as_ref().is_some_and(|grid| grid.id() == Some(id)))?;
        self.mark_modified();
        self.grids[index].as_mut()
    }
    pub fn remove_grid(&mut self, id: CompositionGridId) -> Option<CompositionGrid> {
        let index = self
            .grids
            .iter()
            .position(|grid| grid.as_ref().is_some_and(|grid| grid.id() == Some(id)))?;
        self.mark_modified();
        self.grids[index].take()
    }
    pub fn grids(&self) -> impl Iterator<Item = &CompositionGrid> {
        self.grids.iter().flatten()
    }
    pub fn remove_section(&mut self, id: SectionId) -> Option<crate::PlanarSection> {
        let result = self.plot.remove_section(id);
        if result.is_some() {
            self.mark_modified();
        }
        result
    }
    pub fn remove_embedded_chart(
        &mut self,
        id: EmbeddedChartId,
    ) -> Option<crate::EmbeddedTernaryChart> {
        let result = self.plot.remove_embedded_chart(id);
        if result.is_some() {
            self.mark_modified();
        }
        result
    }
    pub fn prepared_grid_points(
        &self,
        id: CompositionGridId,
    ) -> Result<Vec<crate::PreparedGridPoint>, GridError> {
        let grid = self.grid(id).ok_or(GridError::InvalidComposition {
            message: "grid does not exist".to_owned(),
        })?;
        prepare_grid_points(grid, &self.plot)
    }
    fn mark_modified(&mut self) {
        self.modified = true;
        self.revision += 1;
    }
}

fn prepare_grid_points(
    grid: &CompositionGrid,
    plot: &Tetraplot,
) -> Result<Vec<crate::PreparedGridPoint>, GridError> {
    let owner = grid.id().ok_or(GridError::InvalidComposition {
        message: "grid must be added to a document before preparation".to_owned(),
    })?;
    let space = grid.coordinate_space();
    let points = grid
        .row_ids()
        .into_iter()
        .filter_map(|row_id| {
            let coordinate = grid.row_coordinate(row_id).ok().flatten()?;
            let (local, tetrahedral) = match (space, coordinate) {
                (
                    crate::GridCoordinateSpace::Tetrahedral,
                    GridCoordinate::Tetrahedral(tetrahedral),
                ) => (None, tetrahedral),
                (
                    crate::GridCoordinateSpace::Section(section),
                    GridCoordinate::LocalTernary(local),
                ) => {
                    let embedding = plot.section(section)?.embedding().ok()?;
                    (Some(local), embedding.map(local, plot.tolerance()).ok()?)
                }
                (
                    crate::GridCoordinateSpace::EmbeddedChart(chart),
                    GridCoordinate::LocalTernary(local),
                ) => {
                    let embedding = plot.embedded_chart(chart)?.embedding();
                    (Some(local), embedding.map(local, plot.tolerance()).ok()?)
                }
                _ => return None,
            };
            Some(crate::PreparedGridPoint {
                grid_id: owner,
                row_id,
                local,
                tetrahedral,
                world: plot
                    .geometry()
                    .to_world(tetrahedral)
                    .map(|value| value as f32),
                scalar_values: grid.scalar_values(row_id).ok()?,
            })
        })
        .collect();
    Ok(points)
}
#[derive(Clone, Debug, Eq, Hash, PartialEq, Default)]
pub enum Selection {
    #[default]
    None,
    Frame,
    Series(SeriesId),
    Section(SectionId),
    SectionSeries {
        section: SectionId,
        series: SectionSeriesId,
    },
    EmbeddedChart(EmbeddedChartId),
    EmbeddedSeries {
        chart: EmbeddedChartId,
        series: SectionSeriesId,
    },
    SurfacePatch {
        chart: EmbeddedChartId,
        patch: SurfacePatchId,
    },
    BreakLine {
        chart: EmbeddedChartId,
        line: BreakLineId,
    },
    Grid(CompositionGridId),
    ScalarField {
        grid: CompositionGridId,
        field: ScalarFieldId,
    },
    GridRow {
        grid: CompositionGridId,
        row: GridRowId,
    },
    GridCell {
        grid: CompositionGridId,
        row: GridRowId,
        column: GridColumnId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum FlatViewTarget {
    None,
    #[default]
    FollowSelection,
    Section(SectionId),
    EmbeddedChart(EmbeddedChartId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum LinkedCursorOwner {
    Section(SectionId),
    EmbeddedChart(EmbeddedChartId),
}
#[derive(Clone, Debug, PartialEq)]
pub struct LinkedCursor {
    pub owner: LinkedCursorOwner,
    pub local_position: TernaryPoint,
    pub tetrahedral_position: TetraPoint,
    pub world_position: [f64; 3],
    pub surface_triangle: Option<u32>,
    pub patch: Option<SurfacePatchId>,
}

#[derive(Clone, Debug, Default)]
pub struct EditorState {
    pub selection: Selection,
    pub flat_view: FlatViewTarget,
    pub linked_cursor: Option<LinkedCursor>,
    pub active_grid: Option<CompositionGridId>,
    pub status: Option<String>,
    revision: u64,
}
impl EditorState {
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub fn select(&mut self, selection: Selection) {
        self.selection = selection;
        self.revision += 1;
    }
    pub fn clear_cursor(&mut self) {
        if self.linked_cursor.take().is_some() {
            self.revision += 1;
        }
    }
    pub fn clear_removed(&mut self, document: &TetraplotDocument) {
        let valid = match self.selection {
            Selection::None | Selection::Frame => true,
            Selection::Series(_) => true,
            Selection::Section(id) | Selection::SectionSeries { section: id, .. } => {
                document.plot().section(id).is_some()
            }
            Selection::EmbeddedChart(id)
            | Selection::EmbeddedSeries { chart: id, .. }
            | Selection::SurfacePatch { chart: id, .. }
            | Selection::BreakLine { chart: id, .. } => {
                document.plot().embedded_chart(id).is_some()
            }
            Selection::Grid(id)
            | Selection::ScalarField { grid: id, .. }
            | Selection::GridRow { grid: id, .. }
            | Selection::GridCell { grid: id, .. } => document.grid(id).is_some(),
        };
        if !valid {
            self.select(Selection::None);
            self.clear_cursor();
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorPickResult {
    pub selection: Selection,
    pub local_position: Option<TernaryPoint>,
    pub tetrahedral_position: Option<TetraPoint>,
    pub world_position: [f64; 3],
    pub surface_triangle: Option<u32>,
    pub patch: Option<SurfacePatchId>,
    pub series_id: Option<SectionSeriesId>,
    pub primitive_index: Option<usize>,
    pub grid: Option<CompositionGridId>,
    pub grid_row: Option<GridRowId>,
    pub scalar_value: Option<f64>,
    pub break_line: Option<BreakLineId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: [f64; 3],
    pub direction: [f64; 3],
}
impl Ray {
    pub fn new(origin: [f64; 3], direction: [f64; 3]) -> Option<Self> {
        let length = length(direction);
        if origin
            .iter()
            .chain(direction.iter())
            .all(|value| value.is_finite())
            && length > 0.0
        {
            Some(Self {
                origin,
                direction: scale(direction, 1.0 / length),
            })
        } else {
            None
        }
    }
}

pub fn pick_prepared_surface(
    ray: Ray,
    prepared: &PreparedEmbeddedChart,
    _geometry: &TetraGeometry,
    tolerance: Tolerance,
) -> Option<SurfaceHit> {
    let mut nearest: Option<(f64, SurfaceHit)> = None;
    for triangle in &prepared.surface.triangles {
        let vertices = triangle
            .indices
            .map(|index| &prepared.surface.vertices[index as usize]);
        let world = vertices.map(|vertex| vertex.world.map(f64::from));
        let Some((distance, weights)) = ray_triangle(ray, world) else {
            continue;
        };
        if nearest
            .as_ref()
            .is_some_and(|(nearest_distance, _)| *nearest_distance <= distance)
        {
            continue;
        }
        let local = TernaryPoint::from_unchecked(weighted3(
            vertices.map(|vertex| vertex.local.as_array()),
            weights,
        ));
        let tetrahedral = TetraPoint::from_unchecked(weighted4(
            vertices.map(|vertex| vertex.tetrahedral.as_array()),
            weights,
        ));
        let point = add(ray.origin, scale(ray.direction, distance));
        let break_line = nearest_break_line(point, &prepared.break_lines, tolerance);
        nearest = Some((
            distance,
            SurfaceHit {
                local,
                tetrahedral,
                world: point,
                surface_triangle: triangle.source_triangle,
                patch: triangle.patch,
                break_line,
            },
        ));
    }
    nearest.map(|(_, hit)| hit)
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceHit {
    pub local: TernaryPoint,
    pub tetrahedral: TetraPoint,
    pub world: [f64; 3],
    pub surface_triangle: u32,
    pub patch: SurfacePatchId,
    pub break_line: Option<BreakLineId>,
}

pub fn pick_embedded_chart(
    ray: Ray,
    chart: EmbeddedChartId,
    embedding: &ChartEmbedding,
    prepared: &PreparedEmbeddedChart,
    _geometry: &TetraGeometry,
    tolerance: Tolerance,
) -> Option<EditorPickResult> {
    let hit = pick_prepared_surface(ray, prepared, _geometry, tolerance)?;
    let _ = embedding;
    Some(EditorPickResult {
        selection: Selection::SurfacePatch {
            chart,
            patch: hit.patch,
        },
        local_position: Some(hit.local),
        tetrahedral_position: Some(hit.tetrahedral),
        world_position: hit.world,
        surface_triangle: Some(hit.surface_triangle),
        patch: Some(hit.patch),
        series_id: None,
        primitive_index: Some(hit.surface_triangle as usize),
        grid: None,
        grid_row: None,
        scalar_value: None,
        break_line: hit.break_line,
    })
}

fn nearest_break_line(
    point: [f64; 3],
    lines: &[crate::PreparedBreakLine],
    tolerance: Tolerance,
) -> Option<BreakLineId> {
    let epsilon = tolerance.absolute.max(1.0e-6);
    lines.iter().find_map(|line| {
        line.line
            .segments
            .iter()
            .any(|segment| {
                distance_to_segment(point, segment.world.map(|world| world.map(f64::from)))
                    <= epsilon
            })
            .then_some(line.id)
    })
}
fn ray_triangle(ray: Ray, vertices: [[f64; 3]; 3]) -> Option<(f64, [f64; 3])> {
    let edge1 = sub(vertices[1], vertices[0]);
    let edge2 = sub(vertices[2], vertices[0]);
    let determinant = dot(edge1, cross(ray.direction, edge2));
    if determinant.abs() <= 1.0e-12 {
        return None;
    }
    let inverse = determinant.recip();
    let start = sub(ray.origin, vertices[0]);
    let u = dot(start, cross(ray.direction, edge2)) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let v = dot(ray.direction, cross(start, edge1)) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = dot(edge2, cross(start, edge1)) * inverse;
    (distance >= 0.0).then_some((distance, [1.0 - u - v, u, v]))
}
fn weighted3(values: [[f64; 3]; 3], weights: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| {
        values
            .iter()
            .zip(weights)
            .map(|(value, weight)| value[axis] * weight)
            .sum()
    })
}
fn weighted4(values: [[f64; 4]; 3], weights: [f64; 3]) -> [f64; 4] {
    std::array::from_fn(|axis| {
        values
            .iter()
            .zip(weights)
            .map(|(value, weight)| value[axis] * weight)
            .sum()
    })
}
fn distance_to_segment(point: [f64; 3], segment: [[f64; 3]; 2]) -> f64 {
    let direction = sub(segment[1], segment[0]);
    let denominator = dot(direction, direction);
    let t = if denominator <= 0.0 {
        0.0
    } else {
        dot(sub(point, segment[0]), direction) / denominator
    }
    .clamp(0.0, 1.0);
    length(sub(point, add(segment[0], scale(direction, t))))
}
fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| left[axis] + right[axis])
}
fn sub(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| left[axis] - right[axis])
}
fn scale(value: [f64; 3], scalar: f64) -> [f64; 3] {
    value.map(|value| value * scalar)
}
fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left.into_iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}
fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}
fn length(value: [f64; 3]) -> f64 {
    dot(value, value).sqrt()
}

#[derive(Clone, Debug)]
pub enum EditorCommand {
    SetSelection(Selection),
    SetVisibility {
        target: Selection,
        visible: bool,
    },
    SetOpacity {
        target: Selection,
        opacity: f32,
    },
    SetSectionValue {
        section: SectionId,
        value: f64,
    },
    OpenFlatView(FlatViewTarget),
    CloseFlatView,
    OpenDataGrid(CompositionGridId),
    EditGridScalar {
        grid: CompositionGridId,
        row: GridRowId,
        field: ScalarFieldId,
        value: Option<f64>,
    },
    PasteScalarColumn {
        grid: CompositionGridId,
        field: ScalarFieldId,
        start: GridRowId,
        clipboard: crate::ClipboardTable,
    },
    PasteGridBlock {
        grid: CompositionGridId,
        clipboard: crate::ClipboardTable,
    },
    AddScalarField {
        grid: CompositionGridId,
        name: String,
    },
    RemoveScalarField {
        grid: CompositionGridId,
        field: ScalarFieldId,
    },
    FitCamera,
    ResetCamera,
}

impl EditorCommand {
    pub fn execute(
        self,
        document: &mut TetraplotDocument,
        state: &mut EditorState,
    ) -> Result<Option<PasteSummary>, crate::TetraplotError> {
        match self {
            Self::SetSelection(selection) => {
                state.select(selection);
                Ok(None)
            }
            Self::OpenFlatView(target) => {
                state.flat_view = target;
                state.revision += 1;
                Ok(None)
            }
            Self::CloseFlatView => {
                state.flat_view = FlatViewTarget::None;
                state.clear_cursor();
                Ok(None)
            }
            Self::OpenDataGrid(grid) => {
                if document.grid(grid).is_none() {
                    return Err(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    }
                    .into());
                }
                state.active_grid = Some(grid);
                state.select(Selection::Grid(grid));
                Ok(None)
            }
            Self::SetSectionValue { section, value } => {
                let geometry = document.plot.geometry();
                let tolerance = document.plot.tolerance();
                document
                    .plot_mut()
                    .section_mut(section)
                    .ok_or(crate::SectionError::UnknownSection { id: section.get() })?
                    .set_constant_component_value(value, &geometry, tolerance)?;
                Ok(None)
            }
            Self::SetVisibility { target, visible } => {
                apply_visibility(document, target, visible)?;
                Ok(None)
            }
            Self::SetOpacity { target, opacity } => {
                apply_opacity(document, target, opacity)?;
                Ok(None)
            }
            Self::EditGridScalar {
                grid,
                row,
                field,
                value,
            } => {
                document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .set_scalar(row, field, value)?;
                Ok(None)
            }
            Self::PasteScalarColumn {
                grid,
                field,
                start,
                clipboard,
            } => {
                let summary = document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .paste_scalar_column(field, start, &clipboard)?;
                Ok(Some(summary))
            }
            Self::PasteGridBlock { grid, clipboard } => {
                let tolerance = document.plot.tolerance();
                let summary = document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .append_clipboard(&clipboard, tolerance)?;
                Ok(Some(summary))
            }
            Self::AddScalarField { grid, name } => {
                document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .add_scalar_field(name);
                Ok(None)
            }
            Self::RemoveScalarField { grid, field } => {
                document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .remove_scalar_field(field)
                    .ok_or(GridError::UnknownScalarField { id: field.get() })?;
                Ok(None)
            }
            Self::FitCamera | Self::ResetCamera => {
                let camera = crate::Camera::fit(document.plot().scene_bounds());
                document.plot_mut().set_camera(camera);
                Ok(None)
            }
        }
    }
}
fn apply_visibility(
    document: &mut TetraplotDocument,
    target: Selection,
    visible: bool,
) -> Result<(), crate::TetraplotError> {
    match target {
        Selection::Section(id) => document
            .plot_mut()
            .section_mut(id)
            .ok_or(crate::SectionError::UnknownSection { id: id.get() })?
            .set_visible(visible),
        Selection::EmbeddedChart(id) => document
            .plot_mut()
            .embedded_chart_mut(id)
            .ok_or(crate::SectionError::UnknownSection { id: id.get() })?
            .set_visible(visible),
        _ => {}
    }
    Ok(())
}
fn apply_opacity(
    document: &mut TetraplotDocument,
    target: Selection,
    opacity: f32,
) -> Result<(), crate::TetraplotError> {
    match target {
        Selection::EmbeddedChart(id) => {
            let chart = document
                .plot_mut()
                .embedded_chart_mut(id)
                .ok_or(crate::SectionError::UnknownSection { id: id.get() })?;
            let mut style = chart.style();
            style.fill_opacity = opacity.clamp(0.0, 1.0);
            chart.set_style(style);
        }
        Selection::Section(id) => {
            let section = document
                .plot_mut()
                .section_mut(id)
                .ok_or(crate::SectionError::UnknownSection { id: id.get() })?;
            let mut style = section.style();
            style.fill_opacity = opacity.clamp(0.0, 1.0);
            section.set_style(style);
        }
        _ => {}
    }
    Ok(())
}
