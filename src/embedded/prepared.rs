use super::{
    BreakLineId, ChartEmbedding, DiagramSeries, EmbeddedChartStyle, LocatedSurfacePoint,
    SectionSeriesId, SurfaceNormalMode, SurfacePatchId, TernaryDiagram, TernaryPoint,
    TriangulatedEmbedding,
};
use crate::{Color, CoordinateError, SectionError, TetraGeometry, TetraPoint, Tolerance};
use std::collections::{BTreeMap, BTreeSet};

/// A renderer-neutral vertex of a prepared embedded surface.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedEmbeddedVertex {
    /// Original local chart coordinate.
    pub local: TernaryPoint,
    /// Original parent tetrahedral coordinate.
    pub tetrahedral: TetraPoint,
    /// Cartesian world coordinate.
    pub world: [f32; 3],
    /// Patch-aware world-space normal.
    pub normal: [f32; 3],
    /// Scientific patch used to prepare the vertex.
    pub patch: SurfacePatchId,
    /// Source scientific mesh vertex where applicable.
    pub source_vertex: Option<u32>,
}

/// One indexed prepared supporting-surface triangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedEmbeddedTriangle {
    /// Prepared vertex indexes.
    pub indices: [u32; 3],
    /// Source embedding triangle index.
    pub source_triangle: u32,
    /// Scientific patch identity.
    pub patch: SurfacePatchId,
}

/// Indexed supporting geometry with renderer-only vertex duplication at patch boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedEmbeddedSurface {
    /// Prepared render vertices.
    pub vertices: Vec<PreparedEmbeddedVertex>,
    /// Prepared indexed triangles.
    pub triangles: Vec<PreparedEmbeddedTriangle>,
    /// Normal preparation mode.
    pub normal_mode: SurfaceNormalMode,
}

/// One mapped local chart point with recoverable scientific coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedEmbeddedPoint {
    /// Source point index in the local series.
    pub source_index: usize,
    /// Located chart and parent scientific data.
    pub location: LocatedSurfacePoint,
    /// Cartesian world position.
    pub world: [f32; 3],
}

/// One prepared local point series.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedEmbeddedPointSeries {
    /// Stable diagram series ID.
    pub series_id: SectionSeriesId,
    /// Prepared points.
    pub points: Vec<PreparedEmbeddedPoint>,
    /// Renderer colour.
    pub color: Color,
    /// Renderer glyph diameter.
    pub size: f32,
}

/// A local line fragment split at parameter-triangle and break boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedEmbeddedLineSegment {
    /// Local endpoints after clipping and splitting.
    pub local: [TernaryPoint; 2],
    /// Parent tetrahedral endpoints.
    pub tetrahedral: [TetraPoint; 2],
    /// Cartesian world endpoints.
    pub world: [[f32; 3]; 2],
    /// Embedding triangle containing the segment interior.
    pub surface_triangle: u32,
    /// Scientific patch containing the segment interior.
    pub patch: SurfacePatchId,
    /// Break crossed at the first endpoint, if any.
    pub start_break: Option<BreakLineId>,
    /// Break crossed at the second endpoint, if any.
    pub end_break: Option<BreakLineId>,
}

/// A renderer-neutral mapped curve.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedEmbeddedLine {
    /// `None` for generated boundary or grid geometry.
    pub series_id: Option<SectionSeriesId>,
    /// Patch-associated line fragments.
    pub segments: Vec<PreparedEmbeddedLineSegment>,
    /// Renderer colour.
    pub color: Color,
    /// Renderer width.
    pub width: f32,
}

/// A prepared explicit scientific break line.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedBreakLine {
    /// Stable scientific break identity.
    pub id: BreakLineId,
    /// Mapped curve segments.
    pub line: PreparedEmbeddedLine,
}

/// Prepared ternary grid geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedEmbeddedGrid {
    /// Grid curves prepared through the ordinary mapped-line pipeline.
    pub lines: Vec<PreparedEmbeddedLine>,
}

/// Complete renderer-neutral result for one embedded chart.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedEmbeddedChart {
    /// Supporting piecewise-planar surface.
    pub surface: PreparedEmbeddedSurface,
    /// Mapped local point series.
    pub points: Vec<PreparedEmbeddedPointSeries>,
    /// Mapped local line series.
    pub lines: Vec<PreparedEmbeddedLine>,
    /// Chart outer boundary.
    pub boundary: Vec<PreparedEmbeddedLine>,
    /// Optional mapped ternary grid.
    pub grid: Option<PreparedEmbeddedGrid>,
    /// Explicit scientific break overlays.
    pub break_lines: Vec<PreparedBreakLine>,
    /// Rendering style captured at preparation time.
    pub style: EmbeddedChartStyle,
}

