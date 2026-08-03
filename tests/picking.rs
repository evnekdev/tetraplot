use tetraplot::{
    ChartEmbedding, EmbeddedTernaryChart, PlanarEmbedding, Ray, TetraPoint, Tetraplot, Tolerance,
    pick_embedded_chart, pick_prepared_surface,
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
