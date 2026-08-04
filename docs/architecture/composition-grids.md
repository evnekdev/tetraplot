# Composition grids

CompositionGrid is renderer-independent scientific data owned by TetraplotDocument. Grids, rows, scalar fields, and table columns use stable IDs.

A RegularCompositionGrid generates tetrahedral or local-ternary lattice coordinates from RegularGridDefinition. Coordinates are read-only; scalar fields remain editable. Redefinition is explicit and accepts either ClearScalars or tolerance-aware PreserveMatchingCoordinates. The editor exposes subdivisions, the current lexicographic ordering, and the selected redefinition policy.

An IrregularCompositionGrid retains raw component strings plus its last validated coordinate. Invalid and incomplete rows therefore remain visible. It supports all-component input or a selected dependent component computed as 1 - sum(independent). Independent values are never silently normalized. A complete pasted row containing the dependent component is checked against the computed value; disagreement produces a sum-mismatch error.

Validation covers missing, malformed, non-finite, negative, sum-mismatch, negative-dependent, duplicate-warning, and duplicate-rejection states. Scalar storage separately retains invalid raw text. Coordinate, scalar, and structure revisions are independent.

Valid rows prepare to PreparedGridPoint, which carries grid and row IDs, optional local coordinate, tetrahedral coordinate, world coordinate, and scalar values. Missing section/chart targets return GridError::CoordinateSpaceMismatch; no graphical coordinate is invented for invalid or unmappable rows.
