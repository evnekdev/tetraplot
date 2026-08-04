# Data table

The editor lower panel uses the selected grid's stable row IDs, not view positions. Regular composition columns are generated read-only data; scalar cells are editable. Irregular rows retain raw composition input and validation state.

`CompositionEntryMode::DependentComponent` calculates the selected component as `1 - sum(independent)`. A supplied complete row is checked rather than overwritten. No components are silently normalized. Missing, non-finite, negative, sum-mismatch, dependent-negative, and tolerance-aware duplicate conditions are retained as `GridValidationIssue` values.

The native table uses a scrolling row range so it does not create one widget hierarchy for every stored row. It currently provides direct scalar entry, row selection, and clipboard copy actions; richer rectangular keyboard navigation is a planned enhancement.

## Interaction model

DataTableState is independent of egui. It owns the active GridCellAddress, anchor/focus CellRange, stable row order, explicit GridColumn descriptors, edit buffer, staged invalid scalar cells, scroll target, and column widths. Refresh removes deleted identities and appends new rows without destroying a user-defined sort order.

The docked table provides rectangular click/shift selection, arrows, Tab and Shift+Tab, Enter and Shift+Enter, Ctrl+A, Ctrl+C, Ctrl+V, Delete/Backspace, copy with headers, transposed paste, clear, and fill down. Shortcuts are disabled while a text editor owns keyboard input. Grid row identity never derives from a visible index.

Regular component columns are selectable but read-only. Irregular independent components and all scalar columns are editable. A ParsedCellValue distinguishes Empty, ValidNumber, and InvalidText; invalid scalar text remains in DataTableState until replaced, while invalid irregular component text remains in IrregularGridRow.
