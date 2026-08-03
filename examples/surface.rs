use tetraplot::prelude::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let vertices = [
        [0.4, 0.2, 0.2, 0.2],
        [0.2, 0.4, 0.2, 0.2],
        [0.2, 0.2, 0.4, 0.2],
    ];
    let mut plot = TetraplotBuilder::new()
        .caption("Internal surface")
        .build()?;
    plot.draw_series(TetraSurfaceSeries::new(vertices, vec![[0, 1, 2]])?.opacity(0.65))?;
    plot.save_png("surface.png", (1000, 800))?;
    Ok(())
}
