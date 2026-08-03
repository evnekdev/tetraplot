# Clipboard and TSV import/export

`ClipboardTable` is a GUI-independent rectangular TSV representation. It parses headers, preserves cells as strings, supports transposition, and writes tab-separated text. Header matching for components and scalar names is case-insensitive where unambiguous.

`CompositionGrid::append_clipboard` routes data into regular scalar columns or irregular coordinate-plus-scalar rows. Malformed values become validation feedback instead of discarding their rows. `GridExportOptions` controls headers, row IDs, compositions, scalar fields, and precision. `write_tsv` and `read_tsv` are headless helpers.

The native editor uses `arboard` only inside the `editor` feature for the operating-system clipboard. Core grid and TSV logic has no GUI/clipboard dependency.
