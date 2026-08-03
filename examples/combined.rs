use tetraplot::prelude::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut plot = TetraplotBuilder::new()
        .caption("Combined scientific scene")
        .vertex_labels(["A", "B", "C", "D"])
        .build()?;
    plot.configure_frame().faces(false).draw()?;
    plot.draw_series(
        TetraSurfaceSeries::new(
            [
                [0.4, 0.2, 0.2, 0.2],
                [0.2, 0.4, 0.2, 0.2],
                [0.2, 0.2, 0.4, 0.2],
            ],
            vec![[0, 1, 2]],
        )?
        .opacity(0.4),
    )?;
    plot.draw_series(
        TetraLineSeries::new([[0.7, 0.1, 0.1, 0.1], [0.25, 0.3, 0.25, 0.2]]).width(3.0),
    )?;
    plot.draw_series(
        TetraPointSeries::new([[0.25, 0.25, 0.25, 0.25]])
            .color(RED)
            .size(9.0),
    )?;
    plot.save_png("combined.png", (1200, 900))?;
    Ok(())
}
