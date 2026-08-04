//! Editor-neutral document, selection, linked cursor, commands, and scientific picking.

use crate::{BreakLineId, EmbeddedChartId, SeriesId};
use crate::{
    ChartEmbedding, CompositionGrid, CompositionGridId, GridColumnId, GridCoordinate, GridError,
    GridRowId, PasteSummary, PreparedEmbeddedChart, ScalarFieldId, SectionId, SectionSeriesId,
    SurfacePatchId, TernaryPoint, TetraGeometry, TetraPoint, Tetraplot, Tolerance,
};
use std::collections::BTreeMap;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PhysicalViewport {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EditorViewport {
    /// Logical points in top-left-origin order: [min_x, min_y, max_x, max_y].
    pub logical_rect: [f32; 4],
    /// Physical pixels in bottom-left-origin renderer coordinates.
    pub physical_rect: PhysicalViewport,
    pub device_pixel_ratio: f32,
}

impl EditorViewport {
    pub fn from_logical_rect(
        logical_rect: [f32; 4],
        window_size: [u32; 2],
        device_pixel_ratio: f32,
    ) -> Option<Self> {
        if !device_pixel_ratio.is_finite() || device_pixel_ratio <= 0.0 {
            return None;
        }
        let logical_width = window_size[0] as f32 / device_pixel_ratio;
        let logical_height = window_size[1] as f32 / device_pixel_ratio;
        let min_x = logical_rect[0].clamp(0.0, logical_width);
        let min_y = logical_rect[1].clamp(0.0, logical_height);
        let max_x = logical_rect[2].clamp(min_x, logical_width);
        let max_y = logical_rect[3].clamp(min_y, logical_height);
        let x = (min_x * device_pixel_ratio).round() as i32;
        let top = (min_y * device_pixel_ratio).round() as i32;
        let right = (max_x * device_pixel_ratio).round() as i32;
        let bottom_from_top = (max_y * device_pixel_ratio).round() as i32;
        let y = window_size[1] as i32 - bottom_from_top;
        let width = (right - x).max(0) as u32;
        let height = (bottom_from_top - top).max(0) as u32;
        (width >= 2 && height >= 2).then_some(Self {
            logical_rect: [min_x, min_y, max_x, max_y],
            physical_rect: PhysicalViewport {
                x,
                y,
                width,
                height,
            },
            device_pixel_ratio,
        })
    }

    pub fn contains_logical(&self, point: [f32; 2]) -> bool {
        point[0] >= self.logical_rect[0]
            && point[0] <= self.logical_rect[2]
            && point[1] >= self.logical_rect[1]
            && point[1] <= self.logical_rect[3]
    }

    pub fn contains_physical(&self, point: [f32; 2]) -> bool {
        point[0] >= self.physical_rect.x as f32
            && point[0] <= (self.physical_rect.x + self.physical_rect.width as i32) as f32
            && point[1] >= self.physical_rect.y as f32
            && point[1] <= (self.physical_rect.y + self.physical_rect.height as i32) as f32
    }
}
#[derive(Clone, Debug, Default)]
pub struct EditorState {
    pub selection: Selection,
    pub flat_view: FlatViewTarget,
    pub linked_cursor: Option<LinkedCursor>,
    pub active_grid: Option<CompositionGridId>,
    pub status: Option<String>,
    pub tables: BTreeMap<CompositionGridId, crate::DataTableState>,
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
    pub fn ensure_table(
        &mut self,
        grid_id: CompositionGridId,
        grid: &CompositionGrid,
    ) -> &mut crate::DataTableState {
        let rows = grid.row_ids();
        let columns = grid.columns();
        let table = self
            .tables
            .entry(grid_id)
            .or_insert_with(|| crate::DataTableState::new(grid_id, rows.clone(), columns.clone()));
        table.refresh(rows, columns);
        table
    }
    pub fn select_grid_row(
        &mut self,
        document: &TetraplotDocument,
        grid_id: CompositionGridId,
        row: GridRowId,
    ) -> Result<(), GridError> {
        let grid = document
            .grid(grid_id)
            .ok_or(GridError::InvalidComposition {
                message: "grid does not exist".to_owned(),
            })?;
        if !grid.row_ids().contains(&row) {
            return Err(GridError::UnknownGridRow { id: row.get() });
        }
        let table = self.ensure_table(grid_id, grid);
        table.scroll_to_row(row);
        if let Some(column) = table.columns.first().map(|column| column.id) {
            table.activate(crate::GridCellAddress { row, column }, false);
        }
        self.active_grid = Some(grid_id);
        self.select(Selection::GridRow { grid: grid_id, row });
        Ok(())
    }
    pub fn set_linked_cursor(&mut self, cursor: Option<LinkedCursor>) {
        if self.linked_cursor != cursor {
            self.linked_cursor = cursor;
            self.revision += 1;
        }
    }
    pub fn clear_cursor(&mut self) {
        if self.linked_cursor.take().is_some() {
            self.revision += 1;
        }
    }
    pub fn clear_removed(&mut self, document: &TetraplotDocument) {
        self.tables.retain(|grid, _| document.grid(*grid).is_some());
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
            Selection::Grid(id) | Selection::ScalarField { grid: id, .. } => {
                document.grid(id).is_some()
            }
            Selection::GridRow { grid, row } | Selection::GridCell { grid, row, .. } => document
                .grid(grid)
                .is_some_and(|candidate| candidate.row_ids().contains(&row)),
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

pub fn pick_grid_points(
    ray: Ray,
    points: &[crate::PreparedGridPoint],
    threshold: f64,
) -> Option<EditorPickResult> {
    points
        .iter()
        .filter_map(|point| {
            let world = point.world.map(f64::from);
            let (distance, along_ray) = distance_to_ray(ray, world)?;
            (distance <= threshold).then_some((along_ray, point, world))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, point, world)| EditorPickResult {
            selection: Selection::GridRow {
                grid: point.grid_id,
                row: point.row_id,
            },
            local_position: point.local,
            tetrahedral_position: Some(point.tetrahedral),
            world_position: world,
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

pub fn pick_embedded_chart(
    ray: Ray,
    chart: EmbeddedChartId,
    _embedding: &ChartEmbedding,
    prepared: &PreparedEmbeddedChart,
    geometry: &TetraGeometry,
    tolerance: Tolerance,
) -> Option<EditorPickResult> {
    let point_threshold = tolerance.absolute.max(0.025);
    let point_hit = prepared
        .points
        .iter()
        .flat_map(|series| {
            series.points.iter().filter_map(move |point| {
                let world = point.world.map(f64::from);
                let (distance, along_ray) = distance_to_ray(ray, world)?;
                (distance <= point_threshold).then_some((along_ray, series, point, world))
            })
        })
        .min_by(|left, right| left.0.total_cmp(&right.0));
    if let Some((_, series, point, world)) = point_hit {
        return Some(EditorPickResult {
            selection: Selection::EmbeddedSeries {
                chart,
                series: series.series_id,
            },
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
        });
    }

    let line_threshold = tolerance.absolute.max(0.018);
    let break_hit = prepared
        .break_lines
        .iter()
        .flat_map(|line| {
            line.line
                .segments
                .iter()
                .enumerate()
                .filter_map(move |(index, segment)| {
                    let segment_world = segment.world.map(|point| point.map(f64::from));
                    let (distance, along_ray, along_segment) =
                        distance_between_ray_and_segment(ray, segment_world)?;
                    (distance <= line_threshold).then_some((
                        along_ray,
                        along_segment,
                        line,
                        segment,
                        index,
                    ))
                })
        })
        .min_by(|left, right| left.0.total_cmp(&right.0));
    if let Some((_, along_segment, line, segment, index)) = break_hit {
        return Some(EditorPickResult {
            selection: Selection::BreakLine {
                chart,
                line: line.id,
            },
            local_position: Some(interpolate_ternary(segment.local, along_segment)),
            tetrahedral_position: Some(interpolate_tetrahedral(segment.tetrahedral, along_segment)),
            world_position: interpolate3(
                segment.world.map(|point| point.map(f64::from)),
                along_segment,
            ),
            surface_triangle: Some(segment.surface_triangle),
            patch: Some(segment.patch),
            series_id: None,
            primitive_index: Some(index),
            grid: None,
            grid_row: None,
            scalar_value: None,
            break_line: Some(line.id),
        });
    }

    let line_hit = prepared
        .lines
        .iter()
        .filter_map(|line| line.series_id.map(|series| (series, line)))
        .flat_map(|(series, line)| {
            line.segments
                .iter()
                .enumerate()
                .filter_map(move |(index, segment)| {
                    let segment_world = segment.world.map(|point| point.map(f64::from));
                    let (distance, along_ray, along_segment) =
                        distance_between_ray_and_segment(ray, segment_world)?;
                    (distance <= line_threshold).then_some((
                        along_ray,
                        along_segment,
                        series,
                        segment,
                        index,
                    ))
                })
        })
        .min_by(|left, right| left.0.total_cmp(&right.0));
    if let Some((_, along_segment, series, segment, index)) = line_hit {
        return Some(EditorPickResult {
            selection: Selection::EmbeddedSeries { chart, series },
            local_position: Some(interpolate_ternary(segment.local, along_segment)),
            tetrahedral_position: Some(interpolate_tetrahedral(segment.tetrahedral, along_segment)),
            world_position: interpolate3(
                segment.world.map(|point| point.map(f64::from)),
                along_segment,
            ),
            surface_triangle: Some(segment.surface_triangle),
            patch: Some(segment.patch),
            series_id: Some(series),
            primitive_index: Some(index),
            grid: None,
            grid_row: None,
            scalar_value: None,
            break_line: None,
        });
    }

    let hit = pick_prepared_surface(ray, prepared, geometry, tolerance)?;
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
fn distance_to_ray(ray: Ray, point: [f64; 3]) -> Option<(f64, f64)> {
    let along_ray = dot(sub(point, ray.origin), ray.direction);
    if along_ray < 0.0 {
        return None;
    }
    let nearest = add(ray.origin, scale(ray.direction, along_ray));
    Some((length(sub(point, nearest)), along_ray))
}

fn distance_between_ray_and_segment(ray: Ray, segment: [[f64; 3]; 2]) -> Option<(f64, f64, f64)> {
    let segment_direction = sub(segment[1], segment[0]);
    let segment_length_squared = dot(segment_direction, segment_direction);
    if segment_length_squared <= f64::EPSILON {
        let (distance, along_ray) = distance_to_ray(ray, segment[0])?;
        return Some((distance, along_ray, 0.0));
    }
    let origin_delta = sub(ray.origin, segment[0]);
    let ray_segment = dot(ray.direction, segment_direction);
    let ray_origin = dot(ray.direction, origin_delta);
    let segment_origin = dot(segment_direction, origin_delta);
    let denominator = segment_length_squared - ray_segment * ray_segment;
    let mut along_segment = if denominator.abs() <= 1.0e-12 {
        0.0
    } else {
        (segment_origin - ray_segment * ray_origin) / denominator
    }
    .clamp(0.0, 1.0);
    let mut along_ray = dot(
        sub(
            add(segment[0], scale(segment_direction, along_segment)),
            ray.origin,
        ),
        ray.direction,
    )
    .max(0.0);
    along_segment = (dot(
        sub(add(ray.origin, scale(ray.direction, along_ray)), segment[0]),
        segment_direction,
    ) / segment_length_squared)
        .clamp(0.0, 1.0);
    along_ray = dot(
        sub(
            add(segment[0], scale(segment_direction, along_segment)),
            ray.origin,
        ),
        ray.direction,
    )
    .max(0.0);
    let ray_point = add(ray.origin, scale(ray.direction, along_ray));
    let segment_point = add(segment[0], scale(segment_direction, along_segment));
    Some((
        length(sub(ray_point, segment_point)),
        along_ray,
        along_segment,
    ))
}

fn interpolate3(values: [[f64; 3]; 2], amount: f64) -> [f64; 3] {
    std::array::from_fn(|axis| values[0][axis] + (values[1][axis] - values[0][axis]) * amount)
}

fn interpolate_ternary(values: [TernaryPoint; 2], amount: f64) -> TernaryPoint {
    let values = values.map(TernaryPoint::as_array);
    TernaryPoint::from_unchecked(std::array::from_fn(|axis| {
        values[0][axis] + (values[1][axis] - values[0][axis]) * amount
    }))
}

fn interpolate_tetrahedral(values: [TetraPoint; 2], amount: f64) -> TetraPoint {
    let values = values.map(TetraPoint::as_array);
    TetraPoint::from_unchecked(std::array::from_fn(|axis| {
        values[0][axis] + (values[1][axis] - values[0][axis]) * amount
    }))
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
    EditGridComponent {
        grid: CompositionGridId,
        row: GridRowId,
        component: usize,
        text: String,
    },
    EditGridCellText {
        grid: CompositionGridId,
        address: crate::GridCellAddress,
        text: String,
    },
    PasteGridCells {
        grid: CompositionGridId,
        anchor: crate::GridCellAddress,
        clipboard: crate::ClipboardTable,
        transpose: bool,
    },
    AppendGridClipboard {
        grid: CompositionGridId,
        clipboard: crate::ClipboardTable,
    },
    SetEntryMode {
        grid: CompositionGridId,
        mode: crate::CompositionEntryMode,
    },
    SetDuplicatePolicy {
        grid: CompositionGridId,
        policy: crate::DuplicateCompositionPolicy,
    },
    InsertGridRow {
        grid: CompositionGridId,
        before: Option<GridRowId>,
    },
    AppendGridRow {
        grid: CompositionGridId,
    },
    DeleteGridRows {
        grid: CompositionGridId,
        rows: Vec<GridRowId>,
    },
    ClearGridCells {
        grid: CompositionGridId,
        cells: Vec<crate::GridCellAddress>,
    },
    RenameGrid {
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
    RemoveGrid {
        grid: CompositionGridId,
    },
    SetName {
        target: Selection,
        name: String,
    },
    SetChartStyle {
        target: Selection,
        style: crate::EmbeddedChartStyle,
    },
    SetFrameProperties {
        edge_color: crate::Color,
        edge_width: f32,
        faces: bool,
        face_color: crate::Color,
        background: crate::Color,
    },
    SetBreakLineProperties {
        chart: EmbeddedChartId,
        line: BreakLineId,
        kind: crate::BreakLineKind,
        style: crate::BreakLineStyle,
    },
    RemoveSection(SectionId),
    RemoveEmbeddedChart(EmbeddedChartId),
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
                if let Selection::GridRow { grid, row } = selection {
                    state.select_grid_row(document, grid, row)?;
                } else {
                    state.select(selection);
                }
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
                let source = document.grid(grid).ok_or(GridError::InvalidComposition {
                    message: "grid does not exist".to_owned(),
                })?;
                state.ensure_table(grid, source);
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
                refresh_table(document, state, grid);
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
                refresh_table(document, state, grid);
                Ok(None)
            }
            Self::EditGridComponent {
                grid,
                row,
                component,
                text,
            } => {
                let tolerance = document.plot().tolerance();
                document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .set_component_text(row, component, text, tolerance)?;
                refresh_table(document, state, grid);
                Ok(None)
            }
            Self::EditGridCellText {
                grid,
                address,
                text,
            } => {
                edit_grid_cell(document, state, grid, address, text)?;
                refresh_table(document, state, grid);
                Ok(None)
            }
            Self::PasteGridCells {
                grid,
                anchor,
                clipboard,
                transpose,
            } => {
                let summary =
                    paste_grid_cells(document, state, grid, anchor, clipboard, transpose)?;
                refresh_table(document, state, grid);
                Ok(Some(summary))
            }
            Self::AppendGridClipboard { grid, clipboard } => {
                if clipboard.rows.is_empty() {
                    return Ok(Some(PasteSummary {
                        inserted: 0,
                        updated: 0,
                        valid: 0,
                        incomplete: 0,
                        invalid: 0,
                        warnings: vec!["clipboard contains no data rows".to_owned()],
                    }));
                }
                let tolerance = document.plot().tolerance();
                let row = document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .append_empty_row(tolerance)?;
                let column = document
                    .grid(grid)
                    .and_then(|source| {
                        source
                            .columns()
                            .into_iter()
                            .find(|column| column.editable)
                            .map(|column| column.id)
                    })
                    .ok_or(GridError::InvalidComposition {
                        message: "grid has no editable columns".to_owned(),
                    })?;
                let mut summary = paste_grid_cells(
                    document,
                    state,
                    grid,
                    crate::GridCellAddress { row, column },
                    clipboard,
                    false,
                )?;
                summary.inserted += 1;
                refresh_table(document, state, grid);
                Ok(Some(summary))
            }
            Self::SetEntryMode { grid, mode } => {
                document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .set_entry_mode(mode)?;
                refresh_table(document, state, grid);
                Ok(None)
            }
            Self::SetDuplicatePolicy { grid, policy } => {
                document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .set_duplicate_policy(policy);
                refresh_table(document, state, grid);
                Ok(None)
            }
            Self::InsertGridRow { grid, before } => {
                let tolerance = document.plot().tolerance();
                let index = match before {
                    Some(row) => document
                        .grid(grid)
                        .ok_or(GridError::InvalidComposition {
                            message: "grid does not exist".to_owned(),
                        })?
                        .row_ids()
                        .iter()
                        .position(|candidate| *candidate == row)
                        .ok_or(GridError::UnknownGridRow { id: row.get() })?,
                    None => 0,
                };
                let row = document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .insert_empty_row(index, tolerance)?;
                refresh_table(document, state, grid);
                state.select_grid_row(document, grid, row)?;
                Ok(None)
            }
            Self::AppendGridRow { grid } => {
                let tolerance = document.plot().tolerance();
                let row = document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .append_empty_row(tolerance)?;
                refresh_table(document, state, grid);
                state.select_grid_row(document, grid, row)?;
                Ok(None)
            }
            Self::DeleteGridRows { grid, rows } => {
                document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .delete_rows(&rows)?;
                refresh_table(document, state, grid);
                state.select(Selection::Grid(grid));
                Ok(None)
            }
            Self::ClearGridCells { grid, cells } => {
                for address in cells {
                    edit_grid_cell(document, state, grid, address, String::new())?;
                }
                refresh_table(document, state, grid);
                Ok(None)
            }
            Self::RenameGrid { grid, name } => {
                document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .set_name(name);
                Ok(None)
            }
            Self::RenameScalarField { grid, field, name } => {
                document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .rename_scalar_field(field, name)?;
                refresh_table(document, state, grid);
                Ok(None)
            }
            Self::SetScalarUnits { grid, field, units } => {
                document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .set_scalar_units(field, units)?;
                refresh_table(document, state, grid);
                Ok(None)
            }
            Self::SetScalarPrecision {
                grid,
                field,
                precision,
            } => {
                document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .set_scalar_precision(field, precision)?;
                Ok(None)
            }
            Self::ClearScalarField { grid, field } => {
                document
                    .grid_mut(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?
                    .clear_scalar_field(field)?;
                Ok(None)
            }
            Self::RemoveGrid { grid } => {
                document
                    .remove_grid(grid)
                    .ok_or(GridError::InvalidComposition {
                        message: "grid does not exist".to_owned(),
                    })?;
                state.tables.remove(&grid);
                if state.active_grid == Some(grid) {
                    state.active_grid = None;
                }
                state.select(Selection::None);
                Ok(None)
            }
            Self::SetName { target, name } => {
                match target {
                    Selection::Section(id) => document
                        .plot_mut()
                        .section_mut(id)
                        .ok_or(crate::SectionError::UnknownSection { id: id.get() })?
                        .set_name(Some(name)),
                    Selection::EmbeddedChart(id) => document
                        .plot_mut()
                        .embedded_chart_mut(id)
                        .ok_or(crate::SectionError::UnknownSection { id: id.get() })?
                        .set_name(Some(name)),
                    Selection::Grid(id) => document
                        .grid_mut(id)
                        .ok_or(GridError::InvalidComposition {
                            message: "grid does not exist".to_owned(),
                        })?
                        .set_name(name),
                    _ => {}
                }
                Ok(None)
            }
            Self::SetChartStyle { target, style } => {
                match target {
                    Selection::Section(id) => document
                        .plot_mut()
                        .section_mut(id)
                        .ok_or(crate::SectionError::UnknownSection { id: id.get() })?
                        .set_style(style),
                    Selection::EmbeddedChart(id) => document
                        .plot_mut()
                        .embedded_chart_mut(id)
                        .ok_or(crate::SectionError::UnknownSection { id: id.get() })?
                        .set_style(style),
                    _ => {}
                }
                Ok(None)
            }
            Self::SetFrameProperties {
                edge_color,
                edge_width,
                faces,
                face_color,
                background,
            } => {
                let plot = document.plot_mut();
                plot.set_background(background);
                plot.configure_frame()
                    .edge_style(edge_color)
                    .edge_width(edge_width.max(0.1))
                    .faces(faces)
                    .face_color(face_color)
                    .draw()?;
                Ok(None)
            }
            Self::SetBreakLineProperties {
                chart,
                line,
                kind,
                style,
            } => {
                let mut embedding = document
                    .plot()
                    .embedded_chart(chart)
                    .ok_or(crate::SectionError::UnknownSection { id: chart.get() })?
                    .embedding()
                    .clone();
                let updated = match &mut embedding {
                    ChartEmbedding::Triangulated(embedding) => {
                        embedding.update_break_line(line, kind, style)
                    }
                    ChartEmbedding::Planar(_) => false,
                };
                if !updated {
                    return Err(GridError::InvalidComposition {
                        message: format!("break line {} does not exist", line.get()),
                    }
                    .into());
                }
                document
                    .plot_mut()
                    .embedded_chart_mut(chart)
                    .ok_or(crate::SectionError::UnknownSection { id: chart.get() })?
                    .replace_embedding(embedding);
                Ok(None)
            }
            Self::RemoveSection(section) => {
                document
                    .remove_section(section)
                    .ok_or(crate::SectionError::UnknownSection { id: section.get() })?;
                state.select(Selection::None);
                Ok(None)
            }
            Self::RemoveEmbeddedChart(chart) => {
                document
                    .remove_embedded_chart(chart)
                    .ok_or(crate::SectionError::UnknownSection { id: chart.get() })?;
                state.select(Selection::None);
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
fn refresh_table(document: &TetraplotDocument, state: &mut EditorState, grid: CompositionGridId) {
    if let Some(source) = document.grid(grid) {
        state.ensure_table(grid, source);
    }
}

fn edit_grid_cell(
    document: &mut TetraplotDocument,
    state: &mut EditorState,
    grid: CompositionGridId,
    address: crate::GridCellAddress,
    text: String,
) -> Result<bool, crate::TetraplotError> {
    let column = document
        .grid(grid)
        .ok_or(GridError::InvalidComposition {
            message: "grid does not exist".to_owned(),
        })?
        .columns()
        .into_iter()
        .find(|column| column.id == address.column)
        .ok_or(GridError::UnknownColumn {
            column: address.column.get() as usize,
        })?;
    match column.kind {
        crate::GridColumnKind::Component {
            component,
            dependent,
        } => {
            if dependent {
                return Err(GridError::ReadOnlyColumn {
                    column: component.index(),
                }
                .into());
            }
            let tolerance = document.plot().tolerance();
            document
                .grid_mut(grid)
                .ok_or(GridError::InvalidComposition {
                    message: "grid does not exist".to_owned(),
                })?
                .set_component_text(address.row, component.index(), text, tolerance)?;
            Ok(document
                .grid(grid)
                .and_then(|source| source.row_coordinate(address.row).ok().flatten())
                .is_some())
        }
        crate::GridColumnKind::LocalComponent {
            component,
            dependent,
        } => {
            if dependent {
                return Err(GridError::ReadOnlyColumn { column: component }.into());
            }
            let tolerance = document.plot().tolerance();
            document
                .grid_mut(grid)
                .ok_or(GridError::InvalidComposition {
                    message: "grid does not exist".to_owned(),
                })?
                .set_component_text(address.row, component, text, tolerance)?;
            Ok(document
                .grid(grid)
                .and_then(|source| source.row_coordinate(address.row).ok().flatten())
                .is_some())
        }
        crate::GridColumnKind::Scalar { field } => {
            match crate::ParsedCellValue::parse(text.clone()) {
                crate::ParsedCellValue::Empty => {
                    document
                        .grid_mut(grid)
                        .ok_or(GridError::InvalidComposition {
                            message: "grid does not exist".to_owned(),
                        })?
                        .set_scalar(address.row, field, None)?;
                    if let Some(source) = document.grid(grid) {
                        state.ensure_table(grid, source).clear_invalid(address);
                    }
                    Ok(true)
                }
                crate::ParsedCellValue::ValidNumber(value) => {
                    document
                        .grid_mut(grid)
                        .ok_or(GridError::InvalidComposition {
                            message: "grid does not exist".to_owned(),
                        })?
                        .set_scalar(address.row, field, Some(value))?;
                    if let Some(source) = document.grid(grid) {
                        state.ensure_table(grid, source).clear_invalid(address);
                    }
                    Ok(true)
                }
                crate::ParsedCellValue::InvalidText(raw) => {
                    if let Some(source) = document.grid(grid) {
                        state
                            .ensure_table(grid, source)
                            .retain_invalid(address, raw);
                    }
                    state.status = Some(format!(
                        "Invalid scalar text retained at row {}, column {}.",
                        address.row.get(),
                        column.label
                    ));
                    Ok(false)
                }
            }
        }
        crate::GridColumnKind::RowId | crate::GridColumnKind::Validation => {
            Err(GridError::ReadOnlyColumn {
                column: address.column.get() as usize,
            }
            .into())
        }
    }
}

fn paste_grid_cells(
    document: &mut TetraplotDocument,
    state: &mut EditorState,
    grid: CompositionGridId,
    anchor: crate::GridCellAddress,
    clipboard: crate::ClipboardTable,
    transpose: bool,
) -> Result<PasteSummary, crate::TetraplotError> {
    let clipboard = if transpose {
        clipboard.transpose()
    } else {
        clipboard
    };
    let source = document.grid(grid).ok_or(GridError::InvalidComposition {
        message: "grid does not exist".to_owned(),
    })?;
    let columns = source.columns();
    let anchor_column = columns
        .iter()
        .position(|column| column.id == anchor.column)
        .ok_or(GridError::UnknownColumn {
            column: anchor.column.get() as usize,
        })?;
    let mut warnings = clipboard.shape_warnings();
    let mapping: Vec<Option<GridColumnId>> = if let Some(headers) = &clipboard.headers {
        headers
            .iter()
            .map(|header| {
                let normalized = header.trim();
                if normalized.eq_ignore_ascii_case("row")
                    || normalized.eq_ignore_ascii_case("row id")
                {
                    return None;
                }
                let matches: Vec<_> = columns
                    .iter()
                    .filter(|column| {
                        column.label.eq_ignore_ascii_case(normalized)
                            || match column.kind {
                                crate::GridColumnKind::Scalar { field } => source
                                    .fields()
                                    .iter()
                                    .find(|candidate| candidate.id() == field)
                                    .is_some_and(|candidate| {
                                        candidate.name().eq_ignore_ascii_case(normalized)
                                    }),
                                _ => false,
                            }
                    })
                    .collect();
                match matches.as_slice() {
                    [column] => Some(column.id),
                    [] => {
                        warnings.push(format!("unrecognized header '{header}'"));
                        None
                    }
                    _ => {
                        warnings.push(format!("ambiguous header '{header}'"));
                        None
                    }
                }
            })
            .collect()
    } else {
        (0..clipboard.rows.iter().map(Vec::len).max().unwrap_or(0))
            .map(|offset| columns.get(anchor_column + offset).map(|column| column.id))
            .collect()
    };
    let row_ids = source.row_ids();
    let start_row =
        row_ids
            .iter()
            .position(|row| *row == anchor.row)
            .ok_or(GridError::UnknownGridRow {
                id: anchor.row.get(),
            })?;
    let regular = source.is_regular();
    let tolerance = document.plot().tolerance();
    let mut summary = PasteSummary {
        inserted: 0,
        updated: 0,
        valid: 0,
        invalid: 0,
        incomplete: 0,
        warnings,
    };
    let mut scalar_invalid = 0;
    for (row_offset, values) in clipboard.rows.iter().enumerate() {
        let row_index = start_row + row_offset;
        let row = if let Some(row) = document
            .grid(grid)
            .and_then(|source| source.row_ids().get(row_index).copied())
        {
            row
        } else if regular {
            summary.warnings.push(format!(
                "paste row {} extends beyond the regular grid",
                row_offset + 1
            ));
            break;
        } else {
            let row = document
                .grid_mut(grid)
                .ok_or(GridError::InvalidComposition {
                    message: "grid does not exist".to_owned(),
                })?
                .append_empty_row(tolerance)?;
            summary.inserted += 1;
            row
        };
        for (column_index, value) in values.iter().enumerate() {
            let Some(column) = mapping.get(column_index).and_then(|column| *column) else {
                continue;
            };
            let address = crate::GridCellAddress { row, column };
            match edit_grid_cell(document, state, grid, address, value.clone()) {
                Ok(true) => {
                    summary.updated += 1;
                    summary.valid += 1;
                }
                Ok(false) => {
                    scalar_invalid += 1;
                }
                Err(error) => {
                    summary.invalid += 1;
                    summary.warnings.push(error.to_string());
                }
            }
        }
    }
    let validation = document
        .grid(grid)
        .map(CompositionGrid::validation_summary)
        .unwrap_or_default();
    if !regular {
        summary.valid = validation.valid;
        summary.incomplete = validation.incomplete;
        summary.invalid += validation.invalid;
    }
    summary.invalid += scalar_invalid;
    state.status = Some(format!(
        "Paste: {} inserted, {} updated, {} valid, {} incomplete, {} invalid{}",
        summary.inserted,
        summary.updated,
        summary.valid,
        summary.incomplete,
        summary.invalid,
        if summary.warnings.is_empty() {
            String::new()
        } else {
            format!(", {} warning(s)", summary.warnings.len())
        }
    ));
    Ok(summary)
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
