//! Generates a regular tetrahedral composition lattice and exports one scalar field as TSV.
use std::fs::File;
use tetraplot::{
    CompositionGrid, GridExportOptions, RegularCompositionGrid, RegularTetraGridDefinition,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut grid = RegularCompositionGrid::tetrahedral(
        "Regular tetrahedron grid",
        RegularTetraGridDefinition {
            subdivisions: 4,
            ..Default::default()
        },
    )?;
    let field = grid.add_scalar_field("Temperature");
    let rows: Vec<_> = grid
        .points()
        .iter()
        .take(6)
        .map(|point| (point.row_id, point.logical_index))
        .collect();
    for (row, index) in rows {
        grid.set_scalar(row, field, Some(1450.0 + index.0[0] as f64 * 25.0))?;
    }
    let grid = CompositionGrid::Regular(grid);
    grid.write_tsv(
        File::create("regular_grid_data.tsv")?,
        &GridExportOptions::default(),
    )?;
    println!(
        "Wrote regular_grid_data.tsv with {} generated compositions.",
        grid.rows_len()
    );
    Ok(())
}
