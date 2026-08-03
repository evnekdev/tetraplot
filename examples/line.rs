use tetraplot::prelude::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut plot = TetraplotBuilder::new()
        .caption("Composition path")
        .build()?;
    plot.draw_series(
        TetraLineSeries::new([
            [0.85, 0.05, 0.05, 0.05],
            [0.55, 0.2, 0.15, 0.1],
            [0.2, 0.35, 0.25, 0.2],
        ])
        .color(BLUE)
        .width(3.0),
    )?;
    plot.save_png("line.png", (1000, 800))?;
    Ok(())
}
