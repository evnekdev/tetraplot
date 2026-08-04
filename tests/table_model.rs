use tetraplot::{
    ClipboardTable, CompositionEntryMode, CompositionGrid, DataTableState, EditorCommand,
    EditorState, GridCellAddress, GridColumnKind, IrregularCompositionGrid, RegularCompositionGrid,
    RegularTetraGridDefinition, Tetraplot, TetraplotDocument, Tolerance,
};

fn regular() -> CompositionGrid {
    let mut grid = RegularCompositionGrid::tetrahedral(
        "regular",
        RegularTetraGridDefinition {
            subdivisions: 2,
            ..Default::default()
        },
    )
    .unwrap();
    grid.add_scalar_field("Temperature");
    grid.add_scalar_field("Pressure");
    CompositionGrid::Regular(grid)
}

#[test]
fn table_navigation_range_sort_fill_and_scroll_keep_stable_ids() {
    let mut document = TetraplotDocument::new(Tetraplot::default());
    let grid_id = document.add_grid(regular());
    let grid = document.grid(grid_id).unwrap();
    let rows = grid.row_ids();
    let columns = grid.columns();
    let mut table = DataTableState::new(grid_id, rows.clone(), columns.clone());
    let first = GridCellAddress {
        row: rows[0],
        column: columns[1].id,
    };
    assert!(table.activate(first, false));
    assert!(table.move_active(1, 1, true));
    assert_eq!(table.selected_cells().len(), 4);
    assert_eq!(table.active_cell.unwrap().row, rows[1]);

    let mut sorted = rows.clone();
    sorted.reverse();
    table.replace_row_order(sorted.clone());
    assert_eq!(table.active_cell.unwrap().row, rows[1]);
    assert_eq!(table.scroll_to_row(rows[1]), Some(sorted.len() - 2));
    assert_eq!(table.first_visible_row, sorted.len() - 2);

    let copied = grid.selected_table(&table, false).unwrap();
    let filled = table.fill_down(&copied);
    assert_eq!(filled.rows.len(), 2);
    assert_eq!(filled.rows[0], filled.rows[1]);

    assert!(table.select_all());
    assert_eq!(table.selected_cells().len(), rows.len() * columns.len());
}

#[test]
fn scalar_text_edit_retains_invalid_text_and_revisions_are_independent() {
    let mut document = TetraplotDocument::new(Tetraplot::default());
    let grid_id = document.add_grid(regular());
    let field = document.grid(grid_id).unwrap().fields()[0].id();
    let row = document.grid(grid_id).unwrap().row_ids()[0];
    let scalar_column = document
        .grid(grid_id)
        .unwrap()
        .columns()
        .into_iter()
        .find(|column| matches!(column.kind, GridColumnKind::Scalar { field: id } if id == field))
        .unwrap();
    let address = GridCellAddress {
        row,
        column: scalar_column.id,
    };
    let mut state = EditorState::default();
    EditorCommand::OpenDataGrid(grid_id)
        .execute(&mut document, &mut state)
        .unwrap();
    let coordinate_revision = document.grid(grid_id).unwrap().coordinate_revision();
    let scalar_revision = document.grid(grid_id).unwrap().scalar_revision();

    EditorCommand::EditGridCellText {
        grid: grid_id,
        address,
        text: "abc".to_owned(),
    }
    .execute(&mut document, &mut state)
    .unwrap();
    assert_eq!(state.tables[&grid_id].invalid_text(address), Some("abc"));
    assert_eq!(
        document.grid(grid_id).unwrap().scalar_values(row).unwrap()[0],
        None
    );
    assert_eq!(
        document.grid(grid_id).unwrap().coordinate_revision(),
        coordinate_revision
    );
    assert_eq!(
        document.grid(grid_id).unwrap().scalar_revision(),
        scalar_revision
    );

    EditorCommand::EditGridCellText {
        grid: grid_id,
        address,
        text: "1600".to_owned(),
    }
    .execute(&mut document, &mut state)
    .unwrap();
    assert_eq!(state.tables[&grid_id].invalid_text(address), None);
    assert_eq!(
        document.grid(grid_id).unwrap().scalar_values(row).unwrap()[0],
        Some(1600.0)
    );
    assert_eq!(
        document.grid(grid_id).unwrap().coordinate_revision(),
        coordinate_revision
    );
    assert!(document.grid(grid_id).unwrap().scalar_revision() > scalar_revision);
}

