use tetraplot::prelude::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut plot = TetraplotBuilder::new().caption("Edited section").build()?;
    let id = plot.add_section(PlanarSection::constant_component(2, 0.2)?)?;
    let geometry = plot.geometry();
    let tolerance = plot.tolerance();
    let section = plot.section_mut(id).expect("newly-added section exists");
    section.set_constant_component_value(0.45, &geometry, tolerance)?;
    section.chart_mut().add_points([[0.2, 0.3, 0.5]]);
    plot.save_png("edit_section.png", (1000, 800))?;
    Ok(())
}
