//! Two simplified saturation branches joined by a declared univariant break line.
use tetraplot::prelude::*;
use tetraplot::{
    BreakLineKind, BreakLineStyle, ChartEmbedding, EmbeddedTernaryChart, SurfacePatch,
    SurfaceTopology, SurfaceVertexIndex, TriangulatedEmbedding,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tolerance = Tolerance::default();
    let local = vec![
        TernaryPoint::new([1.0, 0.0, 0.0])?,
        TernaryPoint::new([0.0, 1.0, 0.0])?,
        TernaryPoint::new([0.0, 0.0, 1.0])?,
        TernaryPoint::new([0.5, 0.5, 0.0])?,
    ];
    // These are illustrative connected branches, not validated thermodynamic data.
    let parent = vec![
        TetraPoint::new([0.55, 0.20, 0.15, 0.10])?,
        TetraPoint::new([0.15, 0.60, 0.15, 0.10])?,
        TetraPoint::new([0.15, 0.15, 0.55, 0.15])?,
        TetraPoint::new([0.35, 0.35, 0.08, 0.22])?,
    ];
    let topology = SurfaceTopology::new(
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
    );
    let mut embedding = TriangulatedEmbedding::new(
        local,
        parent,
        vec![[0, 3, 2], [3, 1, 2]],
        topology,
        tolerance,
    )?;
    embedding.set_patches(vec![
        SurfacePatch::named(vec![0], "branch I"),
        SurfacePatch::named(vec![1], "branch II"),
    ])?;
    embedding.add_break_line(
        vec![SurfaceVertexIndex(3), SurfaceVertexIndex(2)],
        [None, None],
        BreakLineKind::Univariant,
        BreakLineStyle {
            color: Color::rgb(0.75, 0.08, 0.16),
            width: 3.5,
            ..BreakLineStyle::default()
        },
    )?;

    let mut chart = EmbeddedTernaryChart::new(ChartEmbedding::Triangulated(embedding));
    let mut style = chart.style();
    style.grid.visible = true;
    style.grid.subdivisions = 5;
    chart.set_style(style);
    chart
        .diagram_mut()
        .add_points([[0.65, 0.15, 0.20], [0.18, 0.62, 0.20]]);
    // This local straight line crosses the univariant line and is split into patch fragments.
    chart
        .diagram_mut()
        .add_line([[0.70, 0.10, 0.20], [0.10, 0.70, 0.20]], false);

    let mut plot = TetraplotBuilder::new()
        .caption("Piecewise curved ternary chart")
        .vertex_labels(["A", "B", "C", "D"])
        .build()?;
    plot.configure_frame().faces(false).draw()?;
    plot.add_embedded_chart(chart)?;
    plot.save_png("piecewise_curved_chart.png", (1200, 900))?;

    // Run `cargo run --example piecewise_curved_chart -- --show` to open the native view.
    if std::env::args().any(|argument| argument == "--show") {
        plot.show()?;
    }
    Ok(())
}