/// Prepare a local ternary diagram for an affine or triangulated embedding.
pub fn prepare_embedded_chart(
    embedding: &ChartEmbedding,
    diagram: &TernaryDiagram,
    style: EmbeddedChartStyle,
    geometry: &TetraGeometry,
    tolerance: Tolerance,
) -> Result<PreparedEmbeddedChart, SectionError> {
    embedding.validate_for_preparation(tolerance)?;
    let surface = prepare_surface(embedding, style.normal_mode, geometry)?;
    let mut points = Vec::new();
    let mut lines = Vec::new();
    for series in diagram.series() {
        match series {
            DiagramSeries::Points { id, points: source } => {
                let mut prepared = Vec::with_capacity(source.len());
                for (source_index, local) in source.iter().copied().enumerate() {
                    let location = embedding.locate(local, tolerance)?;
                    prepared.push(PreparedEmbeddedPoint {
                        source_index,
                        world: geometry
                            .to_world(location.tetrahedral)
                            .map(|value| value as f32),
                        location,
                    });
                }
                points.push(PreparedEmbeddedPointSeries {
                    series_id: *id,
                    points: prepared,
                    color: style.point_color,
                    size: 8.0,
                });
            }
            DiagramSeries::Line {
                id,
                points: source,
                closed,
            } => lines.push(PreparedEmbeddedLine {
                series_id: Some(*id),
                segments: prepare_polyline(embedding, source, *closed, geometry, tolerance)?,
                color: style.line_color,
                width: 2.4,
            }),
        }
    }
    let boundary = prepare_boundary(embedding, geometry, tolerance, style)?;
    let grid = prepare_grid(embedding, geometry, tolerance, style)?;
    let break_lines = prepare_break_lines(embedding, geometry, tolerance)?;
    Ok(PreparedEmbeddedChart {
        surface,
        points,
        lines,
        boundary,
        grid,
        break_lines,
        style,
    })
}

fn prepare_surface(
    embedding: &ChartEmbedding,
    mode: SurfaceNormalMode,
    geometry: &TetraGeometry,
) -> Result<PreparedEmbeddedSurface, SectionError> {
    match embedding {
        ChartEmbedding::Planar(planar) => {
            let normal = planar.world_normal(geometry)?;
            let vertices = planar
                .vertices()
                .into_iter()
                .enumerate()
                .map(|(source_vertex, tetrahedral)| PreparedEmbeddedVertex {
                    local: TernaryPoint::from_unchecked(match source_vertex {
                        0 => [1.0, 0.0, 0.0],
                        1 => [0.0, 1.0, 0.0],
                        _ => [0.0, 0.0, 1.0],
                    }),
                    tetrahedral,
                    world: geometry.to_world(tetrahedral).map(|value| value as f32),
                    normal: normal.map(|value| value as f32),
                    patch: SurfacePatchId::new(0),
                    source_vertex: Some(source_vertex as u32),
                })
                .collect();
            Ok(PreparedEmbeddedSurface {
                vertices,
                triangles: vec![PreparedEmbeddedTriangle {
                    indices: [0, 1, 2],
                    source_triangle: 0,
                    patch: SurfacePatchId::new(0),
                }],
                normal_mode: mode,
            })
        }
        ChartEmbedding::Triangulated(value) => prepare_triangulated_surface(value, mode, geometry),
    }
}

