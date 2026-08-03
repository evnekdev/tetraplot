use tetraplot::prelude::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut plot = TetraplotBuilder::new().caption("25% A section").build()?;
    let mut section = PlanarSection::constant_component(0, 0.25)?
        .name("A = 0.25")
        .fill_opacity(0.2)
        .show_grid(true);
    section.chart_mut().add_points([[0.3, 0.3, 0.4]]);
    section
        .chart_mut()
        .add_line([[0.8, 0.1, 0.1], [0.2, 0.6, 0.2]], false);
    plot.add_section(section)?;
    plot.save_png("constant_component_section.png", (1000, 800))?;
    Ok(())
}
