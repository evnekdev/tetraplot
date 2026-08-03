use tetraplot::prelude::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut plot = TetraplotBuilder::new()
        .caption("Interior points")
        .vertex_labels(["A", "B", "C", "D"])
        .build()?;
    plot.configure_frame().faces(false).draw()?;
    plot.draw_series(
        TetraPointSeries::new([[0.25, 0.25, 0.25, 0.25], [0.5, 0.2, 0.2, 0.1]])
            .color(RED)
            .size(8.0),
    )?;
    plot.save_png("basic_points.png", (1000, 800))?;
    Ok(())
}
