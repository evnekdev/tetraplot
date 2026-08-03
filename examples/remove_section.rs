use tetraplot::prelude::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut plot = TetraplotBuilder::new().caption("Section removal").build()?;
    let first = plot.add_section(PlanarSection::constant_component(0, 0.2)?)?;
    let second = plot.add_section(PlanarSection::constant_component(1, 0.35)?)?;
    assert!(plot.remove_section(first).is_some());
    assert!(plot.section(second).is_some());
    plot.save_png("remove_section.png", (1000, 800))?;
    Ok(())
}
