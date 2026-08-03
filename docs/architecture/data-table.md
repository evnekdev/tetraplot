# Data table

The editor lower panel uses the selected grid's stable row IDs, not view positions. Regular composition columns are generated read-only data; scalar cells are editable. Irregular rows retain raw composition input and validation state.

`CompositionEntryMode::DependentComponent` calculates the selected component as `1 - sum(independent)`. A supplied complete row is checked rather than overwritten. No components are silently normalized. Missing, non-finite, negative, sum-mismatch, dependent-negative, and tolerance-aware duplicate conditions are retained as `GridValidationIssue` values.

The native table uses a scrolling row range so it does not create one widget hierarchy for every stored row. It currently provides direct scalar entry, row selection, and clipboard copy actions; richer rectangular keyboard navigation is a planned enhancement.
