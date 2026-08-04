//! Regression tests for the renderer-independent editor table and commands.

use tetraplot::{
    CellRange, ClipboardTable, Component, CompositionEntryMode, CompositionGrid, DataTableState,
    DuplicateCompositionPolicy, EditorCommand, EditorState, GridCellAddress, GridColumnKind,
    IrregularCompositionGrid, ParsedCellValue, Selection, TableMove, TableSort, Tetraplot,
    TetraplotDocument, Tolerance, grid_columns,
};

fn irregular_with_two_rows() -> CompositionGrid {
    let mut grid = IrregularCompositionGrid::tetrahedral("measurements");
    grid.append_raw_row(["0.2", "0.3", "0.1", "0.4"], Tolerance::default());
    grid.append_raw_row(["0.4", "0.2", "0.1", "0.3"], Tolerance::default());
    CompositionGrid::Irregular(grid)
}

#[test]
fn clipboard_preserves_empty_and_ragged_cells() {
    let table = ClipboardTable::parse_tsv("A\tB\tC\n0.2\t\t0.8\n0.4\t0.6\n", true);
    assert_eq!(
        table.headers.as_deref(),
        Some(&["A".into(), "B".into(), "C".into()][..])
    );
    assert_eq!(table.rows[0], ["0.2", "", "0.8"]);
    assert_eq!(table.width(), 3);
    assert_eq!(table.ragged_rows(), vec![1]);
}

#[test]
fn invalid_scalar_text_is_retained_until_corrected() {
    let mut grid = irregular_with_two_rows();
    let field = grid.add_scalar_field("Temperature");
    let row = grid.row_ids()[0];

    let parsed = grid.set_scalar_text(row, field, "not-a-number").unwrap();
    assert_eq!(
        parsed,
        ParsedCellValue::InvalidText("not-a-number".to_owned())
    );
    assert_eq!(grid.scalar_text(row, field).unwrap(), "not-a-number");
    assert_eq!(grid.scalar_values(row).unwrap(), vec![None]);
    assert!(grid.validation_summary().invalid >= 1);

    assert_eq!(
        grid.set_scalar_text(row, field, "1675.5").unwrap(),
        ParsedCellValue::ValidNumber(1675.5)
    );
    assert_eq!(grid.scalar_values(row).unwrap(), vec![Some(1675.5)]);
}

#[test]
fn header_aware_rectangular_paste_maps_fields_and_reports_unknown_headers() {
    let mut grid = irregular_with_two_rows();
    let temperature = grid.add_scalar_field("Temperature");
    let pressure = grid.add_scalar_field("Pressure");
    let mut document = TetraplotDocument::new(Tetraplot::default());
    let grid_id = document.add_grid(grid);
    let rows = document.grid(grid_id).unwrap().row_ids();
    let columns = grid_columns(document.grid(grid_id).unwrap());
    let temperature_column = columns
        .iter()
        .find(|column| {
            matches!(column.kind, GridColumnKind::Scalar { field } if field == temperature)
        })
        .unwrap();

    let mut table = DataTableState::new(grid_id);
    table.activate(
        GridCellAddress {
            row: rows[0],
            column: temperature_column.id,
        },
        false,
    );
    let clipboard = ClipboardTable::parse_tsv(
        "Pressure\tUnknown property\tTemperature\n12.5\tignored\tbad\n13.0\tignored\t1700",
        true,
    );
    let summary = table
        .paste(
            document.grid_mut(grid_id).unwrap(),
            &clipboard,
            Tolerance::default(),
            false,
        )
        .unwrap();
    let grid = document.grid(grid_id).unwrap();

    assert_eq!(grid.scalar_values(rows[0]).unwrap()[1], Some(12.5));
    assert_eq!(grid.scalar_text(rows[0], temperature).unwrap(), "bad");
    assert_eq!(grid.scalar_values(rows[1]).unwrap()[0], Some(1700.0));
    assert_eq!(grid.scalar_values(rows[1]).unwrap()[1], Some(13.0));
    assert!(
        summary
            .warnings
            .iter()
            .any(|warning| warning.contains("unrecognized header 'Unknown property'"))
    );
    assert!(summary.invalid >= 1);
    assert_eq!(pressure, grid.fields()[1].id());
}

#[test]
fn dependent_component_is_read_only_and_invalid_component_text_survives() {
    let mut source = IrregularCompositionGrid::tetrahedral("dependent");
    source
        .set_entry_mode(CompositionEntryMode::DependentComponent(Component::D))
        .unwrap();
    let row = source.append_raw_row(["0.2", "0.3", "0.1"], Tolerance::default());
    let mut grid = CompositionGrid::Irregular(source);
    let columns = grid_columns(&grid);
    let dependent = columns
        .iter()
        .find(|column| {
            matches!(
                column.kind,
                GridColumnKind::Component {
                    component: 3,
                    dependent: true
                }
            )
        })
        .unwrap();
    assert!(!dependent.editable);

    grid.set_component_text(row, 1, "bad", Tolerance::default())
        .unwrap();
    assert_eq!(grid.component_text(row, 1, 6).unwrap(), "bad");
    assert!(grid.row_validation(row).unwrap().iter().any(|issue| {
        matches!(
            issue,
            tetraplot::GridValidationIssue::InvalidNumber { column: 1, .. }
        )
    }));
}

