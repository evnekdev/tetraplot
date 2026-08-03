use tetraplot::prelude::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let geometry = TetraGeometry::new(
        [
            [0.0, 0.0, 1.4],
            [-1.0, -0.8, -0.6],
            [1.0, -0.8, -0.6],
            [0.0, 1.0, -0.6],
        ],
        Tolerance::default(),
    )?;
    let mut plot = TetraplotBuilder::new()
        .geometry(geometry)
        .caption("Custom tetrahedron")
        .build()?;
    plot.draw_series(
        TetraPointSeries::new([[0.25, 0.25, 0.25, 0.25]])
            .color(RED)
            .size(9.0),
    )?;
    plot.save_png("custom_geometry.png", (1000, 800))?;
    Ok(())
}
