#![cfg(feature = "flat-view")]

use tetraplot::{
    ChartEmbedding, EmbeddedTernaryChart, FlatViewTarget, PlanarEmbedding, TetraPoint, Tetraplot,
    Tolerance,
};

#[test]
fn flat_adapter_renders_local_diagram_and_recovers_hover_coordinate() {
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
    chart.diagram_mut().add_points([[0.3, 0.4, 0.3]]);
    chart
        .diagram_mut()
        .add_line([[0.8, 0.1, 0.1], [0.1, 0.8, 0.1]], false);
    let mut plot = Tetraplot::default();
    let id = plot.add_embedded_chart(chart).unwrap();
    let marker = tetraplot::TernaryPoint::new([0.2, 0.5, 0.3]).unwrap();
    let mut flat = plot.flat_chart(FlatViewTarget::EmbeddedChart(id)).unwrap();
    flat.options_mut().grid_points = vec![marker];
    flat.options_mut().selected_points = vec![marker];
    flat.options_mut().linked_cursor = Some(marker);
    let image = flat.render((800, 700)).unwrap();
    assert_eq!((image.width(), image.height()), (800, 700));
    assert!(
        image
            .rgba()
            .chunks_exact(4)
            .any(|pixel| pixel[..3] != [255, 255, 255])
    );
    let local = image.local_at_pixel([400.0, 350.0]).unwrap();
    let marker_pixel = image.pixel_at_local(marker).unwrap();
    let recovered = image.local_at_pixel(marker_pixel).unwrap();
    for (expected, actual) in marker.as_array().into_iter().zip(recovered.as_array()) {
        assert!((expected - actual).abs() < 1.0e-6);
    }
    assert!((local.sum() - 1.0).abs() < 1e-8);
}
