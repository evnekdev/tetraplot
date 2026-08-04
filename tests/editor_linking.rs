use tetraplot::{
    ChartEmbedding, CompositionGrid, EditorViewport, EmbeddedTernaryChart, GridCoordinateSpace,
    IrregularCompositionGrid, PlanarEmbedding, Ray, Selection, TernaryPoint, TetraPoint, Tetraplot,
    TetraplotDocument, Tolerance, pick_embedded_chart, pick_grid_points,
};

#[test]
fn editor_viewport_converts_top_left_logical_to_bottom_left_physical() {
    let viewport =
        EditorViewport::from_logical_rect([100.0, 50.0, 400.0, 300.0], [1000, 800], 2.0).unwrap();
    assert_eq!(viewport.physical_rect.x, 200);
    assert_eq!(viewport.physical_rect.y, 200);
    assert_eq!(viewport.physical_rect.width, 600);
    assert_eq!(viewport.physical_rect.height, 500);
    assert!(viewport.contains_logical([250.0, 100.0]));
    assert!(viewport.contains_physical([300.0, 300.0]));
    assert!(!viewport.contains_physical([50.0, 300.0]));
    assert!(
        EditorViewport::from_logical_rect([10.0, 10.0, 10.5, 10.5], [1000, 800], 1.0).is_none()
    );
}

#[test]
fn grid_point_pick_returns_stable_grid_and_row_metadata() {
    let mut grid = IrregularCompositionGrid::tetrahedral("points");
    let row = grid.append_raw_row(["0.25", "0.25", "0.25", "0.25"], Tolerance::default());
    let mut document = TetraplotDocument::new(Tetraplot::default());
    let grid_id = document.add_grid(CompositionGrid::Irregular(grid));
    let points = document.prepared_grid_points(grid_id).unwrap();
    let world = points[0].world.map(f64::from);
    let ray = Ray::new([world[0], world[1], world[2] + 1.0], [0.0, 0.0, -1.0]).unwrap();
    let hit = pick_grid_points(ray, &points, 0.01).unwrap();
    assert_eq!(hit.selection, Selection::GridRow { grid: grid_id, row });
    assert_eq!(hit.grid, Some(grid_id));
    assert_eq!(hit.grid_row, Some(row));
}

#[test]
fn embedded_point_has_precedence_over_supporting_surface() {
    let embedding = ChartEmbedding::Planar(
        PlanarEmbedding::new(
            [
                TetraPoint::new([0.5, 0.5, 0.0, 0.0]).unwrap(),
                TetraPoint::new([0.5, 0.0, 0.5, 0.0]).unwrap(),
                TetraPoint::new([0.5, 0.0, 0.0, 0.5]).unwrap(),
            ],
            Tolerance::default(),
        )
        .unwrap(),
    );
    let mut chart = EmbeddedTernaryChart::new(embedding);
    let local = TernaryPoint::new([1.0 / 3.0; 3]).unwrap();
    let series = chart.diagram_mut().add_points([local]);
    let mut plot = Tetraplot::default();
    let chart_id = plot.add_embedded_chart(chart).unwrap();
    let chart = plot.embedded_chart(chart_id).unwrap();
    let prepared = chart.prepared(&plot.geometry(), plot.tolerance()).unwrap();
    let point = prepared.points[0].points[0].world.map(f64::from);
    let triangle = prepared.surface.triangles[0];
    let vertices = triangle.indices.map(|index| {
        prepared.surface.vertices[index as usize]
            .world
            .map(f64::from)
    });
    let edge_a = [
        vertices[1][0] - vertices[0][0],
        vertices[1][1] - vertices[0][1],
        vertices[1][2] - vertices[0][2],
    ];
    let edge_b = [
        vertices[2][0] - vertices[0][0],
        vertices[2][1] - vertices[0][1],
        vertices[2][2] - vertices[0][2],
    ];
    let normal = [
        edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1],
        edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2],
        edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0],
    ];
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    let normal = normal.map(|value| value / length);
    let ray = Ray::new(
        [
            point[0] + normal[0],
            point[1] + normal[1],
            point[2] + normal[2],
        ],
        normal.map(|value| -value),
    )
    .unwrap();
    let hit = pick_embedded_chart(
        ray,
        chart_id,
        chart.embedding(),
        &prepared,
        &plot.geometry(),
        plot.tolerance(),
    )
    .unwrap();
    assert_eq!(
        hit.selection,
        Selection::EmbeddedSeries {
            chart: chart_id,
            series
        }
    );
    assert_eq!(hit.primitive_index, Some(0));
}

#[test]
fn sorted_table_linkage_scrolls_to_the_same_stable_row() {
    let embedding = PlanarEmbedding::new(
        [
            TetraPoint::new([0.5, 0.5, 0.0, 0.0]).unwrap(),
            TetraPoint::new([0.5, 0.0, 0.5, 0.0]).unwrap(),
            TetraPoint::new([0.5, 0.0, 0.0, 0.5]).unwrap(),
        ],
        Tolerance::default(),
    )
    .unwrap();
    let mut plot = Tetraplot::default();
    let chart = plot
        .add_embedded_chart(EmbeddedTernaryChart::new(ChartEmbedding::Planar(embedding)))
        .unwrap();
    let mut grid =
        IrregularCompositionGrid::local_ternary("local", GridCoordinateSpace::EmbeddedChart(chart))
            .unwrap();
    let first = grid.append_raw_row(["0.2", "0.3", "0.5"], Tolerance::default());
    let second = grid.append_raw_row(["0.4", "0.4", "0.2"], Tolerance::default());
    let mut document = TetraplotDocument::new(plot);
    let grid_id = document.add_grid(CompositionGrid::Irregular(grid));
    let mut state = tetraplot::EditorState::default();
    tetraplot::EditorCommand::OpenDataGrid(grid_id)
        .execute(&mut document, &mut state)
        .unwrap();
    state
        .tables
        .get_mut(&grid_id)
        .unwrap()
        .replace_row_order(vec![second, first]);
    state.select_grid_row(&document, grid_id, first).unwrap();
    assert_eq!(state.tables[&grid_id].first_visible_row, 1);
    assert_eq!(
        state.selection,
        Selection::GridRow {
            grid: grid_id,
            row: first
        }
    );
}
