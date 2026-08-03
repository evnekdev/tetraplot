use tetraplot::prelude::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut plot = TetraplotBuilder::new().caption("PNG export").build()?;
    plot.draw_series(
        TetraPointSeries::new([[0.25, 0.25, 0.25, 0.25]])
            .color(GREEN)
            .size(10.0),
    )?;
    let image = plot.render((1600, 1200))?;
    image.save_png("save_png.png")?;
    Ok(())
}
