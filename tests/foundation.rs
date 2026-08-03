use tetraplot::prelude::*;
use tetraplot::{
    Camera, DomainClip, Projection, SectionIntersection, TetraPointLocation, TetraSegment,
    clip_segment_with_parameters,
};
#[test]
fn point_validation_normalization_and_classification_are_explicit() {
    assert!(TetraPoint::new([0.4, 0.3, 0.2, 0.2]).is_err());
    let normalized = TetraPoint::from_unchecked([2.0, 3.0, 4.0, 1.0])
        .normalized(Tolerance::default())
        .unwrap();
    assert!((normalized.sum() - 1.0).abs() < 1.0e-12);
    let geometry = TetraGeometry::default();
    assert_eq!(
        geometry.classify(TetraPoint::new([0.25; 4]).unwrap(), Tolerance::default()),
        TetraPointLocation::Interior
    );
    assert_eq!(
        geometry.classify(
            TetraPoint::new([0.0, 0.4, 0.3, 0.3]).unwrap(),
            Tolerance::default()
        ),
        TetraPointLocation::Face(tetraplot::Face::OppositeA)
    );
    assert_eq!(
        geometry.classify(
            TetraPoint::new([0.0, 0.0, 0.5, 0.5]).unwrap(),
            Tolerance::default()
        ),
        TetraPointLocation::Edge(tetraplot::Edge::CD)
    );
}
#[test]
fn tetrahedron_round_trip_and_barycentric_clipping_are_stable() {
    let geometry = TetraGeometry::default();
    let point = TetraPoint::new([0.1, 0.2, 0.3, 0.4]).unwrap();
    let recovered = geometry
        .from_world(geometry.to_world(point), Tolerance::default())
        .unwrap();
    for (actual, expected) in recovered.as_array().into_iter().zip(point.as_array()) {
        assert!((actual - expected).abs() < 1.0e-10);
    }
    let start = TetraPoint::from_unchecked([-0.4, 0.7, 0.4, 0.3]);
    let end = TetraPoint::from_unchecked([0.6, 0.2, 0.1, 0.1]);
    let clipped = clip_segment_with_parameters(TetraSegment::new(start, end), Tolerance::default())
        .unwrap()
        .unwrap();
    assert!(clipped.parameter_start > 0.0);
    assert!(
        clipped
            .segment
            .start
            .as_array()
            .iter()
            .all(|weight| *weight >= -1.0e-12)
    );
}
#[test]
fn implicit_camera_follows_geometry_and_explicit_camera_is_preserved() {
    let geometry = TetraGeometry::new(
        [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 3.0, 0.0],
            [0.0, 0.0, 4.0],
        ],
        Tolerance::default(),
    )
    .unwrap();
    let implicit = TetraplotBuilder::new().geometry(geometry).build().unwrap();
    assert_eq!(implicit.scene_bounds(), geometry.bounds());
    let camera = Camera::new(
        [10.0, 10.0, 10.0],
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        Projection::Orthographic { vertical_span: 7.0 },
    )
    .unwrap();
    let explicit = TetraplotBuilder::new()
        .camera(camera)
        .geometry(geometry)
        .build()
        .unwrap();
    assert_eq!(explicit.camera(), camera);
}
#[test]
fn sections_preserve_topology_transform_and_stable_identity() {
    let geometry = TetraGeometry::default();
    let tolerance = Tolerance::default();
    let plane = tetraplot::SectionPlane::constant_component(0, 0.25).unwrap();
    let intersection = plane.intersection(&geometry, tolerance).unwrap();
    let SectionIntersection::Triangle(triangle) = intersection else {
        panic!("constant component must be triangular");
    };
    let transform = tetraplot::SectionTransform::new(
        triangle.vertices.map(|vertex| vertex.tetrahedral),
        tolerance,
    )
    .unwrap();
    let local = TernaryPoint::new([0.2, 0.3, 0.5]).unwrap();
    let world = transform.map(local).unwrap();
    let recovered = transform.unmap(world, tolerance).unwrap();
    for (actual, expected) in recovered.as_array().into_iter().zip(local.as_array()) {
        assert!((actual - expected).abs() < 1.0e-12);
    }
    let mut plot = TetraplotBuilder::new().build().unwrap();
    let first = plot
        .add_section(PlanarSection::constant_component(0, 0.25).unwrap())
        .unwrap();
    let second = plot
        .add_section(PlanarSection::constant_component(1, 0.35).unwrap())
        .unwrap();
    assert!(plot.remove_section(first).is_some());
    assert!(plot.section(second).is_some());
    assert!(plot.remove_section(first).is_none());
}
#[test]
fn general_plane_reports_quadrilateral_instead_of_hidden_triangulation() {
    let geometry = TetraGeometry::default();
    let intersection = tetraplot::SectionPlane::Barycentric {
        coefficients: [1.0, 1.0, 0.0, 0.0],
        value: 0.5,
    }
    .intersection(&geometry, Tolerance::default())
    .unwrap();
    assert!(matches!(
        intersection,
        SectionIntersection::Quadrilateral(_)
    ));
}
#[test]
fn prepared_series_and_static_render_use_the_same_scene() {
    let mut plot = TetraplotBuilder::new().background(WHITE).build().unwrap();
    plot.draw_series(
        TetraPointSeries::new([[0.25, 0.25, 0.25, 0.25]])
            .color(RED)
            .size(12.0),
    )
    .unwrap();
    plot.draw_series(
        TetraLineSeries::new([[0.9, 0.1, 0.0, 0.0], [0.1, 0.3, 0.3, 0.3]])
            .color(BLUE)
            .width(3.0)
            .clip(DomainClip::Tetrahedron),
    )
    .unwrap();
    let image = plot.render((320, 240)).unwrap();
    assert_eq!((image.width(), image.height()), (320, 240));
    assert!(
        image
            .rgba()
            .chunks_exact(4)
            .any(|pixel| pixel != [255, 255, 255, 255])
    );
}

