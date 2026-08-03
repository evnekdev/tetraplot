# Composition grids

`CompositionGrid` is renderer-independent scientific data. It has stable `CompositionGridId`, `GridRowId`, and `ScalarFieldId` values and is owned by `TetraplotDocument`.

A `RegularCompositionGrid` generates tetrahedral or local-ternary lattice compositions from `RegularGridDefinition`; scalar values remain aligned with stable generated rows rather than sorted table positions. Redefinition is explicit: `ClearScalars` or tolerance-aware `PreserveMatchingCoordinates`.

An `IrregularCompositionGrid` retains raw component cells plus its last validated coordinate. Invalid and incomplete rows therefore stay visible and editable. Local coordinates are primary for section/chart grids; tetrahedral/world positions are derived on preparation. `PreparedGridPoint` carries these derived positions without storing a second editable world copy.
