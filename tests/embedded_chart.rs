use tetraplot::prelude::*;
use tetraplot::{
    BreakLineKind, BreakLineStyle, ChartEmbedding, EmbeddedChartStyle, SectionError, SurfacePatch,
    SurfaceTopology, SurfaceVertexIndex, TriangulatedEmbedding,
};

fn topology() -> SurfaceTopology {
    SurfaceTopology::new(
        [
            SurfaceVertexIndex(0),
            SurfaceVertexIndex(1),
            SurfaceVertexIndex(2),
        ],
        [
            vec![
                SurfaceVertexIndex(0),
                SurfaceVertexIndex(3),
                SurfaceVertexIndex(1),
            ],
            vec![SurfaceVertexIndex(1), SurfaceVertexIndex(2)],
            vec![SurfaceVertexIndex(2), SurfaceVertexIndex(0)],
        ],
    )
}

fn embedding() -> TriangulatedEmbedding {
    TriangulatedEmbedding::new(
        vec![
            TernaryPoint::new([1.0, 0.0, 0.0]).unwrap(),
            TernaryPoint::new([0.0, 1.0, 0.0]).unwrap(),
            TernaryPoint::new([0.0, 0.0, 1.0]).unwrap(),
            TernaryPoint::new([0.5, 0.5, 0.0]).unwrap(),
        ],
        vec![
            TetraPoint::new([0.55, 0.20, 0.15, 0.10]).unwrap(),
            TetraPoint::new([0.15, 0.60, 0.15, 0.10]).unwrap(),
            TetraPoint::new([0.15, 0.15, 0.55, 0.15]).unwrap(),
            TetraPoint::new([0.35, 0.35, 0.08, 0.22]).unwrap(),
        ],
        vec![[0, 3, 2], [3, 1, 2]],
        topology(),
        Tolerance::default(),
    )
    .unwrap()
}

fn piecewise_embedding() -> TriangulatedEmbedding {
    let mut result = embedding();
    result
        .set_patches(vec![
            SurfacePatch::named(vec![0], "low-temperature branch"),
            SurfacePatch::named(vec![1], "high-temperature branch"),
        ])
        .unwrap();
    result
        .add_break_line(
            vec![SurfaceVertexIndex(3), SurfaceVertexIndex(2)],
            [None, None],
            BreakLineKind::Univariant,
            BreakLineStyle {
                color: Color::rgb(0.95, 0.05, 0.85),
                width: 3.0,
                ..BreakLineStyle::default()
            },
        )
        .unwrap();
    result
}

#[test]
fn mapping_locates_parent_triangle_and_round_trips() {
    let surface = piecewise_embedding();
    let point = TernaryPoint::new([0.70, 0.10, 0.20]).unwrap();
    let located = surface.locate(point, Tolerance::default()).unwrap();
    assert_eq!(located.triangle, 0);
    assert_eq!(located.patch, surface.patches()[0].id());
    let recovered = surface
        .unmap(located.tetrahedral, Tolerance::default())
        .unwrap();
    for (actual, expected) in recovered.as_array().into_iter().zip(point.as_array()) {
        assert!((actual - expected).abs() < 1.0e-10);
    }
    assert!(matches!(
        surface.map(
            TernaryPoint::from_unchecked([1.1, -0.1, 0.0]),
            Tolerance::default()
        ),
        Err(SectionError::Coordinate(_))
    ));
}

