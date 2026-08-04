# Clipboard and TSV import/export

ClipboardTable is a GUI-independent TSV representation. Parsing normalizes line endings while preserving empty and trailing cells, reports ragged rows, supports transposition, and writes tab-separated text suitable for Excel.

Table paste supports:

- an active-cell anchor and headerless positional mapping;
- case-insensitive canonical components A-D and local labels u-w;
- scalar field labels, including unambiguous unit suffixes;
- optional row-ID headers;
- multi-row and multi-field rectangles;
- warnings for unknown, ambiguous, read-only, ragged, extra, or out-of-range columns;
- retained invalid component and scalar text.

When a dependent-mode clipboard block supplies all components, the row is validated as a complete scientific statement. The supplied dependent value is not silently discarded or overwritten.

CompositionGrid::append_clipboard appends irregular composition/scalar rows or updates regular scalar columns. GridExportOptions controls headers, row IDs, compositions, selected scalar fields, and precision. write_tsv and read_tsv are headless helpers; OS clipboard access is confined to the optional editor through arboard.

Every paste returns PasteSummary counts for inserted, updated, valid, incomplete, and invalid cells/rows plus warnings, which the editor reports in its status area.
