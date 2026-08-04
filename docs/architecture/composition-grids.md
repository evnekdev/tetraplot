# Composition grids

`CompositionGrid` is renderer-independent scientific data. It has stable `CompositionGridId`, `GridRowId`, and `ScalarFieldId` values and is owned by `TetraplotDocument`.

A `RegularCompositionGrid` generates tetrahedral or local-ternary lattice compositions from `RegularGridDefinition`; scalar values remain aligned with stable generated rows rather than sorted table positions. Redefinition is explicit: `ClearScalars` or tolerance-aware `PreserveMatchingCoordinates`.

An `IrregularCompositionGrid` retains raw component cells plus its last validated coordinate. Invalid and incomplete rows therefore stay visible and editable. Local coordinates are primary for section/chart grids; tetrahedral/world positions are derived on preparation. `PreparedGridPoint` carries these derived positions without storing a second editable world copy.

## Editor behaviour

CompositionGrid::columns builds stable row, component, scalar, and validation descriptors. Regular coordinates remain generated and read-only. Irregular rows support single-component edits, insertion, append, deletion, and reorder while GridRowId remains stable. Valid rows alone produce PreparedGridPoint values for rendering and picking.

CompositionEntryMode supports all components or one dependent Component. Direct edits exclude the dependent component from raw input so it is recomputed immediately without normalizing independent values. A supplied full clipboard row is checked against the computed dependent value. Negative dependent values and sum mismatches remain row errors.

Coordinate, scalar, and structure revisions are independent. Scalar edits do not regenerate coordinates. Duplicate policies allow, warn, or reject; rejection clears the later scientific coordinate but leaves the raw row visible.
