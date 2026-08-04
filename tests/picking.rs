use tetraplot::{
    ChartEmbedding, EditorViewport, EmbeddedTernaryChart, LogicalViewportRect, PlanarEmbedding,
    Ray, Selection, TetraPoint, TetraPointSeries, Tetraplot, Tolerance, pick_embedded_chart,
    pick_embedded_series, pick_plot_series, pick_prepared_surface, select_best_pick,
};
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn norm(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}

#[test]
fn ray_hit_recovers_local_and_parent_coordinates() {
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
    let mut plot = Tetraplot::default();
    let id = plot
        .add_embedded_chart(EmbeddedTernaryChart::new(embedding))
        .unwrap();
    let chart = plot.embedded_chart(id).unwrap();
    let prepared = chart.prepared(&plot.geometry(), plot.tolerance()).unwrap();
    let triangle = prepared.surface.triangles[0];
    let vertices = triangle.indices.map(|index| {
        prepared.surface.vertices[index as usize]
            .world
            .map(f64::from)
    });
    let center = [
        (vertices[0][0] + vertices[1][0] + vertices[2][0]) / 3.0,
        (vertices[0][1] + vertices[1][1] + vertices[2][1]) / 3.0,
        (vertices[0][2] + vertices[1][2] + vertices[2][2]) / 3.0,
    ];
    let normal = norm(cross(
        sub(vertices[1], vertices[0]),
        sub(vertices[2], vertices[0]),
    ));
    let ray = Ray::new(
        [
            center[0] + normal[0],
            center[1] + normal[1],
            center[2] + normal[2],
        ],
        [-normal[0], -normal[1], -normal[2]],
    )
    .unwrap();
    let hit = pick_prepared_surface(ray, &prepared, &plot.geometry(), plot.tolerance()).unwrap();
    assert_eq!(hit.surface_triangle, 0);
    assert_eq!(hit.patch.get(), 0);
    let local = hit.local.as_array();
    assert!((local[0] + local[1] + local[2] - 1.0).abs() < 1e-9);
    let editor_hit = pick_embedded_chart(
        ray,
        id,
        chart.embedding(),
        &prepared,
        &plot.geometry(),
        plot.tolerance(),
    )
    .unwrap();
    assert_eq!(editor_hit.surface_triangle, Some(0));
}

#[test]
fn ray_miss_is_not_an_error() {
    let ray = Ray::new([0.0, 0.0, 2.0], [0.0, 1.0, 0.0]).unwrap();
    assert_eq!(ray.direction, [0.0, 1.0, 0.0]);
}

#[test]
fn chart_point_wins_over_supporting_surface_and_recovers_series_identity() {
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
    let series = chart.diagram_mut().add_points([[0.2, 0.3, 0.5]]);
    let mut plot = Tetraplot::default();
    let chart_id = plot.add_embedded_chart(chart).unwrap();
    let chart = plot.embedded_chart(chart_id).unwrap();
    let prepared = chart.prepared(&plot.geometry(), plot.tolerance()).unwrap();
    let point = &prepared.points[0].points[0];
    let triangle = prepared.surface.triangles[0];
    let vertices = triangle.indices.map(|index| {
        prepared.surface.vertices[index as usize]
            .world
            .map(f64::from)
    });
    let normal = norm(cross(
        sub(vertices[1], vertices[0]),
        sub(vertices[2], vertices[0]),
    ));
    let world = point.world.map(f64::from);
    let ray = Ray::new(
        [
            world[0] + normal[0],
            world[1] + normal[1],
            world[2] + normal[2],
        ],
        [-normal[0], -normal[1], -normal[2]],
    )
    .unwrap();

    let element = pick_embedded_series(ray, chart_id, &prepared, 0.02).unwrap();
    assert_eq!(
        element.selection,
        Selection::EmbeddedSeries {
            chart: chart_id,
            series
        }
    );
    assert_eq!(element.primitive_index, Some(0));
    assert_eq!(element.series_id, Some(series));
    assert_eq!(element.local_position.unwrap().as_array(), [0.2, 0.3, 0.5]);

    let surface = pick_embedded_chart(
        ray,
        chart_id,
        chart.embedding(),
        &prepared,
        &plot.geometry(),
        plot.tolerance(),
    )
    .unwrap();
    let best = select_best_pick(ray, [surface, element]).unwrap();
    assert!(matches!(best.selection, Selection::EmbeddedSeries { .. }));
}

#[test]
fn logical_editor_rect_converts_to_bottom_left_physical_viewport() {
    let viewport = EditorViewport::from_logical(
        LogicalViewportRect {
            min: [100.0, 50.0],
            max: [900.0, 650.0],
        },
        [2000, 1400],
        2.0,
    )
    .unwrap();
    assert_eq!(viewport.physical_rect.x, 200);
    assert_eq!(viewport.physical_rect.y, 100);
    assert_eq!(viewport.physical_rect.width, 1600);
    assert_eq!(viewport.physical_rect.height, 1200);
    assert!(viewport.contains_physical([500.0, 500.0]));
    assert!(!viewport.contains_physical([50.0, 500.0]));
}

#[test]
fn ordinary_point_series_pick_preserves_stable_series_and_source_index() {
    let mut plot = Tetraplot::default();
    let series = plot
        .draw_series(TetraPointSeries::new([
            [0.25, 0.25, 0.25, 0.25],
            [0.4, 0.2, 0.2, 0.2],
        ]))
        .unwrap();
    let point = match &plot.prepared_series()[0].1 {
        tetraplot::PreparedSeries::Points { points, .. } => points[1],
        _ => unreachable!(),
    };
    let world = point.world.map(f64::from);
    let ray = Ray::new([world[0], world[1], world[2] + 2.0], [0.0, 0.0, -1.0]).unwrap();

    let hit = pick_plot_series(ray, &plot, 0.02).unwrap();
    assert_eq!(hit.selection, Selection::Series(series));
    assert_eq!(hit.primitive_index, Some(1));
    assert_eq!(
        hit.tetrahedral_position.unwrap().as_array(),
        [0.4, 0.2, 0.2, 0.2]
    );
}
