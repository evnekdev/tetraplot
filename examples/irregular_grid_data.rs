//! Demonstrates editable irregular compositions with component D calculated as the dependent component.
use std::fs::File;
use tetraplot::{
    Component, CompositionEntryMode, CompositionGrid, GridExportOptions, IrregularCompositionGrid,
    Tolerance,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut grid = IrregularCompositionGrid::tetrahedral("Irregular measurements");
    grid.set_entry_mode(CompositionEntryMode::DependentComponent(Component::D))?;
    let temperature = grid.add_scalar_field("Temperature");
    let activity = grid.add_scalar_field("Activity");
    let first = grid.append_raw_row(["0.25", "0.30", "0.10"], Tolerance::default());
    let second = grid.append_raw_row(["0.40", "0.20", "0.15"], Tolerance::default());
    grid.set_scalar(first, temperature, Some(1650.0))?;
    grid.set_scalar(first, activity, Some(0.82))?;
    grid.set_scalar(second, temperature, Some(1715.0))?;
    grid.set_scalar(second, activity, Some(0.71))?;
    let grid = CompositionGrid::Irregular(grid);
    grid.write_tsv(
        File::create("irregular_grid_data.tsv")?,
        &GridExportOptions::default(),
    )?;
    println!("Wrote irregular_grid_data.tsv; invalid rows would remain present for correction.");
    Ok(())
}
