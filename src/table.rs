//! GUI-independent data-table selection, navigation, and edit staging.

use std::collections::BTreeMap;

use crate::{Component, CompositionGridId, GridColumnId, GridRowId, ScalarFieldId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GridColumnKind {
    RowId,
    Component {
        component: Component,
        dependent: bool,
    },
    LocalComponent {
        component: usize,
        dependent: bool,
    },
    Scalar {
        field: ScalarFieldId,
    },
    Validation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridColumn {
    pub id: GridColumnId,
    pub label: String,
    pub kind: GridColumnKind,
    pub editable: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GridCellAddress {
    pub row: GridRowId,
    pub column: GridColumnId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellRange {
    pub anchor: GridCellAddress,
    pub focus: GridCellAddress,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParsedCellValue {
    Empty,
    ValidNumber(f64),
    InvalidText(String),
}

impl ParsedCellValue {
    pub fn parse(text: impl Into<String>) -> Self {
        let text = text.into();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            Self::Empty
        } else {
            match trimmed.parse::<f64>() {
                Ok(value) if value.is_finite() => Self::ValidNumber(value),
                _ => Self::InvalidText(text),
            }
        }
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
        let parsed = ParsedCellValue::parse(text.clone());
        Self {
            address,
            text,
            parsed,
        }
    }

    pub fn update(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.parsed = ParsedCellValue::parse(self.text.clone());
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataTableState {
    pub grid: CompositionGridId,
    pub active_cell: Option<GridCellAddress>,
    pub selection: Option<CellRange>,
    pub edit_buffer: Option<CellEditBuffer>,
    pub first_visible_row: usize,
    pub column_widths: Vec<f32>,
    pub row_order: Vec<GridRowId>,
    pub columns: Vec<GridColumn>,
    pub invalid_cells: BTreeMap<GridCellAddress, String>,
}

impl DataTableState {
    pub fn new(
        grid: CompositionGridId,
        row_order: Vec<GridRowId>,
        columns: Vec<GridColumn>,
    ) -> Self {
        let column_widths = columns
            .iter()
            .map(|column| match column.kind {
                GridColumnKind::RowId => 64.0,
                GridColumnKind::Validation => 110.0,
                _ => 96.0,
            })
            .collect();
        Self {
            grid,
            active_cell: None,
            selection: None,
            edit_buffer: None,
            first_visible_row: 0,
            column_widths,
            row_order,
            columns,
            invalid_cells: BTreeMap::new(),
        }
    }

    pub fn refresh(&mut self, rows: Vec<GridRowId>, columns: Vec<GridColumn>) {
        self.row_order.retain(|row| rows.contains(row));
        for row in rows {
            if !self.row_order.contains(&row) {
                self.row_order.push(row);
            }
        }
        self.columns = columns;
        self.column_widths.resize(self.columns.len(), 96.0);
        self.retain_valid_addresses();
    }

    pub fn activate(&mut self, address: GridCellAddress, extend: bool) -> bool {
        if self.indices(address).is_none() {
            return false;
        }
        if extend {
            let anchor = self
                .selection
                .map(|selection| selection.anchor)
                .or(self.active_cell)
                .unwrap_or(address);
            self.selection = Some(CellRange {
                anchor,
                focus: address,
            });
        } else {
            self.selection = Some(CellRange {
                anchor: address,
                focus: address,
            });
        }
        self.active_cell = Some(address);
        true
    }

    pub fn move_active(&mut self, row_delta: isize, column_delta: isize, extend: bool) -> bool {
        let Some(active) = self.active_cell else {
            let (Some(row), Some(column)) = (
                self.row_order.first().copied(),
                self.columns.first().map(|column| column.id),
            ) else {
                return false;
            };
            return self.activate(GridCellAddress { row, column }, extend);
        };
        let Some((row, column)) = self.indices(active) else {
            return false;
        };
        let row = offset_clamped(row, row_delta, self.row_order.len());
        let column = offset_clamped(column, column_delta, self.columns.len());
        self.activate(
            GridCellAddress {
                row: self.row_order[row],
                column: self.columns[column].id,
            },
            extend,
        )
    }

    pub fn select_all(&mut self) -> bool {
        let (Some(first_row), Some(last_row), Some(first_column), Some(last_column)) = (
            self.row_order.first().copied(),
            self.row_order.last().copied(),
            self.columns.first().map(|column| column.id),
            self.columns.last().map(|column| column.id),
        ) else {
            return false;
        };
        let anchor = GridCellAddress {
            row: first_row,
            column: first_column,
        };
        let focus = GridCellAddress {
            row: last_row,
            column: last_column,
        };
        self.active_cell = Some(anchor);
        self.selection = Some(CellRange { anchor, focus });
        true
    }

    pub fn selected_cells(&self) -> Vec<GridCellAddress> {
        let Some((row_start, row_end, column_start, column_end)) = self.selection_bounds() else {
            return Vec::new();
        };
        self.row_order[row_start..=row_end]
            .iter()
            .flat_map(|row| {
                self.columns[column_start..=column_end]
                    .iter()
                    .map(move |column| GridCellAddress {
                        row: *row,
                        column: column.id,
                    })
            })
            .collect()
    }

    pub fn selected_rows(&self) -> Vec<GridRowId> {
        let Some((row_start, row_end, _, _)) = self.selection_bounds() else {
            return self
                .active_cell
                .map(|cell| vec![cell.row])
                .unwrap_or_default();
        };
        self.row_order[row_start..=row_end].to_vec()
    }

    pub fn selection_bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let selection = self.selection?;
        let (anchor_row, anchor_column) = self.indices(selection.anchor)?;
        let (focus_row, focus_column) = self.indices(selection.focus)?;
        Some((
            anchor_row.min(focus_row),
            anchor_row.max(focus_row),
            anchor_column.min(focus_column),
            anchor_column.max(focus_column),
        ))
    }

    pub fn scroll_to_row(&mut self, row: GridRowId) -> Option<usize> {
        let index = self
            .row_order
            .iter()
            .position(|candidate| *candidate == row)?;
        self.first_visible_row = index;
        Some(index)
    }

    pub fn replace_row_order(&mut self, rows: Vec<GridRowId>) {
        self.row_order = rows;
        self.retain_valid_addresses();
    }

    pub fn fill_down(&self, source: &crate::ClipboardTable) -> crate::ClipboardTable {
        let count = self
            .selection_bounds()
            .map(|(start, end, _, _)| end - start + 1)
            .unwrap_or(0);
        crate::ClipboardTable {
            headers: None,
            rows: source
                .rows
                .first()
                .cloned()
                .map(|row| vec![row; count])
                .unwrap_or_default(),
        }
    }

    pub fn begin_edit(&mut self, address: GridCellAddress, text: impl Into<String>) -> bool {
        if self.indices(address).is_none() {
            return false;
        }
        self.active_cell = Some(address);
        self.edit_buffer = Some(CellEditBuffer::new(address, text));
        true
    }

    pub fn retain_invalid(&mut self, address: GridCellAddress, text: impl Into<String>) {
        self.invalid_cells.insert(address, text.into());
    }

    pub fn clear_invalid(&mut self, address: GridCellAddress) {
        self.invalid_cells.remove(&address);
    }

    pub fn invalid_text(&self, address: GridCellAddress) -> Option<&str> {
        self.invalid_cells.get(&address).map(String::as_str)
    }

    pub fn column(&self, id: GridColumnId) -> Option<&GridColumn> {
        self.columns.iter().find(|column| column.id == id)
    }

    pub fn indices(&self, address: GridCellAddress) -> Option<(usize, usize)> {
        Some((
            self.row_order.iter().position(|row| *row == address.row)?,
            self.columns
                .iter()
                .position(|column| column.id == address.column)?,
        ))
    }

    fn retain_valid_addresses(&mut self) {
        let address_valid =
            |address: GridCellAddress, state: &Self| state.indices(address).is_some();
        if self
            .active_cell
            .is_some_and(|address| !address_valid(address, self))
        {
            self.active_cell = None;
        }
        if self.selection.is_some_and(|range| {
            !address_valid(range.anchor, self) || !address_valid(range.focus, self)
        }) {
            self.selection = None;
        }
        if self
            .edit_buffer
            .as_ref()
            .is_some_and(|edit| !address_valid(edit.address, self))
        {
            self.edit_buffer = None;
        }
        let rows = &self.row_order;
        let columns = &self.columns;
        self.invalid_cells.retain(|address, _| {
            rows.contains(&address.row) && columns.iter().any(|column| column.id == address.column)
        });
        self.first_visible_row = self
            .first_visible_row
            .min(self.row_order.len().saturating_sub(1));
    }
}

fn offset_clamped(index: usize, delta: isize, length: usize) -> usize {
    if length == 0 {
        return 0;
    }
    index.saturating_add_signed(delta).min(length - 1)
}