#[test]
fn adjacency_patches_and_break_lines_are_validated() {
    let mut surface = embedding();
    assert_eq!(
        surface
            .edge_incident_triangles(SurfaceVertexIndex(3), SurfaceVertexIndex(2))
            .unwrap(),
        vec![0, 1]
    );
    assert_eq!(
        surface.boundary_chain(SurfaceVertexIndex(0), SurfaceVertexIndex(3)),
        Some(0)
    );
    surface
        .set_patches(vec![SurfacePatch::new(vec![0]), SurfacePatch::new(vec![1])])
        .unwrap();
    let patches = [surface.patches()[0].id(), surface.patches()[1].id()];
    let id = surface
        .add_break_line(
            vec![SurfaceVertexIndex(3), SurfaceVertexIndex(2)],
            [Some(patches[1]), Some(patches[0])],
            BreakLineKind::Crease,
            BreakLineStyle::default(),
        )
        .unwrap();
    assert_eq!(surface.break_lines()[0].adjacent_patches(), patches);
    assert!(
        matches!(surface.edge_kind(SurfaceVertexIndex(3), SurfaceVertexIndex(2)), Some(tetraplot::SurfaceEdgeKind::BreakLine(found)) if found == id)
    );
    assert!(matches!(
        surface.add_break_line(
            vec![SurfaceVertexIndex(0), SurfaceVertexIndex(3)],
            [None, None],
            BreakLineKind::Crease,
            BreakLineStyle::default()
        ),
        Err(SectionError::BreakLineUsesBoundaryEdge { .. })
    ));
    assert!(matches!(
        surface.add_break_line(
            vec![SurfaceVertexIndex(0), SurfaceVertexIndex(2)],
            [None, None],
            BreakLineKind::Crease,
            BreakLineStyle::default()
        ),
        Err(SectionError::BreakLineUsesBoundaryEdge { .. })
    ));
    assert!(matches!(
        surface.add_break_line(
            vec![SurfaceVertexIndex(3), SurfaceVertexIndex(2)],
            [None, None],
            BreakLineKind::Crease,
            BreakLineStyle::default()
        ),
        Err(SectionError::EdgeAlreadyAssignedToBreakLine { .. })
    ));
    assert!(surface.remove_break_line(id).is_some());
    assert!(matches!(
        surface.edge_kind(SurfaceVertexIndex(3), SurfaceVertexIndex(2)),
        Some(tetraplot::SurfaceEdgeKind::Smooth)
    ));
}

#[test]
fn patch_partition_rejects_overlap_and_mixed_orientation() {
    let mut surface = embedding();
    assert!(matches!(
        surface.set_patches(vec![
            SurfacePatch::new(vec![0, 1]),
            SurfacePatch::new(vec![1])
        ]),
        Err(SectionError::OverlappingPatchMembership { triangle: 1 })
    ));
    let local = vec![
        TernaryPoint::new([1.0, 0.0, 0.0]).unwrap(),
        TernaryPoint::new([0.0, 1.0, 0.0]).unwrap(),
        TernaryPoint::new([0.0, 0.0, 1.0]).unwrap(),
        TernaryPoint::new([0.5, 0.5, 0.0]).unwrap(),
    ];
    let parent = embedding().tetra_vertices().to_vec();
    assert!(matches!(
        TriangulatedEmbedding::new(
            local,
            parent,
            vec![[0, 3, 2], [3, 2, 1]],
            topology(),
            Tolerance::default()
        ),
        Err(SectionError::InconsistentLocalOrientation { triangle: 1 })
    ));
}

#[test]
fn prepared_normals_share_smooth_vertices_and_duplicate_break_vertices() {
    let geometry = TetraGeometry::default();
    let mut smooth = EmbeddedTernaryChart::new(ChartEmbedding::Triangulated(embedding()));
    let smooth_prepared = smooth.prepared(&geometry, Tolerance::default()).unwrap();
    assert_eq!(
        smooth_prepared
            .surface
            .vertices
            .iter()
            .filter(|vertex| vertex.source_vertex == Some(2))
            .count(),
        1
    );
    let mut broken = EmbeddedTernaryChart::new(ChartEmbedding::Triangulated(piecewise_embedding()));
    let broken_prepared = broken.prepared(&geometry, Tolerance::default()).unwrap();
    let copies: Vec<_> = broken_prepared
        .surface
        .vertices
        .iter()
        .filter(|vertex| vertex.source_vertex == Some(2))
        .collect();
    assert_eq!(copies.len(), 2);
    assert_ne!(copies[0].patch, copies[1].patch);
    let dot: f32 = copies[0]
        .normal
        .into_iter()
        .zip(copies[1].normal)
        .map(|(a, b)| a * b)
        .sum();
    assert!(dot < 0.999);
    smooth.set_visible(false);
    broken.set_visible(false);
}