fn prepare_triangulated_surface(
    embedding: &TriangulatedEmbedding,
    mode: SurfaceNormalMode,
    geometry: &TetraGeometry,
) -> Result<PreparedEmbeddedSurface, SectionError> {
    let mut face_normals = Vec::with_capacity(embedding.triangles().len());
    for (triangle_index, triangle) in embedding.triangles().iter().copied().enumerate() {
        let positions =
            triangle.map(|vertex| geometry.to_world(embedding.tetra_vertices()[vertex as usize]));
        face_normals.push(
            normal(
                sub(positions[1], positions[0]),
                sub(positions[2], positions[0]),
            )
            .ok_or(SectionError::DegenerateWorldTriangle {
                triangle: triangle_index,
            })?,
        );
    }
    let mut sums = BTreeMap::<(u64, u32, Option<u32>), [f64; 3]>::new();
    for (triangle_index, triangle) in embedding.triangles().iter().copied().enumerate() {
        let patch = embedding.patch_for_triangle(triangle_index);
        for source_vertex in triangle {
            let key = (
                patch.get(),
                source_vertex,
                matches!(mode, SurfaceNormalMode::Flat).then_some(triangle_index as u32),
            );
            let sum = sums.entry(key).or_insert([0.0; 3]);
            for axis in 0..3 {
                sum[axis] += face_normals[triangle_index][axis];
            }
        }
    }
    let mut indexes = BTreeMap::new();
    let mut vertices = Vec::new();
    let mut triangles = Vec::with_capacity(embedding.triangles().len());
    for (triangle_index, triangle) in embedding.triangles().iter().copied().enumerate() {
        let patch = embedding.patch_for_triangle(triangle_index);
        let indices = triangle.map(|source_vertex| {
            let key = (
                patch.get(),
                source_vertex,
                matches!(mode, SurfaceNormalMode::Flat).then_some(triangle_index as u32),
            );
            if let Some(index) = indexes.get(&key) {
                return *index;
            }
            let tetrahedral = embedding.tetra_vertices()[source_vertex as usize];
            let index = vertices.len() as u32;
            vertices.push(PreparedEmbeddedVertex {
                local: embedding.local_vertices()[source_vertex as usize],
                tetrahedral,
                world: geometry.to_world(tetrahedral).map(|value| value as f32),
                normal: unit(sums[&key])
                    .unwrap_or([0.0, 0.0, 1.0])
                    .map(|value| value as f32),
                patch,
                source_vertex: Some(source_vertex),
            });
            indexes.insert(key, index);
            index
        });
        triangles.push(PreparedEmbeddedTriangle {
            indices,
            source_triangle: triangle_index as u32,
            patch,
        });
    }
    Ok(PreparedEmbeddedSurface {
        vertices,
        triangles,
        normal_mode: mode,
    })
}
fn prepare_boundary(
    embedding: &ChartEmbedding,
    geometry: &TetraGeometry,
    tolerance: Tolerance,
    style: EmbeddedChartStyle,
) -> Result<Vec<PreparedEmbeddedLine>, SectionError> {
    if !style.boundary_visible {
        return Ok(Vec::new());
    }
    let chains: Vec<Vec<TernaryPoint>> = match embedding {
        ChartEmbedding::Planar(_) => vec![
            vec![
                TernaryPoint::from_unchecked([1.0, 0.0, 0.0]),
                TernaryPoint::from_unchecked([0.0, 1.0, 0.0]),
            ],
            vec![
                TernaryPoint::from_unchecked([0.0, 1.0, 0.0]),
                TernaryPoint::from_unchecked([0.0, 0.0, 1.0]),
            ],
            vec![
                TernaryPoint::from_unchecked([0.0, 0.0, 1.0]),
                TernaryPoint::from_unchecked([1.0, 0.0, 0.0]),
            ],
        ],
        ChartEmbedding::Triangulated(value) => value
            .topology()
            .boundary_chains()
            .iter()
            .map(|chain| {
                chain
                    .iter()
                    .map(|vertex| value.local_vertices()[vertex.0 as usize])
                    .collect()
            })
            .collect(),
    };
    chains
        .into_iter()
        .map(|points| {
            Ok(PreparedEmbeddedLine {
                series_id: None,
                segments: prepare_polyline(embedding, &points, false, geometry, tolerance)?,
                color: Color::rgb(0.08, 0.08, 0.1),
                width: 1.4,
            })
        })
        .collect()
}

