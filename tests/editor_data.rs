use tetraplot::{
    ChartEmbedding, CompositionEntryMode, CompositionGrid, DuplicateCompositionPolicy,
    EmbeddedTernaryChart, GridCoordinateSpace, GridExportOptions, IrregularCompositionGrid,
    PlanarEmbedding, RegularCompositionGrid, RegularGridDefinition, RegularTetraGridDefinition,
    TetraPoint, Tetraplot, TetraplotDocument, Tolerance,
};

#[test]
fn regular_grid_has_stable_rows_and_preserves_matching_scalars() {
    let mut grid = RegularCompositionGrid::tetrahedral(
        "regular",
        RegularTetraGridDefinition {
            subdivisions: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(grid.points().len(), 10);
    let field = grid.add_scalar_field("Temperature");
    let row = grid.points()[0].row_id;
    grid.set_scalar(row, field, Some(1600.0)).unwrap();
    grid.set_definition(
        RegularGridDefinition::Tetrahedral(RegularTetraGridDefinition {
            subdivisions: 3,
            ..Default::default()
        }),
        tetraplot::GridRedefinitionPolicy::PreserveMatchingCoordinates,
        Tolerance::default(),
    )
    .unwrap();
    let matching = grid
        .points()
        .iter()
        .find(|point| point.logical_index.0 == [0, 0, 0, 3])
        .unwrap();
    assert_eq!(
        grid.field(field)
            .unwrap()
            .value(grid.row_index(matching.row_id).unwrap()),
        Some(Some(1600.0))
    );
    let table = CompositionGrid::Regular(grid)
        .table(&GridExportOptions::default())
        .unwrap();
    assert_eq!(table.headers.unwrap()[0], "A");
}

#[test]
fn irregular_grid_keeps_invalid_rows_and_calculates_dependent_component() {
    let mut grid = IrregularCompositionGrid::tetrahedral("measurements");
    grid.set_entry_mode(CompositionEntryMode::DependentComponent(
        tetraplot::Component::D,
    ))
    .unwrap();
    let valid = grid.append_raw_row(["0.20", "0.30", "0.10"], Tolerance::default());
    assert_eq!(
        grid.rows()[0].coordinate.unwrap().as_values(),
        [0.2, 0.3, 0.1, 0.4]
    );
    let full = grid.append_raw_row(["0.20", "0.30", "0.10", "0.40"], Tolerance::default());
    assert_eq!(
        grid.rows()[1].coordinate.unwrap().as_values(),
        [0.2, 0.3, 0.1, 0.4]
    );
    let invalid = grid.append_raw_row(["0.8", "0.5", "bad"], Tolerance::default());
    assert!(
        grid.rows()
            .iter()
            .find(|row| row.id == invalid)
            .unwrap()
            .coordinate
            .is_none()
    );
    assert_eq!(grid.rows()[0].id, valid);
    grid.set_duplicate_policy(DuplicateCompositionPolicy::Warn);
    assert!(
        grid.rows()
            .iter()
            .find(|row| row.id == full)
            .unwrap()
            .validation
            .iter()
            .any(|issue| matches!(
                issue,
                tetraplot::GridValidationIssue::DuplicateComposition { .. }
            ))
    );
}

#[test]
fn clipboard_tsv_paste_handles_headers_and_scalar_fields() {
    let mut irregular = IrregularCompositionGrid::tetrahedral("import");
    let temperature = irregular.add_scalar_field("Temperature");
    let table = tetraplot::ClipboardTable::parse_tsv(
        "A\tB\tC\tD\tTemperature\n0.2\t0.3\t0.1\t0.4\t1600\n0.4\t0.2\t0.1\t0.3\t1710",
        true,
    );
    let mut grid = CompositionGrid::Irregular(irregular);
    let report = grid.append_clipboard(&table, Tolerance::default()).unwrap();
    assert_eq!(report.inserted, 2);
    assert_eq!(
        grid.scalar_values(grid.row_ids()[1]).unwrap(),
        vec![Some(1710.0)]
    );
    assert_eq!(grid.fields()[0].id(), temperature);
}

#[test]
fn document_maps_local_grid_rows_without_storing_world_copies() {
    let embedding = ChartEmbedding::Planar(
        PlanarEmbedding::new(
            [
                TetraPoint::new([0.5, 0.5, 0.0, 0.0]).unwrap(),
                TetraPoint::new([0.5, 0.0, 0.5, 0.0]).unwrap(),
                TetraPoint::new([0.5, 0.0, 0.0, 0.5]).unwrap(),
            ],
            Tolerance::default(),
        )
        .unwrap(),
    );
    let mut plot = Tetraplot::default();
    let chart = plot
        .add_embedded_chart(EmbeddedTernaryChart::new(embedding))
        .unwrap();
    let mut document = TetraplotDocument::new(plot);
    let grid = RegularCompositionGrid::local_ternary(
        "local",
        GridCoordinateSpace::EmbeddedChart(chart),
        tetraplot::RegularTernaryGridDefinition {
            subdivisions: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let id = document.add_grid(CompositionGrid::Regular(grid));
    let prepared = document.prepared_grid_points(id).unwrap();
    assert_eq!(prepared.len(), 6);
    assert!(prepared.iter().all(|point| point.local.is_some()));
}

#[test]
fn tsv_round_trip_preserves_scalar_headers_units_and_empty_cells() {
    let mut irregular = IrregularCompositionGrid::tetrahedral("round trip");
    let temperature = irregular.add_scalar_field("Temperature");
    irregular
        .field_mut(temperature)
        .unwrap()
        .set_units(Some("C"));
    let row = irregular.append_raw_row(["0.2", "0.3", "0.1", "0.4"], Tolerance::default());
    irregular
        .set_scalar(row, temperature, Some(1234.5))
        .unwrap();
    let grid = CompositionGrid::Irregular(irregular);
    let mut output = Vec::new();
    grid.write_tsv(&mut output, &GridExportOptions::default())
        .unwrap();
    let text = String::from_utf8(output.clone()).unwrap();
    assert!(text.starts_with("A\tB\tC\tD\tTemperature [C]"));

    let imported = CompositionGrid::read_tsv(
        output.as_slice(),
        GridCoordinateSpace::Tetrahedral,
        true,
        Tolerance::default(),
    )
    .unwrap();
    assert_eq!(imported.fields().len(), 1);
    assert_eq!(imported.fields()[0].name(), "Temperature");
    assert_eq!(imported.fields()[0].units(), Some("C"));
    assert_eq!(
        imported.scalar_values(imported.row_ids()[0]).unwrap(),
        vec![Some(1234.5)]
    );

    let empty = tetraplot::ClipboardTable::parse_tsv("1\t\t3\n4\t5", false);
    assert_eq!(empty.rows[0], vec!["1", "", "3"]);
    assert_eq!(empty.shape_warnings().len(), 1);
    assert_eq!(empty.transpose().rows.len(), 3);
}

#[test]
fn duplicate_reject_marks_the_later_row_invalid() {
    let mut grid = IrregularCompositionGrid::tetrahedral("duplicates");
    grid.set_duplicate_policy(DuplicateCompositionPolicy::Reject);
    let first = grid.append_raw_row(["0.25", "0.25", "0.25", "0.25"], Tolerance::default());
    let second = grid.append_raw_row(["0.25", "0.25", "0.25", "0.25"], Tolerance::default());
    assert!(
        grid.rows()
            .iter()
            .find(|row| row.id == first)
            .unwrap()
            .is_valid()
    );
    let rejected = grid.rows().iter().find(|row| row.id == second).unwrap();
    assert!(rejected.coordinate.is_none());
    assert!(rejected.validation.iter().any(|issue| matches!(
        issue,
        tetraplot::GridValidationIssue::DuplicateCompositionRejected { other }
            if *other == first
    )));
}
