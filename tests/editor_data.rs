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
    irregular
        .set_scalar_units(temperature, Some("K".to_owned()))
        .unwrap();
    let table = tetraplot::ClipboardTable::parse_tsv(
        "A\tB\tC\tD\tTemperature [K]\tUnknown\n0.2\t0.3\t0.1\t0.4\t1600\tignored\n0.4\t0.2\t0.1\t0.3\t1710\tignored",
        true,
    );
    let mut grid = CompositionGrid::Irregular(irregular);
    let report = grid.append_clipboard(&table, Tolerance::default()).unwrap();
    assert_eq!(report.inserted, 2);
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("unrecognized header 'Unknown'"))
    );
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
fn scalar_edits_do_not_change_coordinate_revision() {
    let mut grid = CompositionGrid::Irregular(IrregularCompositionGrid::tetrahedral("revisions"));
    let row = grid.append_empty_row(Tolerance::default()).unwrap();
    grid.set_component_text(row, 0, "0.2", Tolerance::default())
        .unwrap();
    grid.set_component_text(row, 1, "0.3", Tolerance::default())
        .unwrap();
    grid.set_component_text(row, 2, "0.1", Tolerance::default())
        .unwrap();
    grid.set_component_text(row, 3, "0.4", Tolerance::default())
        .unwrap();
    let field = grid.add_scalar_field("Temperature");
    let coordinate_revision = grid.coordinate_revision();
    let scalar_revision = grid.scalar_revision();

    grid.set_scalar_text(row, field, "nan").unwrap();

    assert_eq!(grid.coordinate_revision(), coordinate_revision);
    assert!(grid.scalar_revision() > scalar_revision);
    assert_eq!(grid.scalar_text(row, field).unwrap(), "nan");
    assert_eq!(grid.scalar_values(row).unwrap(), vec![None]);
}

#[test]
fn flat_revision_ignores_camera_but_tracks_attached_grid_changes() {
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
    let mut grid = RegularCompositionGrid::local_ternary(
        "local",
        GridCoordinateSpace::EmbeddedChart(chart),
        tetraplot::RegularTernaryGridDefinition {
            subdivisions: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let field = grid.add_scalar_field("Temperature");
    let row = grid.points()[0].row_id;
    let grid_id = document.add_grid(CompositionGrid::Regular(grid));
    let target = tetraplot::FlatViewTarget::EmbeddedChart(chart);
    let initial = document.flat_revision(target);

    let camera = document.plot().camera();
    document.plot_mut().set_camera(camera);
    assert_eq!(document.flat_revision(target), initial);

    document
        .grid_mut(grid_id)
        .unwrap()
        .set_scalar(row, field, Some(1625.0))
        .unwrap();
    assert_ne!(document.flat_revision(target), initial);
}

#[test]
fn removed_coordinate_target_is_reported_instead_of_silently_mapping_nothing() {
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
        "orphan",
        GridCoordinateSpace::EmbeddedChart(chart),
        tetraplot::RegularTernaryGridDefinition {
            subdivisions: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let grid_id = document.add_grid(CompositionGrid::Regular(grid));
    document.remove_embedded_chart(chart).unwrap();

    assert_eq!(
        document.prepared_grid_points(grid_id).unwrap_err(),
        tetraplot::GridError::CoordinateSpaceMismatch
    );
}
