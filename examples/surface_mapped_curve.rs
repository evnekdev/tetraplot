use tetraplot::prelude::*;
use tetraplot::{SurfaceCurve, SurfacePoint, TetraSurface};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let surface = TetraSurface::new(
        [
            [0.4, 0.2, 0.2, 0.2],
            [0.2, 0.4, 0.2, 0.2],
            [0.2, 0.2, 0.4, 0.2],
        ],
        vec![[0, 1, 2]],
    )?;
    let curve = SurfaceCurve::new(vec![
        SurfacePoint::new(0, [0.8, 0.1, 0.1])?,
        SurfacePoint::new(0, [0.2, 0.6, 0.2])?,
        SurfacePoint::new(0, [0.1, 0.1, 0.8])?,
    ]);
    let points = curve.evaluate(&surface, Tolerance::default())?;
    let mut plot = TetraplotBuilder::new()
        .caption("Surface-attached curve")
        .build()?;
    plot.draw_series(TetraLineSeries::new(points).color(BLUE).width(3.0))?;
    plot.save_png("surface_mapped_curve.png", (1000, 800))?;
    Ok(())
}