#[test]
fn fill_down_command_uses_stable_table_selection() {
    let mut grid = irregular_with_two_rows();
    let field = grid.add_scalar_field("Temperature");
    let rows = grid.row_ids();
    grid.set_scalar(rows[0], field, Some(1550.0)).unwrap();

    let mut document = TetraplotDocument::new(Tetraplot::default());
    let grid_id = document.add_grid(grid);
    let columns = grid_columns(document.grid(grid_id).unwrap());
    let scalar_column = columns
        .iter()
        .find(|column| matches!(column.kind, GridColumnKind::Scalar { field: id } if id == field))
        .unwrap()
        .id;
    let mut state = EditorState::default();
    EditorCommand::OpenDataGrid(grid_id)
        .execute(&mut document, &mut state)
        .unwrap();
    let table = state.table_state_mut(grid_id);
    table.active_cell = Some(GridCellAddress {
        row: rows[0],
        column: scalar_column,
    });
    table.selection = Some(CellRange {
        anchor: GridCellAddress {
            row: rows[0],
            column: scalar_column,
        },
        focus: GridCellAddress {
            row: rows[1],
            column: scalar_column,
        },
    });

    EditorCommand::FillDownGrid { grid: grid_id }
        .execute(&mut document, &mut state)
        .unwrap();

    assert_eq!(
        document
            .grid(grid_id)
            .unwrap()
            .scalar_values(rows[1])
            .unwrap(),
        vec![Some(1550.0)]
    );
    assert_eq!(state.selection, Selection::Grid(grid_id));
}

#[test]
fn duplicate_reject_policy_keeps_row_identity_but_invalidates_coordinate() {
    let mut grid = IrregularCompositionGrid::tetrahedral("duplicates");
    let first = grid.append_raw_row(["0.2", "0.3", "0.1", "0.4"], Tolerance::default());
    let second = grid.append_raw_row(["0.2", "0.3", "0.1", "0.4"], Tolerance::default());
    grid.set_duplicate_policy(DuplicateCompositionPolicy::Reject);

    assert_ne!(first, second);
    let rejected = grid.rows().iter().find(|row| row.id == second).unwrap();
    assert!(rejected.coordinate.is_none());
    assert!(rejected.validation.iter().any(|issue| {
        matches!(
            issue,
            tetraplot::GridValidationIssue::DuplicateCompositionRejected { other }
                if *other == first
        )
    }));
}

#[test]
fn table_navigation_sorting_and_scroll_keep_stable_row_ids() {
    let mut grid = irregular_with_two_rows();
    let field = grid.add_scalar_field("Temperature");
    let source_rows = grid.row_ids();
    grid.set_scalar(source_rows[0], field, Some(1500.0))
        .unwrap();
    grid.set_scalar(source_rows[1], field, Some(1700.0))
        .unwrap();
    let mut document = TetraplotDocument::new(Tetraplot::default());
    let grid_id = document.add_grid(grid);
    let grid = document.grid(grid_id).unwrap();
    let columns = grid_columns(grid);
    let scalar = columns
        .iter()
        .find(|column| matches!(column.kind, GridColumnKind::Scalar { field: id } if id == field))
        .unwrap()
        .id;
    let mut table = DataTableState::new(grid_id);
    table.set_sort(TableSort::Scalar {
        field,
        descending: true,
    });
    let displayed = table.displayed_rows(grid);
    assert_eq!(displayed, vec![source_rows[1], source_rows[0]]);

    table.activate(
        GridCellAddress {
            row: displayed[0],
            column: scalar,
        },
        false,
    );
    table.move_active(&displayed, &columns, TableMove::Down, true);
    assert_eq!(table.active_cell.unwrap().row, displayed[1]);
    assert_eq!(table.selection.unwrap().anchor.row, displayed[0]);
    assert_eq!(table.selection.unwrap().focus.row, displayed[1]);

    assert_eq!(table.scroll_to_row(&displayed, source_rows[0]), Some(1));
    assert_eq!(table.take_scroll_request(), Some(1));
    assert_eq!(table.take_scroll_request(), None);
}

#[test]
fn all_component_paste_validates_dependent_component_consistency() {
    let mut source = IrregularCompositionGrid::tetrahedral("dependent paste");
    source
        .set_entry_mode(CompositionEntryMode::DependentComponent(Component::D))
        .unwrap();
    let row = source.append_raw_row(["0.2", "0.3", "0.1"], Tolerance::default());
    let mut document = TetraplotDocument::new(Tetraplot::default());
    let grid_id = document.add_grid(CompositionGrid::Irregular(source));
    let columns = grid_columns(document.grid(grid_id).unwrap());
    let mut table = DataTableState::new(grid_id);
    table.activate(
        GridCellAddress {
            row,
            column: columns
                .iter()
                .find(|column| {
                    matches!(column.kind, GridColumnKind::Component { component: 0, .. })
                })
                .unwrap()
                .id,
        },
        false,
    );

    let inconsistent = ClipboardTable::parse_tsv("A\tB\tC\tD\n0.2\t0.3\t0.1\t0.9", true);
    let summary = table
        .paste(
            document.grid_mut(grid_id).unwrap(),
            &inconsistent,
            Tolerance::default(),
            false,
        )
        .unwrap();
    let grid = document.grid(grid_id).unwrap();
    assert!(grid.row_coordinate(row).unwrap().is_none());
    assert!(
        grid.row_validation(row)
            .unwrap()
            .iter()
            .any(|issue| { matches!(issue, tetraplot::GridValidationIssue::SumMismatch { .. }) })
    );
    assert!(summary.invalid >= 1);

    let consistent = ClipboardTable::parse_tsv("A\tB\tC\tD\n0.2\t0.3\t0.1\t0.4", true);
    table
        .paste(
            document.grid_mut(grid_id).unwrap(),
            &consistent,
            Tolerance::default(),
            false,
        )
        .unwrap();
    assert!(
        document
            .grid(grid_id)
            .unwrap()
            .row_coordinate(row)
            .unwrap()
            .is_some()
    );
}
