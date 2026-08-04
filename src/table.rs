//! GUI-independent spreadsheet-style composition-grid interaction.

use std::cmp::Ordering;

use crate::{
    ClipboardTable, CompositionEntryMode, CompositionGrid, CompositionGridId, GridColumnId,
    GridError, GridRedefinitionPolicy, GridRowId, GridValidationIssue, ParsedCellValue,
    PasteSummary, ScalarFieldId, Tolerance,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GridColumnKind {
    RowId,
    Component { component: usize, dependent: bool },
    Scalar { field: ScalarFieldId },
    Validation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridColumn {
    pub id: GridColumnId,
    pub label: String,
    pub kind: GridColumnKind,
    pub editable: bool,
}

impl GridColumn {
    pub const fn row_id() -> GridColumnId {
        GridColumnId::new(0)
    }

    pub const fn component(component: usize) -> GridColumnId {
        GridColumnId::new(1 + component as u64)
    }

    pub const fn scalar(field: ScalarFieldId) -> GridColumnId {
        GridColumnId::new(0x1_0000_0000 + field.get())
    }

    pub const fn validation() -> GridColumnId {
        GridColumnId::new(u64::MAX)
    }
}

pub fn grid_columns(grid: &CompositionGrid) -> Vec<GridColumn> {
    let dimension = if grid.coordinate_space().is_local() {
        3
    } else {
        4
    };
    let component_labels = if dimension == 3 {
        ["u", "v", "w", ""]
    } else {
        ["A", "B", "C", "D"]
    };
    let dependent = match grid.entry_mode() {
        CompositionEntryMode::AllComponents => None,
        CompositionEntryMode::DependentComponent(component) => Some(component.index()),
    };
    let mut columns = vec![GridColumn {
        id: GridColumn::row_id(),
        label: "Row".to_owned(),
        kind: GridColumnKind::RowId,
        editable: false,
    }];
    columns.extend((0..dimension).map(|component| {
        let dependent = dependent == Some(component);
        GridColumn {
            id: GridColumn::component(component),
            label: if dependent {
                format!("{} (dependent)", component_labels[component])
            } else {
                component_labels[component].to_owned()
            },
            kind: GridColumnKind::Component {
                component,
                dependent,
            },
            editable: !grid.is_regular() && !dependent,
        }
    }));
    columns.extend(grid.fields().iter().map(|field| GridColumn {
        id: GridColumn::scalar(field.id()),
        label: field.units().map_or_else(
            || field.name().to_owned(),
            |units| format!("{} [{}]", field.name(), units),
        ),
        kind: GridColumnKind::Scalar { field: field.id() },
        editable: true,
    }));
    columns.push(GridColumn {
        id: GridColumn::validation(),
        label: "Validation".to_owned(),
        kind: GridColumnKind::Validation,
        editable: false,
    });
    columns
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GridCellAddress {
    pub row: GridRowId,
    pub column: GridColumnId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellRange {
    pub anchor: GridCellAddress,
    pub focus: GridCellAddress,
}

impl CellRange {
    pub const fn single(cell: GridCellAddress) -> Self {
        Self {
            anchor: cell,
            focus: cell,
        }
    }

    fn bounds(
        self,
        rows: &[GridRowId],
        columns: &[GridColumn],
    ) -> Option<(usize, usize, usize, usize)> {
        let anchor_row = rows.iter().position(|row| *row == self.anchor.row)?;
        let focus_row = rows.iter().position(|row| *row == self.focus.row)?;
        let anchor_column = columns
            .iter()
            .position(|column| column.id == self.anchor.column)?;
        let focus_column = columns
            .iter()
            .position(|column| column.id == self.focus.column)?;
        Some((
            anchor_row.min(focus_row),
            anchor_row.max(focus_row),
            anchor_column.min(focus_column),
            anchor_column.max(focus_column),
        ))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CellEditBuffer {
    pub address: GridCellAddress,
    pub text: String,
    pub parsed: ParsedCellValue,
}

impl CellEditBuffer {
    pub fn new(address: GridCellAddress, text: impl Into<String>) -> Self {
        let text = text.into();
        let parsed = parse_cell_text(&text);
        Self {
            address,
            text,
            parsed,
        }
    }

    pub fn update(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.parsed = parse_cell_text(&self.text);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableMove {
    Left,
    Right,
    Up,
    Down,
    FirstColumn,
    LastColumn,
    FirstRow,
    LastRow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum TableSort {
    #[default]
    Source,
    RowId {
        descending: bool,
    },
    Scalar {
        field: ScalarFieldId,
        descending: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataTableState {
    pub grid: CompositionGridId,
    pub active_cell: Option<GridCellAddress>,
    pub selection: Option<CellRange>,
    pub edit_buffer: Option<CellEditBuffer>,
    pub first_visible_row: usize,
    scroll_request: Option<usize>,
    pub column_widths: Vec<f32>,
    pub sort: TableSort,
    pub redefinition_policy: GridRedefinitionPolicy,
    revision: u64,
}

impl DataTableState {
    pub fn new(grid: CompositionGridId) -> Self {
        Self {
            grid,
            active_cell: None,
            selection: None,
            edit_buffer: None,
            first_visible_row: 0,
            scroll_request: None,
            column_widths: Vec::new(),
            sort: TableSort::Source,
            redefinition_policy: GridRedefinitionPolicy::PreserveMatchingCoordinates,
            revision: 0,
        }
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn displayed_rows(&self, grid: &CompositionGrid) -> Vec<GridRowId> {
        let mut rows = grid.row_ids();
        match self.sort {
            TableSort::Source => {}
            TableSort::RowId { descending } => {
                rows.sort_by_key(|row| row.get());
                if descending {
                    rows.reverse();
                }
            }
            TableSort::Scalar { field, descending } => {
                rows.sort_by(|left, right| {
                    let left_row = *left;
                    let right_row = *right;
                    let left = scalar_sort_value(grid, left_row, field);
                    let right = scalar_sort_value(grid, right_row, field);
                    let ordering = left.partial_cmp(&right).unwrap_or(Ordering::Equal);
                    let ordering = ordering.then_with(|| left_row_id(left_row, right_row));
                    if descending {
                        ordering.reverse()
                    } else {
                        ordering
                    }
                });
            }
        }
        rows
    }

    pub fn set_sort(&mut self, sort: TableSort) {
        if self.sort != sort {
            self.sort = sort;
            self.revision += 1;
        }
    }

    pub fn activate(&mut self, cell: GridCellAddress, extend: bool) {
        if extend {
            let anchor = self
                .selection
                .map_or(self.active_cell.unwrap_or(cell), |range| range.anchor);
            self.selection = Some(CellRange {
                anchor,
                focus: cell,
            });
        } else {
            self.selection = Some(CellRange::single(cell));
        }
        self.active_cell = Some(cell);
        self.revision += 1;
    }

    pub fn select_row(&mut self, row: GridRowId, columns: &[GridColumn]) {
        let Some(first) = columns.first() else {
            return;
        };
        let Some(last) = columns.last() else {
            return;
        };
        self.active_cell = Some(GridCellAddress {
            row,
            column: first.id,
        });
        self.selection = Some(CellRange {
            anchor: GridCellAddress {
                row,
                column: first.id,
            },
            focus: GridCellAddress {
                row,
                column: last.id,
            },
        });
        self.revision += 1;
    }

    pub fn move_active(
        &mut self,
        rows: &[GridRowId],
        columns: &[GridColumn],
        movement: TableMove,
        extend: bool,
    ) {
        if rows.is_empty() || columns.is_empty() {
            return;
        }
        let current = self.active_cell.unwrap_or(GridCellAddress {
            row: rows[0],
            column: columns[0].id,
        });
        let row = rows
            .iter()
            .position(|value| *value == current.row)
            .unwrap_or(0);
        let column = columns
            .iter()
            .position(|value| value.id == current.column)
            .unwrap_or(0);
        let (row, column) = match movement {
            TableMove::Left => (row, column.saturating_sub(1)),
            TableMove::Right => (row, (column + 1).min(columns.len() - 1)),
            TableMove::Up => (row.saturating_sub(1), column),
            TableMove::Down => ((row + 1).min(rows.len() - 1), column),
            TableMove::FirstColumn => (row, 0),
            TableMove::LastColumn => (row, columns.len() - 1),
            TableMove::FirstRow => (0, column),
            TableMove::LastRow => (rows.len() - 1, column),
        };
        self.activate(
            GridCellAddress {
                row: rows[row],
                column: columns[column].id,
            },
            extend,
        );
    }

    pub fn select_all(&mut self, rows: &[GridRowId], columns: &[GridColumn]) {
        if let (Some(first_row), Some(last_row), Some(first_column), Some(last_column)) =
            (rows.first(), rows.last(), columns.first(), columns.last())
        {
            let anchor = GridCellAddress {
                row: *first_row,
                column: first_column.id,
            };
            self.active_cell = Some(anchor);
            self.selection = Some(CellRange {
                anchor,
                focus: GridCellAddress {
                    row: *last_row,
                    column: last_column.id,
                },
            });
            self.revision += 1;
        }
    }

    pub fn selected_addresses(
        &self,
        rows: &[GridRowId],
        columns: &[GridColumn],
    ) -> Vec<GridCellAddress> {
        let Some(range) = self.selection else {
            return self.active_cell.into_iter().collect();
        };
        let Some((row_start, row_end, column_start, column_end)) = range.bounds(rows, columns)
        else {
            return Vec::new();
        };
        (row_start..=row_end)
            .flat_map(|row| {
                (column_start..=column_end).map(move |column| GridCellAddress {
                    row: rows[row],
                    column: columns[column].id,
                })
            })
            .collect()
    }

    pub fn scroll_to_row(&mut self, rows: &[GridRowId], row: GridRowId) -> Option<usize> {
        let index = rows.iter().position(|candidate| *candidate == row)?;
        self.first_visible_row = index;
        self.scroll_request = Some(index);
        self.revision += 1;
        Some(index)
    }

    pub fn take_scroll_request(&mut self) -> Option<usize> {
        self.scroll_request.take()
    }

    pub fn begin_edit(&mut self, text: impl Into<String>) {
        if let Some(address) = self.active_cell {
            self.edit_buffer = Some(CellEditBuffer::new(address, text));
            self.revision += 1;
        }
    }

    pub fn cancel_edit(&mut self) {
        if self.edit_buffer.take().is_some() {
            self.revision += 1;
        }
    }

    pub fn copy(
        &self,
        grid: &CompositionGrid,
        include_headers: bool,
    ) -> Result<ClipboardTable, GridError> {
        let rows = self.displayed_rows(grid);
        let columns = grid_columns(grid);
        let addresses = self.selected_addresses(&rows, &columns);
        if addresses.is_empty() {
            return Ok(ClipboardTable::default());
        }
        let Some(range) = self
            .selection
            .or_else(|| self.active_cell.map(CellRange::single))
        else {
            return Ok(ClipboardTable::default());
        };
        let (row_start, row_end, column_start, column_end) =
            range
                .bounds(&rows, &columns)
                .ok_or(GridError::InvalidComposition {
                    message: "table selection no longer exists".to_owned(),
                })?;
        let output = (row_start..=row_end)
            .map(|row| {
                (column_start..=column_end)
                    .map(|column| cell_text(grid, rows[row], &columns[column]))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ClipboardTable {
            headers: include_headers.then(|| {
                columns[column_start..=column_end]
                    .iter()
                    .map(|column| column.label.clone())
                    .collect()
            }),
            rows: output,
        })
    }

    pub fn clear(
        &mut self,
        grid: &mut CompositionGrid,
        tolerance: Tolerance,
    ) -> Result<usize, GridError> {
        let rows = self.displayed_rows(grid);
        let columns = grid_columns(grid);
        let mut cleared = 0;
        for address in self.selected_addresses(&rows, &columns) {
            let Some(column) = columns.iter().find(|column| column.id == address.column) else {
                continue;
            };
            match column.kind {
                GridColumnKind::Component {
                    component,
                    dependent: false,
                } if column.editable => {
                    grid.set_component_text(address.row, component, "", tolerance)?;
                    cleared += 1;
                }
                GridColumnKind::Scalar { field } => {
                    grid.set_scalar_text(address.row, field, "")?;
                    cleared += 1;
                }
                _ => {}
            }
        }
        Ok(cleared)
    }

    pub fn fill_down(
        &mut self,
        grid: &mut CompositionGrid,
        tolerance: Tolerance,
    ) -> Result<usize, GridError> {
        let rows = self.displayed_rows(grid);
        let columns = grid_columns(grid);
        let Some(range) = self.selection else {
            return Ok(0);
        };
        let Some((row_start, row_end, column_start, column_end)) = range.bounds(&rows, &columns)
        else {
            return Ok(0);
        };
        let mut updated = 0;
        for column in columns.iter().take(column_end + 1).skip(column_start) {
            let text = cell_text(grid, rows[row_start], column)?;
            for row in rows.iter().take(row_end + 1).skip(row_start + 1) {
                updated += usize::from(
                    set_grid_cell_text(grid, *row, column, text.clone(), tolerance)?.is_some(),
                );
            }
        }
        Ok(updated)
    }

    pub fn paste(
        &mut self,
        grid: &mut CompositionGrid,
        clipboard: &ClipboardTable,
        tolerance: Tolerance,
        transposed: bool,
    ) -> Result<PasteSummary, GridError> {
        let clipboard = if transposed {
            clipboard.transpose()
        } else {
            clipboard.clone()
        };
        let rows = self.displayed_rows(grid);
        let columns = grid_columns(grid);
        let anchor = self.active_cell.ok_or(GridError::InvalidComposition {
            message: "paste requires an active table cell".to_owned(),
        })?;
        let start_row =
            rows.iter()
                .position(|row| *row == anchor.row)
                .ok_or(GridError::UnknownGridRow {
                    id: anchor.row.get(),
                })?;
        let start_column = columns
            .iter()
            .position(|column| column.id == anchor.column)
            .ok_or(GridError::UnknownColumn {
                column: anchor.column.get() as usize,
            })?;
        let mut summary = PasteSummary {
            inserted: 0,
            updated: 0,
            valid: 0,
            invalid: 0,
            incomplete: 0,
            warnings: Vec::new(),
        };
        for row in clipboard.ragged_rows() {
            summary
                .warnings
                .push(format!("clipboard row {} is ragged", row + 1));
        }
        let mapped_columns = if let Some(headers) = &clipboard.headers {
            headers
                .iter()
                .map(|header| resolve_header(header, &columns, &mut summary.warnings))
                .collect::<Vec<_>>()
        } else {
            (0..clipboard.width())
                .map(|offset| columns.get(start_column + offset).map(|column| column.id))
                .collect()
        };
        for (row_offset, values) in clipboard.rows.iter().enumerate() {
            let Some(row) = rows.get(start_row + row_offset).copied() else {
                summary
                    .warnings
                    .push("paste extends beyond the existing rows".to_owned());
                break;
            };
            let mut component_updates = values
                .iter()
                .enumerate()
                .filter_map(|(input_column, text)| {
                    let column_id = mapped_columns.get(input_column).and_then(|value| *value)?;
                    let column = columns.iter().find(|column| column.id == column_id)?;
                    match column.kind {
                        GridColumnKind::Component { component, .. } => {
                            Some((input_column, component, text.clone()))
                        }
                        _ => None,
                    }
                })
                .collect::<Vec<_>>();
            component_updates.sort_by_key(|(_, component, _)| *component);
            component_updates.dedup_by_key(|(_, component, _)| *component);
            let dimension = if grid.coordinate_space().is_local() {
                3
            } else {
                4
            };
            let batch_input_columns = if !grid.is_regular() && component_updates.len() == dimension
            {
                let updates = component_updates
                    .iter()
                    .map(|(_, component, text)| (*component, text.clone()))
                    .collect::<Vec<_>>();
                match grid.set_component_block(row, &updates, tolerance) {
                    Ok(()) => {
                        summary.updated += updates.len();
                        if grid.row_coordinate(row)?.is_some() {
                            summary.valid += updates.len();
                        } else {
                            summary.invalid += updates.len();
                        }
                    }
                    Err(error) => summary.warnings.push(error.to_string()),
                }
                component_updates
                    .iter()
                    .map(|(input_column, _, _)| *input_column)
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };

            for (input_column, text) in values.iter().enumerate() {
                if batch_input_columns.contains(&input_column) {
                    continue;
                }
                let Some(column_id) = mapped_columns.get(input_column).and_then(|value| *value)
                else {
                    if clipboard.headers.is_none() {
                        summary.warnings.push(format!(
                            "clipboard column {} extends beyond the table",
                            input_column + 1
                        ));
                    }
                    continue;
                };
                let Some(column) = columns.iter().find(|column| column.id == column_id) else {
                    continue;
                };
                match set_grid_cell_text(grid, row, column, text.clone(), tolerance) {
                    Ok(Some(ParsedCellValue::InvalidText(_))) => {
                        summary.updated += 1;
                        summary.invalid += 1;
                    }
                    Ok(Some(ParsedCellValue::Empty | ParsedCellValue::ValidNumber(_))) => {
                        summary.updated += 1;
                        summary.valid += 1;
                    }
                    Ok(None) => summary
                        .warnings
                        .push(format!("column '{}' is read-only", column.label)),
                    Err(error) => summary.warnings.push(error.to_string()),
                }
            }
        }
        summary.incomplete = grid.validation_summary().incomplete;
        summary.invalid += grid.validation_summary().invalid;
        Ok(summary)
    }
}

pub fn cell_text(
    grid: &CompositionGrid,
    row: GridRowId,
    column: &GridColumn,
) -> Result<String, GridError> {
    match column.kind {
        GridColumnKind::RowId => Ok(row.get().to_string()),
        GridColumnKind::Component { component, .. } => grid.component_text(row, component, 6),
        GridColumnKind::Scalar { field } => grid.scalar_text(row, field),
        GridColumnKind::Validation => Ok(grid
            .row_validation(row)?
            .iter()
            .map(validation_message)
            .collect::<Vec<_>>()
            .join("; ")),
    }
}

pub fn set_grid_cell_text(
    grid: &mut CompositionGrid,
    row: GridRowId,
    column: &GridColumn,
    text: String,
    tolerance: Tolerance,
) -> Result<Option<ParsedCellValue>, GridError> {
    match column.kind {
        GridColumnKind::Component {
            component,
            dependent: false,
        } if column.editable => {
            let parsed = parse_cell_text(&text);
            grid.set_component_text(row, component, text, tolerance)?;
            Ok(Some(parsed))
        }
        GridColumnKind::Scalar { field } => grid.set_scalar_text(row, field, text).map(Some),
        _ => Ok(None),
    }
}

fn resolve_header(
    header: &str,
    columns: &[GridColumn],
    warnings: &mut Vec<String>,
) -> Option<GridColumnId> {
    let normalized = normalize_header(header);
    let matches: Vec<_> = columns
        .iter()
        .filter(|column| {
            normalize_header(&column.label) == normalized
                || match column.kind {
                    GridColumnKind::Component { component, .. } => {
                        let canonical = ["a", "b", "c", "d"];
                        let local = ["u", "v", "w", ""];
                        canonical[component] == normalized || local[component] == normalized
                    }
                    GridColumnKind::RowId => matches!(normalized.as_str(), "row" | "row id" | "id"),
                    _ => false,
                }
        })
        .collect();
    match matches.as_slice() {
        [column] => Some(column.id),
        [] => {
            warnings.push(format!("unrecognized header '{header}'"));
            None
        }
        _ => {
            warnings.push(format!("ambiguous header '{header}'"));
            None
        }
    }
}

fn normalize_header(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .split_once('[')
        .map_or_else(
            || value.trim().to_ascii_lowercase(),
            |(name, _)| name.trim().to_ascii_lowercase(),
        )
}

fn parse_cell_text(text: &str) -> ParsedCellValue {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        ParsedCellValue::Empty
    } else {
        match trimmed.parse::<f64>() {
            Ok(value) if value.is_finite() => ParsedCellValue::ValidNumber(value),
            _ => ParsedCellValue::InvalidText(text.to_owned()),
        }
    }
}

fn scalar_sort_value(grid: &CompositionGrid, row: GridRowId, field: ScalarFieldId) -> f64 {
    grid.scalar_values(row)
        .ok()
        .and_then(|values| {
            grid.fields()
                .iter()
                .position(|candidate| candidate.id() == field)
                .and_then(|index| values[index])
        })
        .unwrap_or(f64::INFINITY)
}

fn left_row_id(left: GridRowId, right: GridRowId) -> Ordering {
    left.get().cmp(&right.get())
}

fn validation_message(issue: &GridValidationIssue) -> String {
    match issue {
        GridValidationIssue::MissingComponent { column } => {
            format!("component {} is missing", column + 1)
        }
        GridValidationIssue::InvalidNumber { column, value } => {
            format!("component {} is not a number: {value}", column + 1)
        }
        GridValidationIssue::NonFiniteComponent { column } => {
            format!("component {} is non-finite", column + 1)
        }
        GridValidationIssue::NegativeComponent { column } => {
            format!("component {} is negative", column + 1)
        }
        GridValidationIssue::SumMismatch { actual } => {
            format!("component sum is {actual}, expected 1")
        }
        GridValidationIssue::NegativeDependentComponent { component } => {
            format!("dependent component {} is negative", component + 1)
        }
        GridValidationIssue::DuplicateComposition { other } => {
            format!("duplicates row {}", other.get())
        }
        GridValidationIssue::DuplicateCompositionRejected { other } => {
            format!("duplicate of row {} rejected", other.get())
        }
        GridValidationIssue::MissingScalar { field } => {
            format!("scalar field {} is missing", field.get())
        }
        GridValidationIssue::CoordinateSpaceMismatch => "coordinate-space mismatch".to_owned(),
    }
}
