//! Editor-neutral document, selection, linked cursor, commands, and scientific picking.

use std::collections::BTreeMap;

use crate::{BreakLineId, EmbeddedChartId, SeriesId};
use crate::{
    ChartEmbedding, ClipboardTable, CompositionEntryMode, CompositionGrid, CompositionGridId,
    DataTableState, DuplicateCompositionPolicy, GridCellAddress, GridColumnId, GridCoordinate,
    GridError, GridRowId, PasteSummary, PreparedEmbeddedChart, PreparedGridPoint, ScalarFieldId,
    SectionId, SectionSeriesId, SurfacePatchId, TernaryPoint, TetraGeometry, TetraPoint, Tetraplot,
    Tolerance, grid_columns, set_grid_cell_text,
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
    pub fn flat_revision(&self, target: FlatViewTarget) -> u64 {
        let mut revision = 0xcbf2_9ce4_8422_2325u64;
        let mut mix = |value: u64| {
            revision ^= value;
            revision = revision.wrapping_mul(0x100_0000_01b3);
        };
        match target {
            FlatViewTarget::Section(id) => {
                mix(1);
                mix(id.get());
                if let Some(section) = self.plot.section(id) {
                    for value in section.revision_key() {
                        mix(value);
                    }
                }
            }
            FlatViewTarget::EmbeddedChart(id) => {
                mix(2);
                mix(id.get());
                if let Some(chart) = self.plot.embedded_chart(id) {
                    for value in chart.revision_key() {
                        mix(value);
                    }
                }
            }
            FlatViewTarget::None | FlatViewTarget::FollowSelection => mix(0),
        }
        for grid in self.grids() {
            let applies = match (target, grid.coordinate_space()) {
                (FlatViewTarget::Section(target), crate::GridCoordinateSpace::Section(owner)) => {
                    target == owner
                }
                (
                    FlatViewTarget::EmbeddedChart(target),
                    crate::GridCoordinateSpace::EmbeddedChart(owner),
                ) => target == owner,
                _ => false,
            };
            if applies {
                mix(grid.id().map_or(u64::MAX, CompositionGridId::get));
                mix(grid.coordinate_revision());
                mix(grid.scalar_revision());
                mix(grid.structure_revision());
            }
        }
        revision
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
    match space {
        crate::GridCoordinateSpace::Section(id) if plot.section(id).is_none() => {
            return Err(GridError::CoordinateSpaceMismatch);
        }
        crate::GridCoordinateSpace::EmbeddedChart(id) if plot.embedded_chart(id).is_none() => {
            return Err(GridError::CoordinateSpaceMismatch);
        }
        _ => {}
    }
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
    pub data_tables: BTreeMap<CompositionGridId, DataTableState>,
    pub status: Option<String>,
    revision: u64,
    selection_revision: u64,
    cursor_revision: u64,
    flat_revision: u64,
}
impl EditorState {
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub const fn selection_revision(&self) -> u64 {
        self.selection_revision
    }
    pub const fn cursor_revision(&self) -> u64 {
        self.cursor_revision
    }
    pub const fn flat_revision(&self) -> u64 {
        self.flat_revision
    }
    pub fn select(&mut self, selection: Selection) {
        if self.selection != selection {
            self.selection = selection;
            self.selection_revision += 1;
            self.revision += 1;
        }
    }
    pub fn set_cursor(&mut self, cursor: Option<LinkedCursor>) {
        if self.linked_cursor != cursor {
            self.linked_cursor = cursor;
            self.cursor_revision += 1;
            self.revision += 1;
        }
    }
    pub fn clear_cursor(&mut self) {
        self.set_cursor(None);
    }
    pub fn open_data_grid(&mut self, grid: CompositionGridId) {
        self.active_grid = Some(grid);
        self.data_tables
            .entry(grid)
            .or_insert_with(|| DataTableState::new(grid));
        self.revision += 1;
    }
    pub fn table_state(&self, grid: CompositionGridId) -> Option<&DataTableState> {
        self.data_tables.get(&grid)
    }
    pub fn table_state_mut(&mut self, grid: CompositionGridId) -> &mut DataTableState {
        self.data_tables
            .entry(grid)
            .or_insert_with(|| DataTableState::new(grid))
    }
    pub fn set_flat_view(&mut self, target: FlatViewTarget) {
        if self.flat_view != target {
            self.flat_view = target;
            self.flat_revision += 1;
            self.revision += 1;
        }
    }
    pub fn clear_removed(&mut self, document: &TetraplotDocument) {
        self.data_tables
            .retain(|grid, _| document.grid(*grid).is_some());
        if self
            .active_grid
            .is_some_and(|grid| document.grid(grid).is_none())
        {
            self.active_grid = None;
        }
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
            Selection::Grid(id) => document.grid(id).is_some(),
            Selection::ScalarField { grid, field } => document
                .grid(grid)
                .is_some_and(|value| value.field(field).is_some()),
            Selection::GridRow { grid, row } => document
                .grid(grid)
                .is_some_and(|value| value.row_ids().contains(&row)),
            Selection::GridCell { grid, row, column } => document.grid(grid).is_some_and(|value| {
                value.row_ids().contains(&row)
                    && grid_columns(value)
                        .iter()
                        .any(|candidate| candidate.id == column)
            }),
        };
        if !valid {
            self.select(Selection::None);
            self.clear_cursor();
        }
        let flat_valid = match self.flat_view {
            FlatViewTarget::None | FlatViewTarget::FollowSelection => true,
            FlatViewTarget::Section(id) => document.plot().section(id).is_some(),
            FlatViewTarget::EmbeddedChart(id) => document.plot().embedded_chart(id).is_some(),
        };
        if !flat_valid {
            self.set_flat_view(FlatViewTarget::None);
            self.clear_cursor();
        }
        let cursor_valid = self
            .linked_cursor
            .as_ref()
            .is_none_or(|cursor| match cursor.owner {
                LinkedCursorOwner::Section(id) => document.plot().section(id).is_some(),
                LinkedCursorOwner::EmbeddedChart(id) => {
                    document.plot().embedded_chart(id).is_some()
                }
            });
        if !cursor_valid {
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalViewportRect {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalViewport {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorViewport {
    pub logical_rect: LogicalViewportRect,
    pub physical_rect: PhysicalViewport,
    pub device_pixel_ratio: f32,
}

impl EditorViewport {
    pub fn from_logical(
        logical_rect: LogicalViewportRect,
        window_size: [u32; 2],
        device_pixel_ratio: f32,
    ) -> Option<Self> {
        if !device_pixel_ratio.is_finite()
            || device_pixel_ratio <= 0.0
            || !logical_rect
                .min
                .iter()
                .chain(logical_rect.max.iter())
                .all(|value| value.is_finite())
        {
            return None;
        }
        let min_x = (logical_rect.min[0] * device_pixel_ratio)
            .round()
            .clamp(0.0, window_size[0] as f32);
        let max_x = (logical_rect.max[0] * device_pixel_ratio)
            .round()
            .clamp(0.0, window_size[0] as f32);
        let top = (logical_rect.min[1] * device_pixel_ratio)
            .round()
            .clamp(0.0, window_size[1] as f32);
        let bottom = (logical_rect.max[1] * device_pixel_ratio)
            .round()
            .clamp(0.0, window_size[1] as f32);
        let width = (max_x - min_x).max(0.0) as u32;
        let height = (bottom - top).max(0.0) as u32;
        if width < 2 || height < 2 {
            return None;
        }
        Some(Self {
            logical_rect,
            physical_rect: PhysicalViewport {
                x: min_x as i32,
                y: (window_size[1] as f32 - bottom) as i32,
                width,
                height,
            },
            device_pixel_ratio,
        })
    }

    pub fn contains_physical(self, pixel: [f32; 2]) -> bool {
        let rect = self.physical_rect;
        pixel[0] >= rect.x as f32
            && pixel[0] <= (rect.x + rect.width as i32) as f32
            && pixel[1] >= rect.y as f32
            && pixel[1] <= (rect.y + rect.height as i32) as f32
    }
}

pub fn pick_grid_points(
    ray: Ray,
    points: &[PreparedGridPoint],
    radius: f64,
) -> Option<EditorPickResult> {
    if !radius.is_finite() || radius <= 0.0 {
        return None;
    }
    let mut nearest: Option<(f64, &PreparedGridPoint)> = None;
    for point in points {
        let world = point.world.map(f64::from);
        let along = dot(sub(world, ray.origin), ray.direction);
        if along < 0.0 {
            continue;
        }
        let closest = add(ray.origin, scale(ray.direction, along));
        if length(sub(world, closest)) > radius {
            continue;
        }
        if nearest
            .as_ref()
            .is_none_or(|(distance, _)| along < *distance)
        {
            nearest = Some((along, point));
        }
    }
    nearest.map(|(_, point)| EditorPickResult {
        selection: Selection::GridRow {
            grid: point.grid_id,
            row: point.row_id,
        },
        local_position: point.local,
        tetrahedral_position: Some(point.tetrahedral),
        world_position: point.world.map(f64::from),
        surface_triangle: None,
        patch: None,
        series_id: None,
        primitive_index: None,
        grid: Some(point.grid_id),
        grid_row: Some(point.row_id),
        scalar_value: point.scalar_values.iter().flatten().next().copied(),
        break_line: None,
    })
}

/// Picks ordinary tetrahedral point and line series while preserving their stable series IDs.
pub fn pick_plot_series(ray: Ray, plot: &Tetraplot, radius: f64) -> Option<EditorPickResult> {
    if !radius.is_finite() || radius <= 0.0 {
        return None;
    }
    let mut nearest: Option<(f64, EditorPickResult)> = None;
    for (series_id, series) in plot.prepared_series() {
        match series {
            crate::PreparedSeries::Points { points, .. } => {
                for point in points {
                    let world = point.world.map(f64::from);
                    let along = dot(sub(world, ray.origin), ray.direction);
                    if along < 0.0 {
                        continue;
                    }
                    let closest = add(ray.origin, scale(ray.direction, along));
                    if length(sub(world, closest)) > radius {
                        continue;
                    }
                    let hit = EditorPickResult {
                        selection: Selection::Series(*series_id),
                        local_position: None,
                        tetrahedral_position: Some(TetraPoint::from_unchecked(point.barycentric)),
                        world_position: world,
                        surface_triangle: None,
                        patch: None,
                        series_id: None,
                        primitive_index: Some(point.source_index),
                        grid: None,
                        grid_row: None,
                        scalar_value: None,
                        break_line: None,
                    };
                    if nearest
                        .as_ref()
                        .is_none_or(|(nearest_distance, _)| along < *nearest_distance)
                    {
                        nearest = Some((along, hit));
                    }
                }
            }
            crate::PreparedSeries::Line(line) => {
                for path in &line.paths {
                    for (primitive_index, pair) in path.windows(2).enumerate() {
                        let world = [pair[0].world.map(f64::from), pair[1].world.map(f64::from)];
                        let Some((along, segment_t)) = ray_segment_proximity(ray, world, radius)
                        else {
                            continue;
                        };
                        let tetrahedral = TetraPoint::from_unchecked(std::array::from_fn(|axis| {
                            pair[0].barycentric[axis] * (1.0 - segment_t)
                                + pair[1].barycentric[axis] * segment_t
                        }));
                        let hit = EditorPickResult {
                            selection: Selection::Series(*series_id),
                            local_position: None,
                            tetrahedral_position: Some(tetrahedral),
                            world_position: add(
                                world[0],
                                scale(sub(world[1], world[0]), segment_t),
                            ),
                            surface_triangle: None,
                            patch: None,
                            series_id: None,
                            primitive_index: Some(primitive_index),
                            grid: None,
                            grid_row: None,
                            scalar_value: None,
                            break_line: None,
                        };
                        if nearest
                            .as_ref()
                            .is_none_or(|(nearest_distance, _)| along < *nearest_distance)
                        {
                            nearest = Some((along, hit));
                        }
                    }
                }
            }
            crate::PreparedSeries::Surface(_) => {}
        }
    }
    nearest.map(|(_, hit)| hit)
}

#[derive(Clone, Copy)]
enum PreparedChartOwner {
    Section(SectionId),
    Embedded(EmbeddedChartId),
}

/// Picks point and line primitives from a prepared embedded diagram.
pub fn pick_embedded_series(
    ray: Ray,
    chart: EmbeddedChartId,
    prepared: &PreparedEmbeddedChart,
    radius: f64,
) -> Option<EditorPickResult> {
    pick_prepared_chart_series(ray, PreparedChartOwner::Embedded(chart), prepared, radius)
}

/// Picks point and line primitives from a prepared planar-section diagram.
pub fn pick_section_series(
    ray: Ray,
    section: SectionId,
    prepared: &PreparedEmbeddedChart,
    radius: f64,
) -> Option<EditorPickResult> {
    pick_prepared_chart_series(ray, PreparedChartOwner::Section(section), prepared, radius)
}

fn pick_prepared_chart_series(
    ray: Ray,
    owner: PreparedChartOwner,
    prepared: &PreparedEmbeddedChart,
    radius: f64,
) -> Option<EditorPickResult> {
    if !radius.is_finite() || radius <= 0.0 {
        return None;
    }

    let selection = |series| match owner {
        PreparedChartOwner::Section(section) => Selection::SectionSeries { section, series },
        PreparedChartOwner::Embedded(chart) => Selection::EmbeddedSeries { chart, series },
    };
    let mut nearest: Option<(f64, EditorPickResult)> = None;

    for series in &prepared.points {
        for point in &series.points {
            let world = point.world.map(f64::from);
            let along = dot(sub(world, ray.origin), ray.direction);
            if along < 0.0 {
                continue;
            }
            let closest = add(ray.origin, scale(ray.direction, along));
            if length(sub(world, closest)) > radius {
                continue;
            }
            let hit = EditorPickResult {
                selection: selection(series.series_id),
                local_position: Some(point.location.local),
                tetrahedral_position: Some(point.location.tetrahedral),
                world_position: world,
                surface_triangle: Some(point.location.triangle),
                patch: Some(point.location.patch),
                series_id: Some(series.series_id),
                primitive_index: Some(point.source_index),
                grid: None,
                grid_row: None,
                scalar_value: None,
                break_line: None,
            };
            if nearest
                .as_ref()
                .is_none_or(|(nearest_distance, _)| along < *nearest_distance)
            {
                nearest = Some((along, hit));
            }
        }
    }

    for line in &prepared.lines {
        let Some(series_id) = line.series_id else {
            continue;
        };
        for (primitive_index, segment) in line.segments.iter().enumerate() {
            let world = segment.world.map(|point| point.map(f64::from));
            let Some((along, segment_t)) = ray_segment_proximity(ray, world, radius) else {
                continue;
            };
            let local = TernaryPoint::from_unchecked(std::array::from_fn(|axis| {
                segment.local[0].as_array()[axis] * (1.0 - segment_t)
                    + segment.local[1].as_array()[axis] * segment_t
            }));
            let tetrahedral = TetraPoint::from_unchecked(std::array::from_fn(|axis| {
                segment.tetrahedral[0].as_array()[axis] * (1.0 - segment_t)
                    + segment.tetrahedral[1].as_array()[axis] * segment_t
            }));
            let world_position = add(world[0], scale(sub(world[1], world[0]), segment_t));
            let hit = EditorPickResult {
                selection: selection(series_id),
                local_position: Some(local),
                tetrahedral_position: Some(tetrahedral),
                world_position,
                surface_triangle: Some(segment.surface_triangle),
                patch: Some(segment.patch),
                series_id: Some(series_id),
                primitive_index: Some(primitive_index),
                grid: None,
                grid_row: None,
                scalar_value: None,
                break_line: segment.start_break.or(segment.end_break),
            };
            if nearest
                .as_ref()
                .is_none_or(|(nearest_distance, _)| along < *nearest_distance)
            {
                nearest = Some((along, hit));
            }
        }
    }

    nearest.map(|(_, hit)| hit)
}

fn ray_segment_proximity(ray: Ray, segment: [[f64; 3]; 2], radius: f64) -> Option<(f64, f64)> {
    let edge = sub(segment[1], segment[0]);
    let from_origin = sub(segment[0], ray.origin);
    let edge_perpendicular = sub(edge, scale(ray.direction, dot(edge, ray.direction)));
    let origin_perpendicular = sub(
        from_origin,
        scale(ray.direction, dot(from_origin, ray.direction)),
    );
    let denominator = dot(edge_perpendicular, edge_perpendicular);
    let segment_t = if denominator <= f64::EPSILON {
        0.0
    } else {
        (-dot(origin_perpendicular, edge_perpendicular) / denominator).clamp(0.0, 1.0)
    };
    let point = add(segment[0], scale(edge, segment_t));
    let along = dot(sub(point, ray.origin), ray.direction);
    if along < 0.0 {
        return None;
    }
    let ray_point = add(ray.origin, scale(ray.direction, along));
    (length(sub(point, ray_point)) <= radius).then_some((along, segment_t))
}

pub fn select_best_pick(
    ray: Ray,
    candidates: impl IntoIterator<Item = EditorPickResult>,
) -> Option<EditorPickResult> {
    candidates.into_iter().min_by(|left, right| {
        pick_priority(&left.selection)
            .cmp(&pick_priority(&right.selection))
            .then_with(|| {
                squared_distance(ray.origin, left.world_position)
                    .total_cmp(&squared_distance(ray.origin, right.world_position))
            })
    })
}

fn pick_priority(selection: &Selection) -> u8 {
    match selection {
        Selection::GridRow { .. }
        | Selection::Series(_)
        | Selection::SectionSeries { .. }
        | Selection::EmbeddedSeries { .. } => 0,
        Selection::BreakLine { .. } => 1,
        Selection::Section(_) | Selection::EmbeddedChart(_) | Selection::SurfacePatch { .. } => 2,
        Selection::Frame => 3,
        _ => 4,
    }
}

fn squared_distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    left.into_iter()
        .zip(right)
        .map(|(left, right)| (left - right).powi(2))
        .sum()
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
    _embedding: &ChartEmbedding,
    prepared: &PreparedEmbeddedChart,
    _geometry: &TetraGeometry,
    tolerance: Tolerance,
) -> Option<EditorPickResult> {
    let hit = pick_prepared_surface(ray, prepared, _geometry, tolerance)?;
    let selection = hit.break_line.map_or(
        Selection::SurfacePatch {
            chart,
            patch: hit.patch,
        },
        |line| Selection::BreakLine { chart, line },
    );
    Some(EditorPickResult {
        selection,
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
    let epsilon = tolerance.absolute.max(1.0e-3);
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
    SetSectionName {
        section: SectionId,
        name: Option<String>,
    },
    SetEmbeddedChartName {
        chart: EmbeddedChartId,
        name: Option<String>,
    },
    SetEmbeddedSurfaceVisible {
        target: Selection,
        visible: bool,
    },
    SetEmbeddedGridVisible {
        target: Selection,
        visible: bool,
    },
    SetEmbeddedBoundaryVisible {
        target: Selection,
        visible: bool,
    },
    SetFrameEdgeColor(crate::Color),
    SetFrameEdgeWidth(f32),
    SetFrameFacesVisible(bool),
    SetFrameFaceColor(crate::Color),
    SetBackground(crate::Color),
    SetEmbeddedNormalMode {
        target: Selection,
        mode: crate::SurfaceNormalMode,
    },
    SetBreakLineStyle {
        chart: EmbeddedChartId,
        line: BreakLineId,
        style: crate::BreakLineStyle,
    },
    SetBreakLineKind {
        chart: EmbeddedChartId,
        line: BreakLineId,
        kind: crate::BreakLineKind,
    },
    OpenFlatView(FlatViewTarget),
    CloseFlatView,
    OpenDataGrid(CompositionGridId),
    SetGridName {
        grid: CompositionGridId,
        name: String,
    },
    RedefineRegularGrid {
        grid: CompositionGridId,
        definition: crate::RegularGridDefinition,
        policy: crate::GridRedefinitionPolicy,
    },
    SetEntryMode {
        grid: CompositionGridId,
        mode: CompositionEntryMode,
    },
    SetDuplicatePolicy {
        grid: CompositionGridId,
        policy: DuplicateCompositionPolicy,
    },
    EditGridComponent {
        grid: CompositionGridId,
        row: GridRowId,
        component: usize,
        text: String,
    },
    EditGridScalar {
        grid: CompositionGridId,
        row: GridRowId,
        field: ScalarFieldId,
        value: Option<f64>,
    },
    EditGridScalarText {
        grid: CompositionGridId,
        row: GridRowId,
        field: ScalarFieldId,
        text: String,
    },
    PasteScalarColumn {
        grid: CompositionGridId,
        field: ScalarFieldId,
        start: GridRowId,
        clipboard: ClipboardTable,
    },
    PasteGridCells {
        grid: CompositionGridId,
        anchor: GridCellAddress,
        clipboard: ClipboardTable,
        transposed: bool,
    },
    AppendGridRows {
        grid: CompositionGridId,
        clipboard: ClipboardTable,
    },
    PasteGridBlock {
        grid: CompositionGridId,
        clipboard: ClipboardTable,
    },
    InsertGridRow {
        grid: CompositionGridId,
        before: Option<GridRowId>,
    },
    DeleteGridRows {
        grid: CompositionGridId,
        rows: Vec<GridRowId>,
    },
    ClearGridCells {
        grid: CompositionGridId,
        cells: Vec<GridCellAddress>,
    },
    FillDownGrid {
        grid: CompositionGridId,
    },
    AddScalarField {
        grid: CompositionGridId,
        name: String,
    },
    RenameScalarField {
        grid: CompositionGridId,
        field: ScalarFieldId,
        name: String,
    },
    SetScalarUnits {
        grid: CompositionGridId,
        field: ScalarFieldId,
        units: Option<String>,
    },
    SetScalarPrecision {
        grid: CompositionGridId,
        field: ScalarFieldId,
        precision: usize,
    },
    ClearScalarField {
        grid: CompositionGridId,
        field: ScalarFieldId,
    },
    RemoveScalarField {
        grid: CompositionGridId,
        field: ScalarFieldId,
    },
    RemoveGrid(CompositionGridId),
    RemoveSection(SectionId),
    RemoveEmbeddedChart(EmbeddedChartId),
    ExportGridTsv {
        grid: CompositionGridId,
        path: std::path::PathBuf,
        options: crate::GridExportOptions,
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
                apply_editor_selection(document, state, selection)?;
                Ok(None)
            }
            Self::OpenFlatView(target) => {
                state.set_flat_view(target);
                Ok(None)
            }
            Self::CloseFlatView => {
                state.set_flat_view(FlatViewTarget::None);
                state.clear_cursor();
                Ok(None)
            }
            Self::OpenDataGrid(grid) => {
                let value = document.grid(grid).ok_or(GridError::InvalidComposition {
                    message: "grid does not exist".to_owned(),
                })?;
                state.open_data_grid(grid);
                let columns = grid_columns(value);
                if state
                    .table_state(grid)
                    .is_some_and(|table| table.active_cell.is_none())
                    && let (Some(row), Some(column)) = (value.row_ids().first(), columns.first())
                {
                    state.table_state_mut(grid).activate(
                        GridCellAddress {
                            row: *row,
                            column: column.id,
                        },
                        false,
                    );
                }
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
            Self::SetSectionName { section, name } => {
                document
                    .plot_mut()
                    .section_mut(section)
                    .ok_or(crate::SectionError::UnknownSection { id: section.get() })?
                    .set_name(name);
                Ok(None)
            }
            Self::SetEmbeddedChartName { chart, name } => {
                document
                    .plot_mut()
                    .embedded_chart_mut(chart)
                    .ok_or(crate::SectionError::UnknownSection { id: chart.get() })?
                    .set_name(name);
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
            Self::SetEmbeddedSurfaceVisible { target, visible } => {
                apply_embedded_style(document, target, |style| style.surface_visible = visible)?;
                Ok(None)
            }
            Self::SetEmbeddedGridVisible { target, visible } => {
                apply_embedded_style(document, target, |style| style.grid.visible = visible)?;
                Ok(None)
            }
            Self::SetEmbeddedBoundaryVisible { target, visible } => {
                apply_embedded_style(document, target, |style| style.boundary_visible = visible)?;
                Ok(None)
            }
            Self::SetFrameEdgeColor(color) => {
                let style = document.plot().frame();
                document
                    .plot_mut()
                    .configure_frame()
                    .edge_style(color)
                    .edge_width(style.edge_width())
                    .faces(style.faces())
                    .face_color(style.face_color())
                    .vertex_labels(style.vertex_labels())
                    .draw()?;
                Ok(None)
            }
            Self::SetFrameEdgeWidth(width) => {
                let style = document.plot().frame();
                document
                    .plot_mut()
                    .configure_frame()
                    .edge_style(style.edge_color())
                    .edge_width(width.max(0.1))
                    .faces(style.faces())
                    .face_color(style.face_color())
                    .vertex_labels(style.vertex_labels())
                    .draw()?;
                Ok(None)
            }
            Self::SetFrameFacesVisible(visible) => {
                let style = document.plot().frame();
                document
                    .plot_mut()
                    .configure_frame()
                    .edge_style(style.edge_color())
                    .edge_width(style.edge_width())
                    .faces(visible)
                    .face_color(style.face_color())
                    .vertex_labels(style.vertex_labels())
                    .draw()?;
                Ok(None)
            }
            Self::SetFrameFaceColor(color) => {
                let style = document.plot().frame();
                document
                    .plot_mut()
                    .configure_frame()
                    .edge_style(style.edge_color())
                    .edge_width(style.edge_width())
                    .faces(style.faces())
                    .face_color(color)
                    .vertex_labels(style.vertex_labels())
                    .draw()?;
                Ok(None)
            }
            Self::SetBackground(color) => {
                document.plot_mut().set_background(color);
                Ok(None)
            }
            Self::SetEmbeddedNormalMode { target, mode } => {
                apply_embedded_style(document, target, |style| style.normal_mode = mode)?;
                Ok(None)
            }
            Self::SetBreakLineStyle { chart, line, style } => {
                document
                    .plot_mut()
                    .embedded_chart_mut(chart)
                    .ok_or(crate::SectionError::UnknownSection { id: chart.get() })?
                    .set_break_line_style(line, style)?;
                Ok(None)
            }
            Self::SetBreakLineKind { chart, line, kind } => {
                document
                    .plot_mut()
                    .embedded_chart_mut(chart)
                    .ok_or(crate::SectionError::UnknownSection { id: chart.get() })?
                    .set_break_line_kind(line, kind)?;
                Ok(None)
            }
            Self::SetGridName { grid, name } => {
                grid_mut(document, grid)?.set_name(name);
                Ok(None)
            }
            Self::RedefineRegularGrid {
                grid,
                definition,
                policy,
            } => {
                let tolerance = document.plot().tolerance();
                grid_mut(document, grid)?.redefine_regular(definition, policy, tolerance)?;
                state.clear_removed(document);
                Ok(None)
            }
            Self::SetEntryMode { grid, mode } => {
                let tolerance = document.plot().tolerance();
                grid_mut(document, grid)?.set_entry_mode(mode, tolerance)?;
                Ok(None)
            }
            Self::SetDuplicatePolicy { grid, policy } => {
                let tolerance = document.plot().tolerance();
                grid_mut(document, grid)?.set_duplicate_policy(policy, tolerance);
                Ok(None)
            }
            Self::EditGridComponent {
                grid,
                row,
                component,
                text,
            } => {
                let tolerance = document.plot().tolerance();
                grid_mut(document, grid)?.set_component_text(row, component, text, tolerance)?;
                Ok(None)
            }
            Self::EditGridScalar {
                grid,
                row,
                field,
                value,
            } => {
                grid_mut(document, grid)?.set_scalar(row, field, value)?;
                Ok(None)
            }
            Self::EditGridScalarText {
                grid,
                row,
                field,
                text,
            } => {
                grid_mut(document, grid)?.set_scalar_text(row, field, text)?;
                Ok(None)
            }
            Self::PasteScalarColumn {
                grid,
                field,
                start,
                clipboard,
            } => {
                let summary =
                    grid_mut(document, grid)?.paste_scalar_column(field, start, &clipboard)?;
                Ok(Some(summary))
            }
            Self::PasteGridCells {
                grid,
                anchor,
                clipboard,
                transposed,
            } => {
                let tolerance = document.plot().tolerance();
                let mut table = state
                    .data_tables
                    .remove(&grid)
                    .unwrap_or_else(|| DataTableState::new(grid));
                table.activate(anchor, false);
                let summary =
                    table.paste(grid_mut(document, grid)?, &clipboard, tolerance, transposed)?;
                state.data_tables.insert(grid, table);
                Ok(Some(summary))
            }
            Self::AppendGridRows { grid, clipboard } | Self::PasteGridBlock { grid, clipboard } => {
                let tolerance = document.plot().tolerance();
                let summary = grid_mut(document, grid)?.append_clipboard(&clipboard, tolerance)?;
                Ok(Some(summary))
            }
            Self::InsertGridRow { grid, before } => {
                let tolerance = document.plot().tolerance();
                let value = grid_mut(document, grid)?;
                let index = before
                    .and_then(|row| {
                        value
                            .row_ids()
                            .iter()
                            .position(|candidate| *candidate == row)
                    })
                    .unwrap_or_else(|| value.rows_len());
                let row = value.insert_empty_row(index, tolerance)?;
                apply_editor_selection(document, state, Selection::GridRow { grid, row })?;
                Ok(None)
            }
            Self::DeleteGridRows { grid, rows } => {
                grid_mut(document, grid)?.delete_rows(&rows)?;
                state.clear_removed(document);
                Ok(None)
            }
            Self::ClearGridCells { grid, cells } => {
                let tolerance = document.plot().tolerance();
                let value = grid_mut(document, grid)?;
                let columns = grid_columns(value);
                for cell in cells {
                    if let Some(column) = columns.iter().find(|column| column.id == cell.column) {
                        set_grid_cell_text(value, cell.row, column, String::new(), tolerance)?;
                    }
                }
                Ok(None)
            }
            Self::FillDownGrid { grid } => {
                let tolerance = document.plot().tolerance();
                let mut table = state
                    .data_tables
                    .remove(&grid)
                    .unwrap_or_else(|| DataTableState::new(grid));
                let updated = table.fill_down(grid_mut(document, grid)?, tolerance)?;
                state.data_tables.insert(grid, table);
                state.status = Some(format!("Filled {updated} cells down"));
                Ok(None)
            }
            Self::AddScalarField { grid, name } => {
                grid_mut(document, grid)?.add_scalar_field(name);
                Ok(None)
            }
            Self::RenameScalarField { grid, field, name } => {
                grid_mut(document, grid)?.set_scalar_name(field, name)?;
                Ok(None)
            }
            Self::SetScalarUnits { grid, field, units } => {
                grid_mut(document, grid)?.set_scalar_units(field, units)?;
                Ok(None)
            }
            Self::SetScalarPrecision {
                grid,
                field,
                precision,
            } => {
                grid_mut(document, grid)?.set_scalar_precision(field, precision)?;
                Ok(None)
            }
            Self::ClearScalarField { grid, field } => {
                grid_mut(document, grid)?.clear_scalar_field(field)?;
                Ok(None)
            }
            Self::RemoveScalarField { grid, field } => {
                grid_mut(document, grid)?
                    .remove_scalar_field(field)
                    .ok_or(GridError::UnknownScalarField { id: field.get() })?;
                state.clear_removed(document);
                Ok(None)
            }
            Self::RemoveGrid(grid) => {
                document
                    .remove_grid(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?;
                state.clear_removed(document);
                Ok(None)
            }
            Self::RemoveSection(section) => {
                document
                    .remove_section(section)
                    .ok_or(crate::SectionError::UnknownSection { id: section.get() })?;
                state.clear_removed(document);
                Ok(None)
            }
            Self::RemoveEmbeddedChart(chart) => {
                document
                    .remove_embedded_chart(chart)
                    .ok_or(crate::SectionError::UnknownSection { id: chart.get() })?;
                state.clear_removed(document);
                Ok(None)
            }
            Self::ExportGridTsv {
                grid,
                path,
                options,
            } => {
                let file = std::fs::File::create(&path).map_err(|error| GridError::Io {
                    message: error.to_string(),
                })?;
                document
                    .grid(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .write_tsv(file, &options)?;
                state.status = Some(format!("Saved {}", path.display()));
                Ok(None)
            }
            Self::FitCamera => {
                let camera = crate::Camera::fit(document.plot().scene_bounds());
                document.plot_mut().set_camera(camera);
                Ok(None)
            }
            Self::ResetCamera => {
                let camera = crate::Camera::fit(document.plot().geometry().bounds());
                document.plot_mut().set_camera(camera);
                Ok(None)
            }
        }
    }
}

fn grid_mut(
    document: &mut TetraplotDocument,
    grid: CompositionGridId,
) -> Result<&mut CompositionGrid, GridError> {
    document
        .grid_mut(grid)
        .ok_or(GridError::InvalidComposition {
            message: "grid does not exist".to_owned(),
        })
}

fn apply_editor_selection(
    document: &TetraplotDocument,
    state: &mut EditorState,
    selection: Selection,
) -> Result<(), GridError> {
    match selection {
        Selection::Grid(grid)
        | Selection::ScalarField { grid, .. }
        | Selection::GridCell { grid, .. } => state.open_data_grid(grid),
        Selection::GridRow { grid, row } => {
            let value = document.grid(grid).ok_or(GridError::InvalidComposition {
                message: "grid does not exist".to_owned(),
            })?;
            if !value.row_ids().contains(&row) {
                return Err(GridError::UnknownGridRow { id: row.get() });
            }
            state.open_data_grid(grid);
            let rows = state
                .table_state(grid)
                .map_or_else(|| value.row_ids(), |table| table.displayed_rows(value));
            let columns = grid_columns(value);
            let table = state.table_state_mut(grid);
            table.select_row(row, &columns);
            table.scroll_to_row(&rows, row);
            if let Ok(points) = document.prepared_grid_points(grid)
                && let Some(point) = points.iter().find(|point| point.row_id == row)
            {
                state.status = Some(format!(
                    "Grid {} row {}: tetra {:?}, world {:?}",
                    grid.get(),
                    row.get(),
                    point.tetrahedral.as_array(),
                    point.world
                ));
                if let (Some(local), owner) = (
                    point.local,
                    match value.coordinate_space() {
                        crate::GridCoordinateSpace::Section(id) => {
                            Some(LinkedCursorOwner::Section(id))
                        }
                        crate::GridCoordinateSpace::EmbeddedChart(id) => {
                            Some(LinkedCursorOwner::EmbeddedChart(id))
                        }
                        crate::GridCoordinateSpace::Tetrahedral => None,
                    },
                ) && let Some(owner) = owner
                {
                    state.set_cursor(Some(LinkedCursor {
                        owner,
                        local_position: local,
                        tetrahedral_position: point.tetrahedral,
                        world_position: point.world.map(f64::from),
                        surface_triangle: None,
                        patch: None,
                    }));
                }
            }
        }
        _ => {}
    }
    state.select(selection);
    Ok(())
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

fn apply_embedded_style(
    document: &mut TetraplotDocument,
    target: Selection,
    modify: impl FnOnce(&mut crate::EmbeddedChartStyle),
) -> Result<(), crate::TetraplotError> {
    match target {
        Selection::Section(id) => {
            let section = document
                .plot_mut()
                .section_mut(id)
                .ok_or(crate::SectionError::UnknownSection { id: id.get() })?;
            let mut style = section.style();
            modify(&mut style);
            section.set_style(style);
        }
        Selection::EmbeddedChart(id) => {
            let chart = document
                .plot_mut()
                .embedded_chart_mut(id)
                .ok_or(crate::SectionError::UnknownSection { id: id.get() })?;
            let mut style = chart.style();
            modify(&mut style);
            chart.set_style(style);
        }
        _ => {}
    }
    Ok(())
}