fn prepare_grid(
    embedding: &ChartEmbedding,
    geometry: &TetraGeometry,
    tolerance: Tolerance,
    style: EmbeddedChartStyle,
) -> Result<Option<PreparedEmbeddedGrid>, SectionError> {
    if !style.grid.visible || style.grid.subdivisions < 2 {
        return Ok(None);
    }
    let mut lines = Vec::new();
    for step in 1..style.grid.subdivisions {
        let value = f64::from(step) / f64::from(style.grid.subdivisions);
        for points in [
            vec![
                TernaryPoint::from_unchecked([value, 0.0, 1.0 - value]),
                TernaryPoint::from_unchecked([value, 1.0 - value, 0.0]),
            ],
            vec![
                TernaryPoint::from_unchecked([0.0, value, 1.0 - value]),
                TernaryPoint::from_unchecked([1.0 - value, value, 0.0]),
            ],
            vec![
                TernaryPoint::from_unchecked([0.0, 1.0 - value, value]),
                TernaryPoint::from_unchecked([1.0 - value, 0.0, value]),
            ],
        ] {
            lines.push(PreparedEmbeddedLine {
                series_id: None,
                segments: prepare_polyline(embedding, &points, false, geometry, tolerance)?,
                color: style.grid.color,
                width: style.grid.width,
            });
        }
    }
    Ok(Some(PreparedEmbeddedGrid { lines }))
}

fn prepare_break_lines(
    embedding: &ChartEmbedding,
    geometry: &TetraGeometry,
    tolerance: Tolerance,
) -> Result<Vec<PreparedBreakLine>, SectionError> {
    let ChartEmbedding::Triangulated(value) = embedding else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    for line in value.break_lines() {
        if !line.style().visible {
            continue;
        }
        let points: Vec<_> = line
            .vertices()
            .iter()
            .map(|vertex| value.local_vertices()[vertex.0 as usize])
            .collect();
        result.push(PreparedBreakLine {
            id: line.id(),
            line: PreparedEmbeddedLine {
                series_id: None,
                segments: prepare_polyline(embedding, &points, false, geometry, tolerance)?,
                color: line.style().color,
                width: line.style().width,
            },
        });
    }
    Ok(result)
}

fn prepare_polyline(
    embedding: &ChartEmbedding,
    points: &[TernaryPoint],
    closed: bool,
    geometry: &TetraGeometry,
    tolerance: Tolerance,
) -> Result<Vec<PreparedEmbeddedLineSegment>, SectionError> {
    let mut result = Vec::new();
    for pair in points.windows(2) {
        prepare_segment(
            embedding,
            pair[0],
            pair[1],
            geometry,
            tolerance,
            &mut result,
        )?;
    }
    if closed && points.len() > 2 {
        prepare_segment(
            embedding,
            points[points.len() - 1],
            points[0],
            geometry,
            tolerance,
            &mut result,
        )?;
    }
    Ok(result)
}

fn prepare_segment(
    embedding: &ChartEmbedding,
    first: TernaryPoint,
    second: TernaryPoint,
    geometry: &TetraGeometry,
    tolerance: Tolerance,
    output: &mut Vec<PreparedEmbeddedLineSegment>,
) -> Result<(), SectionError> {
    let Some((first, second)) = clip_local_segment(first.as_array(), second.as_array(), tolerance)?
    else {
        return Ok(());
    };
    let mut splits = vec![(0.0, None), (1.0, None)];
    if let ChartEmbedding::Triangulated(value) = embedding {
        let mut edges = BTreeSet::new();
        for triangle in value.triangles() {
            for [a, b] in [
                [triangle[0], triangle[1]],
                [triangle[1], triangle[2]],
                [triangle[2], triangle[0]],
            ] {
                edges.insert((a.min(b), a.max(b)));
            }
        }
        for (a, b) in edges {
            let local_a = value.local_vertices()[a as usize].as_array();
            let local_b = value.local_vertices()[b as usize].as_array();
            if let Some(t) = segment_intersection(first, second, local_a, local_b, tolerance) {
                let break_id = match value
                    .edge_kind(super::SurfaceVertexIndex(a), super::SurfaceVertexIndex(b))
                {
                    Some(super::SurfaceEdgeKind::BreakLine(id)) => Some(id),
                    _ => None,
                };
                splits.push((t, break_id));
            }
        }
    }
    splits.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut unique: Vec<(f64, Option<BreakLineId>)> = Vec::new();
    for (position, break_id) in splits {
        if let Some((last, last_break)) = unique.last_mut() {
            if (position - *last).abs() <= tolerance.absolute {
                if last_break.is_none() {
                    *last_break = break_id;
                }
                continue;
            }
        }
        unique.push((position, break_id));
    }
    for pair in unique.windows(2) {
        if pair[1].0 - pair[0].0 <= tolerance.absolute {
            continue;
        }
        let start = TernaryPoint::from_unchecked(interpolate(first, second, pair[0].0));
        let end = TernaryPoint::from_unchecked(interpolate(first, second, pair[1].0));
        let midpoint =
            TernaryPoint::from_unchecked(interpolate(first, second, (pair[0].0 + pair[1].0) * 0.5));
        let interior = embedding.locate(midpoint, tolerance)?;
        let start_location = embedding.locate(start, tolerance)?;
        let end_location = embedding.locate(end, tolerance)?;
        output.push(PreparedEmbeddedLineSegment {
            local: [start, end],
            tetrahedral: [start_location.tetrahedral, end_location.tetrahedral],
            world: [
                geometry
                    .to_world(start_location.tetrahedral)
                    .map(|value| value as f32),
                geometry
                    .to_world(end_location.tetrahedral)
                    .map(|value| value as f32),
            ],
            surface_triangle: interior.triangle,
            patch: interior.patch,
            start_break: pair[0].1,
            end_break: pair[1].1,
        });
    }
    Ok(())
}

