# Clipboard and TSV import/export

`ClipboardTable` is a GUI-independent rectangular TSV representation. It parses headers, preserves cells as strings, supports transposition, and writes tab-separated text. Header matching for components and scalar names is case-insensitive where unambiguous.

`CompositionGrid::append_clipboard` routes data into regular scalar columns or irregular coordinate-plus-scalar rows. Malformed values become validation feedback instead of discarding their rows. `GridExportOptions` controls headers, row IDs, compositions, scalar fields, and precision. `write_tsv` and `read_tsv` are headless helpers.

The native editor uses `arboard` only inside the `editor` feature for the operating-system clipboard. Core grid and TSV logic has no GUI/clipboard dependency.

## Clipboard semantics

ClipboardTable preserves tab-separated empty cells, transposes rectangles, reports ragged row widths, and optionally detects a header row. Copy commands cover the selected rectangle, selected headers, generated regular compositions, and the full table.

Header-aware paste recognizes canonical A/B/C/D and u/v/w coordinates, row headers, scalar field names, and unambiguous scalar names with bracketed units. Headerless paste maps from the active stable cell. Existing-grid paste can cover one scalar column or a multi-field rectangle; irregular append can mix components and scalars. Unknown, ambiguous, extra, and ragged input is reported through PasteSummary warnings.

TSV import creates scalar fields from non-coordinate headers and preserves units. TSV export can limit scalar fields, include row IDs or compositions, and uses field precision. Invalid scalar clipboard text is retained in editor staging and is not committed as a missing value.