#[test]
fn triangulated_embedding_maps_and_unmaps_local_coordinates() {
    let local = vec![
        TernaryPoint::new([1.0, 0.0, 0.0]).unwrap(),
        TernaryPoint::new([0.0, 1.0, 0.0]).unwrap(),
        TernaryPoint::new([0.0, 0.0, 1.0]).unwrap(),
    ];
    let parent = vec![
        TetraPoint::new([1.0, 0.0, 0.0, 0.0]).unwrap(),
        TetraPoint::new([0.0, 1.0, 0.0, 0.0]).unwrap(),
        TetraPoint::new([0.0, 0.0, 1.0, 0.0]).unwrap(),
    ];
    let topology = tetraplot::SurfaceTopology::new(
        [
            tetraplot::SurfaceVertexIndex(0),
            tetraplot::SurfaceVertexIndex(1),
            tetraplot::SurfaceVertexIndex(2),
        ],
        [
            vec![
                tetraplot::SurfaceVertexIndex(0),
                tetraplot::SurfaceVertexIndex(1),
            ],
            vec![
                tetraplot::SurfaceVertexIndex(1),
                tetraplot::SurfaceVertexIndex(2),
            ],
            vec![
                tetraplot::SurfaceVertexIndex(2),
                tetraplot::SurfaceVertexIndex(0),
            ],
        ],
    );
    let embedding = TriangulatedEmbedding::new(
        local,
        parent,
        vec![[0, 1, 2]],
        topology,
        Tolerance::default(),
    )
    .unwrap();
    let source = TernaryPoint::new([0.2, 0.3, 0.5]).unwrap();
    let mapped = embedding.map(source, Tolerance::default()).unwrap();
    for (actual, expected) in mapped.as_array().into_iter().zip([0.2, 0.3, 0.5, 0.0]) {
        assert!((actual - expected).abs() < 1.0e-12);
    }
    let recovered = embedding.unmap(mapped, Tolerance::default()).unwrap();
    for (actual, expected) in recovered.as_array().into_iter().zip(source.as_array()) {
        assert!((actual - expected).abs() < 1.0e-12);
    }
}