type ClippedLocalSegment = ([f64; 3], [f64; 3]);

fn clip_local_segment(
    start: [f64; 3],
    end: [f64; 3],
    tolerance: Tolerance,
) -> Result<Option<ClippedLocalSegment>, SectionError> {
    for (index, value) in start.into_iter().chain(end).enumerate() {
        if !value.is_finite() {
            return Err(CoordinateError::NonFiniteComponent {
                component: index % 3,
                value,
            }
            .into());
        }
    }
    let start_sum: f64 = start.into_iter().sum();
    let end_sum: f64 = end.into_iter().sum();
    if !tolerance.is_close(start_sum, 1.0) {
        return Err(CoordinateError::RequiredSumMismatch {
            expected: 1.0,
            actual: start_sum,
            absolute: tolerance.absolute,
        }
        .into());
    }
    if !tolerance.is_close(end_sum, 1.0) {
        return Err(CoordinateError::RequiredSumMismatch {
            expected: 1.0,
            actual: end_sum,
            absolute: tolerance.absolute,
        }
        .into());
    }
    let mut lower: f64 = 0.0;
    let mut upper: f64 = 1.0;
    for axis in 0..3 {
        let delta = end[axis] - start[axis];
        if delta.abs() <= tolerance.absolute {
            if start[axis] < -tolerance.absolute {
                return Ok(None);
            }
        } else if delta > 0.0 {
            lower = lower.max(-start[axis] / delta);
        } else {
            upper = upper.min(-start[axis] / delta);
        }
    }
    if lower > upper + tolerance.absolute {
        return Ok(None);
    }
    Ok(Some((
        interpolate(start, end, lower.clamp(0.0, 1.0)),
        interpolate(start, end, upper.clamp(0.0, 1.0)),
    )))
}

fn segment_intersection(
    start: [f64; 3],
    end: [f64; 3],
    edge_start: [f64; 3],
    edge_end: [f64; 3],
    tolerance: Tolerance,
) -> Option<f64> {
    let r = [end[0] - start[0], end[1] - start[1]];
    let s = [edge_end[0] - edge_start[0], edge_end[1] - edge_start[1]];
    let denominator = cross2(r, s);
    if denominator.abs() <= tolerance.absolute {
        return None;
    }
    let offset = [edge_start[0] - start[0], edge_start[1] - start[1]];
    let t = cross2(offset, s) / denominator;
    let u = cross2(offset, r) / denominator;
    (t > tolerance.absolute
        && t < 1.0 - tolerance.absolute
        && u >= -tolerance.absolute
        && u <= 1.0 + tolerance.absolute)
        .then_some(t)
}

fn interpolate(start: [f64; 3], end: [f64; 3], amount: f64) -> [f64; 3] {
    [0, 1, 2].map(|axis| start[axis] + (end[axis] - start[axis]) * amount)
}
fn cross2(left: [f64; 2], right: [f64; 2]) -> f64 {
    left[0] * right[1] - left[1] * right[0]
}
fn sub(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}
fn unit(value: [f64; 3]) -> Option<[f64; 3]> {
    let length = value
        .into_iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    (length > f64::EPSILON).then_some(value.map(|value| value / length))
}
fn normal(left: [f64; 3], right: [f64; 3]) -> Option<[f64; 3]> {
    let value = [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ];
    let length = value
        .into_iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    (length > f64::EPSILON).then_some(value.map(|value| value / length))
}
