#![cfg(feature = "flat-view")]

use tetraplot::{
    ChartEmbedding, CompositionGrid, EmbeddedTernaryChart, FlatGridPoint, FlatViewTarget,
    GridCoordinateSpace, IrregularCompositionGrid, PlanarEmbedding, TernaryPoint, TetraPoint,
    Tetraplot, TetraplotDocument, Tolerance,
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
    let image = plot
        .flat_chart(FlatViewTarget::EmbeddedChart(id))
        .unwrap()
        .render((800, 700))
        .unwrap();
    assert_eq!((image.width(), image.height()), (800, 700));
    assert!(
        image
            .rgba()
            .chunks_exact(4)
            .any(|pixel| pixel[..3] != [255, 255, 255])
    );
    let local = image.local_at_pixel([400.0, 350.0]).unwrap();
    assert!((local.sum() - 1.0).abs() < 1e-8);
}

#[test]
fn flat_adapter_renders_attached_grid_highlight_and_linked_cursor() {
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
    let chart = plot
        .add_embedded_chart(EmbeddedTernaryChart::new(embedding))
        .unwrap();
    let mut document = TetraplotDocument::new(plot);
    let mut source = IrregularCompositionGrid::local_ternary(
        "samples",
        GridCoordinateSpace::EmbeddedChart(chart),
    )
    .unwrap();
    let row = source.append_raw_row(["0.2", "0.3", "0.5"], Tolerance::default());
    let grid = document.add_grid(CompositionGrid::Irregular(source));
    let target = FlatViewTarget::EmbeddedChart(chart);

    let baseline = document
        .plot()
        .flat_chart(target)
        .unwrap()
        .render((500, 420))
        .unwrap();
    let local = TernaryPoint::new([0.2, 0.3, 0.5]).unwrap();
    let mut flat = document.plot().flat_chart(target).unwrap();
    flat.options_mut().grid_points.push(FlatGridPoint {
        grid,
        row,
        local,
        selected: true,
    });
    flat.options_mut().linked_cursor = Some(TernaryPoint::new([0.4, 0.2, 0.4]).unwrap());
    let highlighted = flat.render((500, 420)).unwrap();

    assert_ne!(baseline.rgba(), highlighted.rgba());
    let pixel = highlighted.pixel_at_local(local).unwrap();
    let recovered = highlighted.local_at_pixel(pixel).unwrap();
    for (actual, expected) in recovered.as_array().into_iter().zip(local.as_array()) {
        assert!((actual - expected).abs() < 0.01);
    }
}