#[test]
fn mapped_lines_and_grid_split_at_the_break() {
    let geometry = TetraGeometry::default();
    let mut chart = EmbeddedTernaryChart::new(ChartEmbedding::Triangulated(piecewise_embedding()));
    let mut style: EmbeddedChartStyle = chart.style();
    style.grid.visible = true;
    style.grid.subdivisions = 4;
    chart.set_style(style);
    chart
        .diagram_mut()
        .add_line([[0.70, 0.10, 0.20], [0.10, 0.70, 0.20]], false);
    let prepared = chart.prepared(&geometry, Tolerance::default()).unwrap();
    let line = &prepared.lines[0];
    assert_eq!(line.segments.len(), 2);
    assert_ne!(line.segments[0].patch, line.segments[1].patch);
    for (left, right) in line.segments[0].tetrahedral[1]
        .as_array()
        .into_iter()
        .zip(line.segments[1].tetrahedral[0].as_array())
    {
        assert!((left - right).abs() < 1.0e-12);
    }
    assert!(line.segments[0].end_break.is_some());
    assert!(
        prepared
            .grid
            .as_ref()
            .unwrap()
            .lines
            .iter()
            .any(|line| line.segments.len() > 1)
    );
}

#[test]
fn software_renderer_draws_and_hides_embedded_charts_deterministically() {
    let mut chart = EmbeddedTernaryChart::new(ChartEmbedding::Triangulated(piecewise_embedding()));
    let mut style = chart.style();
    style.grid.visible = true;
    chart.set_style(style);
    chart.diagram_mut().add_points([[0.6, 0.2, 0.2]]);
    chart
        .diagram_mut()
        .add_line([[0.7, 0.1, 0.2], [0.1, 0.7, 0.2]], false);
    let mut plot = TetraplotBuilder::new().background(WHITE).build().unwrap();
    let id = plot.add_embedded_chart(chart).unwrap();
    let first = plot.render((360, 280)).unwrap();
    let second = plot.render((360, 280)).unwrap();
    assert_eq!(first, second);
    assert_eq!((first.width(), first.height()), (360, 280));
    assert!(
        first
            .rgba()
            .chunks_exact(4)
            .any(|pixel| pixel != [255, 255, 255, 255])
    );
    assert!(
        first
            .rgba()
            .chunks_exact(4)
            .any(|pixel| { pixel[0] > 180 && pixel[1] < 100 && pixel[2] > 150 })
    );
    let visible = first.rgba().to_vec();
    plot.embedded_chart_mut(id).unwrap().set_visible(false);
    let hidden = plot.render((360, 280)).unwrap();
    assert_ne!(visible, hidden.rgba());
}

#[test]
fn break_line_rejects_non_edges_and_wrong_patch_metadata() {
    let mut surface = embedding();
    surface
        .set_patches(vec![SurfacePatch::new(vec![0]), SurfacePatch::new(vec![1])])
        .unwrap();
    let patches = [surface.patches()[0].id(), surface.patches()[1].id()];
    assert!(matches!(
        surface.add_break_line(
            vec![SurfaceVertexIndex(0), SurfaceVertexIndex(1)],
            [None, None],
            BreakLineKind::Crease,
            BreakLineStyle::default()
        ),
        Err(SectionError::BreakLineSegmentIsNotMeshEdge { .. })
    ));
    assert!(matches!(
        surface.add_break_line(
            vec![SurfaceVertexIndex(3), SurfaceVertexIndex(2)],
            [Some(patches[0]), Some(patches[0])],
            BreakLineKind::Crease,
            BreakLineStyle::default()
        ),
        Err(SectionError::BreakLinePatchMismatch { .. })
    ));
}
