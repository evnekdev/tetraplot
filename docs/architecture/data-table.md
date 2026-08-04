# Data table

DataTableState is independent of egui. It tracks an active stable cell, rectangular CellRange, edit buffer, scroll request, column widths, sort policy, and regular-grid redefinition policy. Rows are addressed by GridRowId and columns by deterministic GridColumnId, never by a persistent vector position.

GridColumnKind distinguishes row IDs, components, scalar fields, and validation. Regular components and dependent components are read-only. Irregular independent components and every scalar column are editable.

Implemented operations include:

- active-cell movement with arrows, Tab/Shift+Tab, and Enter/Shift+Enter;
- Shift-extension and Ctrl/Cmd+A rectangular selection;
- rectangular copy, optional headers, composition-only copy, and full-table copy;
- header-aware or positional paste, transposed paste, clear, and fill-down;
- irregular insert, append, clipboard append, and stable-ID row deletion;
- stable sorting and one-shot scroll-to-row after graphical selection.

Editable text controls retain their own keyboard focus, so table shortcuts do not fire while a cell editor consumes input. Invalid scalar strings use ParsedCellValue::InvalidText and remain visible; they are not converted to missing values. Invalid irregular component text likewise remains in raw component storage.

The GUI virtualizes visible rows with egui ScrollArea::show_rows. The validation column and invalid-cell backgrounds expose row/cell errors with tooltips, and a compact summary reports valid, incomplete, invalid, and warning counts.
