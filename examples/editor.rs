//! Launch the alpha scientific editor with linked planar/curved charts and editable data grids.
//!
//! In the Data table tab, select generated regular-grid compositions and use Copy compositions
//! for Excel. Paste one or several scalar columns at the active scalar cell. The irregular grid
//! accepts composition-plus-scalar TSV blocks, exposes its dependent-component selector, and keeps
//! invalid text visible for correction. Selecting a valid row highlights the same point in the
//! central 3D viewport and the docked flat chart; graphical grid-point picks focus that stable row.
use tetraplot::prelude::*;
use tetraplot::{
    BreakLineKind, BreakLineStyle, ChartEmbedding, CompositionEntryMode, CompositionGrid,
    EmbeddedTernaryChart, GridCoordinateSpace, IrregularCompositionGrid, PlanarSection,
    RegularCompositionGrid, SurfacePatch, SurfaceTopology, SurfaceVertexIndex, TetraplotDocument,
    TetraplotEditor, TriangulatedEmbedding,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tolerance = Tolerance::default();
    let mut plot = TetraplotBuilder::new()
        .caption("tetraplot scientific editor")
        .vertex_labels(["CaO", "FeO", "FeO1.5", "SiO2"])
        .build()?;
    plot.draw_series(TetraPointSeries::new([[0.50, 0.20, 0.20, 0.10]]))?;
    let mut section = PlanarSection::constant_component(0, 0.25)?
        .name("CaO = 25%")
        .show_grid(true);
    section.chart_mut().add_points([[0.30, 0.30, 0.40]]);
    section
        .chart_mut()
        .add_line([[0.75, 0.15, 0.10], [0.20, 0.55, 0.25]], false);
    let section_id = plot.add_section(section)?;
    let topology = SurfaceTopology::new(
        [
            SurfaceVertexIndex(0),
            SurfaceVertexIndex(1),
            SurfaceVertexIndex(2),
        ],
        [
            vec![
                SurfaceVertexIndex(0),
                SurfaceVertexIndex(3),
                SurfaceVertexIndex(1),
            ],
            vec![SurfaceVertexIndex(1), SurfaceVertexIndex(2)],
            vec![SurfaceVertexIndex(2), SurfaceVertexIndex(0)],
        ],
    );
    let mut surface = TriangulatedEmbedding::new(
        vec![
            TernaryPoint::new([1.0, 0.0, 0.0])?,
            TernaryPoint::new([0.0, 1.0, 0.0])?,
            TernaryPoint::new([0.0, 0.0, 1.0])?,
            TernaryPoint::new([0.5, 0.5, 0.0])?,
        ],
        vec![
            TetraPoint::new([0.55, 0.20, 0.15, 0.10])?,
            TetraPoint::new([0.15, 0.60, 0.15, 0.10])?,
            TetraPoint::new([0.15, 0.15, 0.55, 0.15])?,
            TetraPoint::new([0.35, 0.35, 0.08, 0.22])?,
        ],
        vec![[0, 3, 2], [3, 1, 2]],
        topology,
        tolerance,
    )?;
    surface.set_patches(vec![
        SurfacePatch::named(vec![0], "Fe saturation"),
        SurfacePatch::named(vec![1], "silicate branch"),
    ])?;
    surface.add_break_line(
        vec![SurfaceVertexIndex(3), SurfaceVertexIndex(2)],
        [None, None],
        BreakLineKind::Univariant,
        BreakLineStyle::default(),
    )?;
    let mut curved =
        EmbeddedTernaryChart::new(ChartEmbedding::Triangulated(surface)).name("Piecewise liquidus");
    let mut curved_style = curved.style();
    curved_style.grid.visible = true;
    curved_style.grid.subdivisions = 5;
    curved.set_style(curved_style);
    curved
        .diagram_mut()
        .add_points([[0.65, 0.15, 0.20], [0.18, 0.62, 0.20]]);
    curved
        .diagram_mut()
        .add_line([[0.70, 0.10, 0.20], [0.10, 0.70, 0.20]], false);
    let chart_id = plot.add_embedded_chart(curved)?;
    let mut document = TetraplotDocument::new(plot);
    let mut regular = RegularCompositionGrid::local_ternary(
        "Curved-chart lattice",
        GridCoordinateSpace::EmbeddedChart(chart_id),
        tetraplot::RegularTernaryGridDefinition {
            subdivisions: 5,
            ..Default::default()
        },
    )?;
    let temperature = regular.add_scalar_field("Temperature");
    regular.set_scalar_units(temperature, Some("K".to_owned()))?;
    let viscosity = regular.add_scalar_field("Viscosity");
    regular.set_scalar_units(viscosity, Some("Pa·s".to_owned()))?;
    let sample_rows: Vec<_> = regular
        .points()
        .iter()
        .take(4)
        .map(|point| point.row_id)
        .collect();
    for (offset, row) in sample_rows.into_iter().enumerate() {
        regular.set_scalar(row, temperature, Some(1500.0 + offset as f64 * 35.0))?;
        regular.set_scalar(row, viscosity, Some(2.4 - offset as f64 * 0.25))?;
    }
    document.add_grid(CompositionGrid::Regular(regular));
    let mut irregular = IrregularCompositionGrid::local_ternary(
        "Section measurements",
        GridCoordinateSpace::Section(section_id),
    )?;
    irregular.set_entry_mode(CompositionEntryMode::DependentComponent(Component::C))?;
    let activity = irregular.add_scalar_field("Activity");
    let row = irregular.append_raw_row(["0.20", "0.55"], tolerance);
    irregular.set_scalar(row, activity, Some(0.76))?;
    document.add_grid(CompositionGrid::Irregular(irregular));
    TetraplotEditor::with_document(document).run()?;
    Ok(())
}