#[test]
fn rectangular_scalar_paste_and_clear_use_stable_addresses() {
    let mut document = TetraplotDocument::new(Tetraplot::default());
    let grid_id = document.add_grid(regular());
    let grid = document.grid(grid_id).unwrap();
    let rows = grid.row_ids();
    let scalar_columns: Vec<_> = grid
        .columns()
        .into_iter()
        .filter(|column| matches!(column.kind, GridColumnKind::Scalar { .. }))
        .collect();
    let anchor = GridCellAddress {
        row: rows[0],
        column: scalar_columns[0].id,
    };
    let mut state = EditorState::default();
    EditorCommand::OpenDataGrid(grid_id)
        .execute(&mut document, &mut state)
        .unwrap();
    let summary = EditorCommand::PasteGridCells {
        grid: grid_id,
        anchor,
        clipboard: ClipboardTable {
            headers: None,
            rows: vec![
                vec!["1".to_owned(), "2".to_owned()],
                vec!["3".to_owned(), "bad".to_owned()],
            ],
        },
        transpose: false,
    }
    .execute(&mut document, &mut state)
    .unwrap()
    .unwrap();
    assert_eq!(summary.updated, 3);
    assert_eq!(summary.invalid, 1);
    assert_eq!(
        document
            .grid(grid_id)
            .unwrap()
            .scalar_values(rows[1])
            .unwrap(),
        vec![Some(3.0), None]
    );
    let invalid = GridCellAddress {
        row: rows[1],
        column: scalar_columns[1].id,
    };
    assert_eq!(state.tables[&grid_id].invalid_text(invalid), Some("bad"));

    EditorCommand::ClearGridCells {
        grid: grid_id,
        cells: vec![anchor],
    }
    .execute(&mut document, &mut state)
    .unwrap();
    assert_eq!(
        document
            .grid(grid_id)
            .unwrap()
            .scalar_values(rows[0])
            .unwrap()[0],
        None
    );
}

#[test]
fn irregular_commands_edit_components_and_preserve_row_identity() {
    let mut grid = IrregularCompositionGrid::tetrahedral("irregular");
    grid.set_entry_mode(CompositionEntryMode::DependentComponent(
        tetraplot::Component::D,
    ))
    .unwrap();
    let first = grid.append_raw_row(["0.2", "0.3", "0.1"], Tolerance::default());
    let second = grid.append_raw_row(["0.1", "0.2", "0.3"], Tolerance::default());
    let mut document = TetraplotDocument::new(Tetraplot::default());
    let grid_id = document.add_grid(CompositionGrid::Irregular(grid));
    let mut state = EditorState::default();

    EditorCommand::EditGridComponent {
        grid: grid_id,
        row: first,
        component: 0,
        text: "0.25".to_owned(),
    }
    .execute(&mut document, &mut state)
    .unwrap();
    let coordinate = document
        .grid(grid_id)
        .unwrap()
        .row_coordinate(first)
        .unwrap()
        .unwrap()
        .as_values();
    assert!((coordinate[3] - 0.35).abs() < 1.0e-12);

    EditorCommand::InsertGridRow {
        grid: grid_id,
        before: Some(second),
    }
    .execute(&mut document, &mut state)
    .unwrap();
    let inserted = match state.selection {
        tetraplot::Selection::GridRow { row, .. } => row,
        _ => panic!("inserted row was not selected"),
    };
    assert_ne!(inserted, first);
    assert_ne!(inserted, second);
    EditorCommand::DeleteGridRows {
        grid: grid_id,
        rows: vec![inserted],
    }
    .execute(&mut document, &mut state)
    .unwrap();
    assert_eq!(
        document.grid(grid_id).unwrap().row_ids(),
        vec![first, second]
    );

    EditorCommand::EditGridComponent {
        grid: grid_id,
        row: first,
        component: 2,
        text: "bad".to_owned(),
    }
    .execute(&mut document, &mut state)
    .unwrap();
    assert!(
        document
            .grid(grid_id)
            .unwrap()
            .row_coordinate(first)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        document
            .grid(grid_id)
            .unwrap()
            .component_text(first, 2)
            .unwrap(),
        "bad"
    );
}
