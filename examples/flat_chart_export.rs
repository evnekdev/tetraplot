//! Writes the same local section diagram as a conventional plotters-ternary PNG.
use tetraplot::{FlatViewTarget, PlanarSection, TetraplotBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut plot = TetraplotBuilder::new().build()?;
    let mut section = PlanarSection::constant_component(0, 0.25)?.show_grid(true);
    section.chart_mut().add_points([[0.30, 0.30, 0.40]]);
    section
        .chart_mut()
        .add_line([[0.75, 0.15, 0.10], [0.20, 0.55, 0.25]], false);
    let id = plot.add_section(section)?;
    plot.flat_chart(FlatViewTarget::Section(id))?
        .save_png("flat_chart_export.png", (1200, 1000))?;
    println!("Wrote flat_chart_export.png");
    Ok(())
}
