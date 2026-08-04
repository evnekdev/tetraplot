//! Renderer-independent composition grids, scalar values, validation, and TSV interchange.

use std::{
    collections::BTreeMap,
    io::{Read, Write},
};

use crate::{Component, EmbeddedChartId, SectionId, TernaryPoint, TetraPoint, Tolerance};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompositionGridId(u64);
impl CompositionGridId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScalarFieldId(u64);
impl ScalarFieldId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GridRowId(u64);
impl GridRowId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GridColumnId(u64);
impl GridColumnId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GridPointIndex(pub [u32; 4]);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum RegularGridOrdering {
    #[default]
    Lexicographic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegularTetraGridDefinition {
    pub subdivisions: u32,
    pub ordering: RegularGridOrdering,
}
impl Default for RegularTetraGridDefinition {
    fn default() -> Self {
        Self {
            subdivisions: 10,
            ordering: RegularGridOrdering::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegularTernaryGridDefinition {
    pub subdivisions: u32,
    pub ordering: RegularGridOrdering,
}
impl Default for RegularTernaryGridDefinition {
    fn default() -> Self {
        Self {
            subdivisions: 10,
            ordering: RegularGridOrdering::default(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegularGridDefinition {
    Tetrahedral(RegularTetraGridDefinition),
    LocalTernary(RegularTernaryGridDefinition),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridCoordinateSpace {
    Tetrahedral,
    Section(SectionId),
    EmbeddedChart(EmbeddedChartId),
}
impl GridCoordinateSpace {
    pub const fn is_local(self) -> bool {
        !matches!(self, Self::Tetrahedral)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridCoordinate {
    Tetrahedral(TetraPoint),
    LocalTernary(TernaryPoint),
}
impl GridCoordinate {
    pub const fn dimension(self) -> usize {
        match self {
            Self::Tetrahedral(_) => 4,
            Self::LocalTernary(_) => 3,
        }
    }
    pub const fn as_values(self) -> [f64; 4] {
        match self {
            Self::Tetrahedral(value) => value.as_array(),
            Self::LocalTernary(value) => {
                let [a, b, c] = value.as_array();
                [a, b, c, 0.0]
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum CompositionEntryMode {
    #[default]
    AllComponents,
    DependentComponent(Component),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum DuplicateCompositionPolicy {
    Allow,
    #[default]
    Warn,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridRedefinitionPolicy {
    ClearScalars,
    PreserveMatchingCoordinates,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScalarField {
    id: ScalarFieldId,
    name: String,
    units: Option<String>,
    values: Vec<Option<f64>>,
    precision: usize,
    revision: u64,
}
impl ScalarField {
    pub fn new(id: ScalarFieldId, name: impl Into<String>, row_count: usize) -> Self {
        Self {
            id,
            name: name.into(),
            units: None,
            values: vec![None; row_count],
            precision: 6,
            revision: 0,
        }
    }
    pub const fn id(&self) -> ScalarFieldId {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
        self.revision += 1;
    }
    pub fn units(&self) -> Option<&str> {
        self.units.as_deref()
    }
    pub fn set_units(&mut self, value: Option<impl Into<String>>) {
        self.units = value.map(Into::into);
        self.revision += 1;
    }
    pub fn values(&self) -> &[Option<f64>] {
        &self.values
    }
    pub const fn precision(&self) -> usize {
        self.precision
    }
    pub fn set_precision(&mut self, precision: usize) {
        self.precision = precision;
        self.revision += 1;
    }
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub fn value(&self, index: usize) -> Option<Option<f64>> {
        self.values.get(index).copied()
    }
    pub fn set_value(&mut self, index: usize, value: Option<f64>) -> Result<(), GridError> {
        if let Some(value) = value
            && !value.is_finite()
        {
            return Err(GridError::NonFiniteScalar { value });
        }
        let cell = self
            .values
            .get_mut(index)
            .ok_or(GridError::UnknownGridRowIndex { index })?;
        *cell = value;
        self.revision += 1;
        Ok(())
    }
    fn resize(&mut self, count: usize) {
        self.values.resize(count, None);
        self.revision += 1;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegularGridPoint {
    pub row_id: GridRowId,
    pub logical_index: GridPointIndex,
    pub coordinate: GridCoordinate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegularCompositionGrid {
    id: Option<CompositionGridId>,
    name: String,
    definition: RegularGridDefinition,
    coordinate_space: GridCoordinateSpace,
    points: Vec<RegularGridPoint>,
    fields: Vec<ScalarField>,
    next_row_id: u64,
    next_field_id: u64,
    entry_mode: CompositionEntryMode,
    coordinate_revision: u64,
    scalar_revision: u64,
    structure_revision: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrregularGridRow {
    pub id: GridRowId,
    pub coordinate: Option<GridCoordinate>,
    raw_components: Vec<String>,
    pub validation: Vec<GridValidationIssue>,
}
impl IrregularGridRow {
    pub fn raw_components(&self) -> &[String] {
        &self.raw_components
    }
    pub fn is_valid(&self) -> bool {
        self.coordinate.is_some() && !self.validation.iter().any(GridValidationIssue::is_error)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrregularCompositionGrid {
    id: Option<CompositionGridId>,
    name: String,
    coordinate_space: GridCoordinateSpace,
    rows: Vec<IrregularGridRow>,
    fields: Vec<ScalarField>,
    next_row_id: u64,
    next_field_id: u64,
    entry_mode: CompositionEntryMode,
    duplicate_policy: DuplicateCompositionPolicy,
    coordinate_revision: u64,
    scalar_revision: u64,
    structure_revision: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CompositionGrid {
    Regular(RegularCompositionGrid),
    Irregular(IrregularCompositionGrid),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GridValidationIssue {
    MissingComponent { column: usize },
    InvalidNumber { column: usize, value: String },
    NonFiniteComponent { column: usize },
    NegativeComponent { column: usize },
    SumMismatch { actual: String },
    NegativeDependentComponent { component: usize },
    DuplicateComposition { other: GridRowId },
    DuplicateCompositionRejected { other: GridRowId },
    MissingScalar { field: ScalarFieldId },
    CoordinateSpaceMismatch,
}
impl GridValidationIssue {
    pub const fn is_error(&self) -> bool {
        !matches!(self, Self::DuplicateComposition { .. })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GridValidationSummary {
    pub valid: usize,
    pub incomplete: usize,
    pub invalid: usize,
    pub warnings: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasteSummary {
    pub inserted: usize,
    pub updated: usize,
    pub invalid: usize,
    pub valid: usize,
    pub incomplete: usize,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedGridPoint {
    pub grid_id: CompositionGridId,
    pub row_id: GridRowId,
    pub local: Option<TernaryPoint>,
    pub tetrahedral: TetraPoint,
    pub world: [f32; 3],
    pub scalar_values: Vec<Option<f64>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClipboardTable {
    pub headers: Option<Vec<String>>,
    pub rows: Vec<Vec<String>>,
}
impl ClipboardTable {
    pub fn parse_tsv(input: &str, has_headers: bool) -> Self {
        let mut rows: Vec<Vec<String>> = input
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .split('\n')
            .filter(|line| !line.is_empty())
            .map(|line| {
                line.split('\t')
                    .map(|cell| cell.trim().to_owned())
                    .collect()
            })
            .collect();
        let headers = if has_headers && !rows.is_empty() {
            Some(rows.remove(0))
        } else {
            None
        };
        Self { headers, rows }
    }
    pub fn transpose(&self) -> Self {
        let width = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        Self {
            headers: None,
            rows: (0..width)
                .map(|column| {
                    self.rows
                        .iter()
                        .map(|row| row.get(column).cloned().unwrap_or_default())
                        .collect()
                })
                .collect(),
        }
    }
    pub fn to_tsv(&self, include_headers: bool) -> String {
        let mut lines = Vec::new();
        if include_headers && let Some(headers) = &self.headers {
            lines.push(headers.join("\t"));
        }
        lines.extend(self.rows.iter().map(|row| row.join("\t")));
        lines.join("\n")
    }
    pub fn parse_tsv_auto(input: &str) -> Self {
        let first_line = input
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned();
        let has_headers = first_line.split('\t').any(|cell| {
            let cell = cell.trim();
            !cell.is_empty() && cell.parse::<f64>().is_err()
        });
        Self::parse_tsv(input, has_headers)
    }
    pub fn width(&self) -> usize {
        self.headers
            .as_ref()
            .map(Vec::len)
            .into_iter()
            .chain(self.rows.iter().map(Vec::len))
            .max()
            .unwrap_or(0)
    }
    pub fn shape_warnings(&self) -> Vec<String> {
        let expected = self
            .headers
            .as_ref()
            .map(Vec::len)
            .or_else(|| self.rows.first().map(Vec::len))
            .unwrap_or(0);
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.len() != expected)
            .map(|(index, row)| {
                format!(
                    "clipboard row {} has {} columns; expected {}",
                    index + 1,
                    row.len(),
                    expected
                )
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridExportOptions {
    pub include_headers: bool,
    pub include_row_ids: bool,
    pub include_compositions: bool,
    pub scalar_fields: Vec<ScalarFieldId>,
    pub precision: usize,
}
impl Default for GridExportOptions {
    fn default() -> Self {
        Self {
            include_headers: true,
            include_row_ids: false,
            include_compositions: true,
            scalar_fields: Vec::new(),
            precision: 6,
        }
    }
}

#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum GridError {
    #[error("regular grid subdivisions must be greater than zero")]
    InvalidSubdivisions,
    #[error("grid row index {index} does not exist")]
    UnknownGridRowIndex { index: usize },
    #[error("grid row {id} does not exist")]
    UnknownGridRow { id: u64 },
    #[error("scalar field {id} does not exist")]
    UnknownScalarField { id: u64 },
    #[error("scalar value is not finite: {value:?}")]
    NonFiniteScalar { value: f64 },
    #[error("column {column} is outside the table")]
    UnknownColumn { column: usize },
    #[error("column {column} is read-only")]
    ReadOnlyColumn { column: usize },
    #[error("regular-grid rows are generated and cannot be inserted or deleted")]
    RegularRowsFixed,
    #[error("composition uses {actual} values; expected {expected}")]
    ComponentCount { actual: usize, expected: usize },
    #[error("composition is invalid: {message}")]
    InvalidComposition { message: String },
    #[error("grid coordinate does not match its declared coordinate space")]
    CoordinateSpaceMismatch,
    #[error("duplicate composition rejected")]
    DuplicateComposition,
    #[error("TSV parse error: {message}")]
    Tsv { message: String },
    #[error("I/O failed: {message}")]
    Io { message: String },
}
impl RegularCompositionGrid {
    pub fn tetrahedral(
        name: impl Into<String>,
        definition: RegularTetraGridDefinition,
    ) -> Result<Self, GridError> {
        if definition.subdivisions == 0 {
            return Err(GridError::InvalidSubdivisions);
        }
        let mut grid = Self {
            id: None,
            name: name.into(),
            definition: RegularGridDefinition::Tetrahedral(definition),
            coordinate_space: GridCoordinateSpace::Tetrahedral,
            points: Vec::new(),
            fields: Vec::new(),
            next_row_id: 0,
            next_field_id: 0,
            entry_mode: CompositionEntryMode::AllComponents,
            coordinate_revision: 0,
            scalar_revision: 0,
            structure_revision: 0,
        };
        grid.regenerate(GridRedefinitionPolicy::ClearScalars, Tolerance::default())?;
        Ok(grid)
    }
    pub fn local_ternary(
        name: impl Into<String>,
        space: GridCoordinateSpace,
        definition: RegularTernaryGridDefinition,
    ) -> Result<Self, GridError> {
        if definition.subdivisions == 0 {
            return Err(GridError::InvalidSubdivisions);
        }
        if !space.is_local() {
            return Err(GridError::CoordinateSpaceMismatch);
        }
        let mut grid = Self {
            id: None,
            name: name.into(),
            definition: RegularGridDefinition::LocalTernary(definition),
            coordinate_space: space,
            points: Vec::new(),
            fields: Vec::new(),
            next_row_id: 0,
            next_field_id: 0,
            entry_mode: CompositionEntryMode::AllComponents,
            coordinate_revision: 0,
            scalar_revision: 0,
            structure_revision: 0,
        };
        grid.regenerate(GridRedefinitionPolicy::ClearScalars, Tolerance::default())?;
        Ok(grid)
    }
    pub fn id(&self) -> Option<CompositionGridId> {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
        self.structure_revision += 1;
    }
    pub const fn definition(&self) -> RegularGridDefinition {
        self.definition
    }
    pub const fn coordinate_space(&self) -> GridCoordinateSpace {
        self.coordinate_space
    }
    pub fn points(&self) -> &[RegularGridPoint] {
        &self.points
    }
    pub fn fields(&self) -> &[ScalarField] {
        &self.fields
    }
    pub fn field(&self, id: ScalarFieldId) -> Option<&ScalarField> {
        self.fields.iter().find(|field| field.id == id)
    }
    pub fn field_mut(&mut self, id: ScalarFieldId) -> Option<&mut ScalarField> {
        self.fields.iter_mut().find(|field| field.id == id)
    }
    pub const fn entry_mode(&self) -> CompositionEntryMode {
        self.entry_mode
    }
    pub fn set_entry_mode(&mut self, mode: CompositionEntryMode) -> Result<(), GridError> {
        validate_entry_mode(mode, self.dimension())?;
        self.entry_mode = mode;
        self.structure_revision += 1;
        Ok(())
    }
    pub const fn coordinate_revision(&self) -> u64 {
        self.coordinate_revision
    }
    pub const fn scalar_revision(&self) -> u64 {
        self.scalar_revision
    }
    pub const fn structure_revision(&self) -> u64 {
        self.structure_revision
    }
    pub fn add_scalar_field(&mut self, name: impl Into<String>) -> ScalarFieldId {
        let id = ScalarFieldId::new(self.next_field_id);
        self.next_field_id += 1;
        self.fields
            .push(ScalarField::new(id, name, self.points.len()));
        self.structure_revision += 1;
        id
    }
    pub fn remove_scalar_field(&mut self, id: ScalarFieldId) -> Option<ScalarField> {
        let index = self.fields.iter().position(|field| field.id == id)?;
        self.structure_revision += 1;
        Some(self.fields.remove(index))
    }
    pub fn set_scalar(
        &mut self,
        row: GridRowId,
        field: ScalarFieldId,
        value: Option<f64>,
    ) -> Result<(), GridError> {
        let index = self.row_index(row)?;
        self.field_mut(field)
            .ok_or(GridError::UnknownScalarField { id: field.get() })?
            .set_value(index, value)?;
        self.scalar_revision += 1;
        Ok(())
    }
    pub fn row_index(&self, row: GridRowId) -> Result<usize, GridError> {
        self.points
            .iter()
            .position(|point| point.row_id == row)
            .ok_or(GridError::UnknownGridRow { id: row.get() })
    }
    pub fn set_definition(
        &mut self,
        definition: RegularGridDefinition,
        policy: GridRedefinitionPolicy,
        tolerance: Tolerance,
    ) -> Result<(), GridError> {
        if subdivisions(definition) == 0 {
            return Err(GridError::InvalidSubdivisions);
        }
        let is_local = matches!(definition, RegularGridDefinition::LocalTernary(_));
        if is_local != self.coordinate_space.is_local() {
            return Err(GridError::CoordinateSpaceMismatch);
        }
        self.definition = definition;
        self.regenerate(policy, tolerance)
    }
    fn regenerate(
        &mut self,
        policy: GridRedefinitionPolicy,
        tolerance: Tolerance,
    ) -> Result<(), GridError> {
        let previous: BTreeMap<CoordinateKey, Vec<Option<f64>>> = match policy {
            GridRedefinitionPolicy::ClearScalars => BTreeMap::new(),
            GridRedefinitionPolicy::PreserveMatchingCoordinates => self
                .points
                .iter()
                .enumerate()
                .map(|(index, point)| {
                    (
                        CoordinateKey::from(point.coordinate),
                        self.fields
                            .iter()
                            .map(|field| field.value(index).flatten())
                            .collect(),
                    )
                })
                .collect(),
        };
        let n = subdivisions(self.definition);
        let mut points = Vec::new();
        match self.definition {
            RegularGridDefinition::Tetrahedral(_) => {
                for a in 0..=n {
                    for b in 0..=n - a {
                        for c in 0..=n - a - b {
                            let d = n - a - b - c;
                            points.push((
                                GridPointIndex([a, b, c, d]),
                                GridCoordinate::Tetrahedral(TetraPoint::from_unchecked([
                                    f64::from(a) / f64::from(n),
                                    f64::from(b) / f64::from(n),
                                    f64::from(c) / f64::from(n),
                                    f64::from(d) / f64::from(n),
                                ])),
                            ));
                        }
                    }
                }
            }
            RegularGridDefinition::LocalTernary(_) => {
                for a in 0..=n {
                    for b in 0..=n - a {
                        let c = n - a - b;
                        points.push((
                            GridPointIndex([a, b, c, 0]),
                            GridCoordinate::LocalTernary(TernaryPoint::from_unchecked([
                                f64::from(a) / f64::from(n),
                                f64::from(b) / f64::from(n),
                                f64::from(c) / f64::from(n),
                            ])),
                        ));
                    }
                }
            }
        }
        self.points = points
            .into_iter()
            .map(|(logical_index, coordinate)| {
                let row_id = GridRowId::new(self.next_row_id);
                self.next_row_id += 1;
                RegularGridPoint {
                    row_id,
                    logical_index,
                    coordinate,
                }
            })
            .collect();
        for field in &mut self.fields {
            field.resize(self.points.len());
        }
        if matches!(policy, GridRedefinitionPolicy::PreserveMatchingCoordinates) {
            for (index, point) in self.points.iter().enumerate() {
                if let Some(values) = previous.get(&CoordinateKey::from(point.coordinate)) {
                    for (field, value) in self.fields.iter_mut().zip(values) {
                        field.values[index] = *value;
                    }
                }
            }
        }
        tolerance
            .validate()
            .map_err(|error| GridError::InvalidComposition {
                message: error.to_string(),
            })?;
        self.coordinate_revision += 1;
        self.structure_revision += 1;
        Ok(())
    }
    fn dimension(&self) -> usize {
        match self.definition {
            RegularGridDefinition::Tetrahedral(_) => 4,
            RegularGridDefinition::LocalTernary(_) => 3,
        }
    }
    fn assign_id(&mut self, id: CompositionGridId) {
        self.id = Some(id);
    }
}

impl IrregularCompositionGrid {
    pub fn tetrahedral(name: impl Into<String>) -> Self {
        Self::new(name, GridCoordinateSpace::Tetrahedral)
    }
    pub fn local_ternary(
        name: impl Into<String>,
        space: GridCoordinateSpace,
    ) -> Result<Self, GridError> {
        if !space.is_local() {
            return Err(GridError::CoordinateSpaceMismatch);
        }
        Ok(Self::new(name, space))
    }
    fn new(name: impl Into<String>, coordinate_space: GridCoordinateSpace) -> Self {
        Self {
            id: None,
            name: name.into(),
            coordinate_space,
            rows: Vec::new(),
            fields: Vec::new(),
            next_row_id: 0,
            next_field_id: 0,
            entry_mode: CompositionEntryMode::AllComponents,
            duplicate_policy: DuplicateCompositionPolicy::Warn,
            coordinate_revision: 0,
            scalar_revision: 0,
            structure_revision: 0,
        }
    }
    pub fn id(&self) -> Option<CompositionGridId> {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn set_name(&mut self, value: impl Into<String>) {
        self.name = value.into();
        self.structure_revision += 1;
    }
    pub const fn coordinate_space(&self) -> GridCoordinateSpace {
        self.coordinate_space
    }
    pub fn rows(&self) -> &[IrregularGridRow] {
        &self.rows
    }
    pub fn fields(&self) -> &[ScalarField] {
        &self.fields
    }
    pub fn field(&self, id: ScalarFieldId) -> Option<&ScalarField> {
        self.fields.iter().find(|field| field.id == id)
    }
    pub fn field_mut(&mut self, id: ScalarFieldId) -> Option<&mut ScalarField> {
        self.fields.iter_mut().find(|field| field.id == id)
    }
    pub const fn entry_mode(&self) -> CompositionEntryMode {
        self.entry_mode
    }
    pub fn set_entry_mode(&mut self, mode: CompositionEntryMode) -> Result<(), GridError> {
        validate_entry_mode(mode, self.dimension())?;
        self.entry_mode = mode;
        self.revalidate(Tolerance::default());
        Ok(())
    }
    pub const fn duplicate_policy(&self) -> DuplicateCompositionPolicy {
        self.duplicate_policy
    }
    pub fn set_duplicate_policy(&mut self, policy: DuplicateCompositionPolicy) {
        self.duplicate_policy = policy;
        self.revalidate(Tolerance::default());
    }
    pub const fn coordinate_revision(&self) -> u64 {
        self.coordinate_revision
    }
    pub const fn scalar_revision(&self) -> u64 {
        self.scalar_revision
    }
    pub const fn structure_revision(&self) -> u64 {
        self.structure_revision
    }
    pub fn add_scalar_field(&mut self, name: impl Into<String>) -> ScalarFieldId {
        let id = ScalarFieldId::new(self.next_field_id);
        self.next_field_id += 1;
        self.fields
            .push(ScalarField::new(id, name, self.rows.len()));
        self.structure_revision += 1;
        id
    }
    pub fn remove_scalar_field(&mut self, id: ScalarFieldId) -> Option<ScalarField> {
        let index = self.fields.iter().position(|field| field.id == id)?;
        self.structure_revision += 1;
        Some(self.fields.remove(index))
    }
    pub fn append_raw_row<I, S>(&mut self, components: I, tolerance: Tolerance) -> GridRowId
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let id = GridRowId::new(self.next_row_id);
        self.next_row_id += 1;
        self.rows.push(IrregularGridRow {
            id,
            coordinate: None,
            raw_components: components.into_iter().map(Into::into).collect(),
            validation: Vec::new(),
        });
        for field in &mut self.fields {
            field.resize(self.rows.len());
        }
        self.revalidate(tolerance);
        self.structure_revision += 1;
        id
    }
    pub fn insert_raw_row<I, S>(
        &mut self,
        index: usize,
        components: I,
        tolerance: Tolerance,
    ) -> Result<GridRowId, GridError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        if index > self.rows.len() {
            return Err(GridError::UnknownGridRowIndex { index });
        }
        let id = GridRowId::new(self.next_row_id);
        self.next_row_id += 1;
        self.rows.insert(
            index,
            IrregularGridRow {
                id,
                coordinate: None,
                raw_components: components.into_iter().map(Into::into).collect(),
                validation: Vec::new(),
            },
        );
        for field in &mut self.fields {
            field.values.insert(index, None);
            field.revision += 1;
        }
        self.revalidate(tolerance);
        self.structure_revision += 1;
        Ok(id)
    }
    pub fn delete_row(&mut self, id: GridRowId) -> Option<IrregularGridRow> {
        let index = self.rows.iter().position(|row| row.id == id)?;
        for field in &mut self.fields {
            field.values.remove(index);
            field.revision += 1;
        }
        self.structure_revision += 1;
        self.coordinate_revision += 1;
        Some(self.rows.remove(index))
    }
    pub fn reorder_row(&mut self, id: GridRowId, destination: usize) -> Result<(), GridError> {
        let index = self.row_index(id)?;
        if destination >= self.rows.len() {
            return Err(GridError::UnknownGridRowIndex { index: destination });
        }
        let row = self.rows.remove(index);
        self.rows.insert(destination, row);
        for field in &mut self.fields {
            let value = field.values.remove(index);
            field.values.insert(destination, value);
            field.revision += 1;
        }
        self.structure_revision += 1;
        Ok(())
    }
    pub fn set_raw_components<I, S>(
        &mut self,
        id: GridRowId,
        components: I,
        tolerance: Tolerance,
    ) -> Result<(), GridError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let index = self.row_index(id)?;
        self.rows[index].raw_components = components.into_iter().map(Into::into).collect();
        self.revalidate(tolerance);
        self.coordinate_revision += 1;
        Ok(())
    }
    pub fn set_scalar(
        &mut self,
        row: GridRowId,
        field: ScalarFieldId,
        value: Option<f64>,
    ) -> Result<(), GridError> {
        let index = self.row_index(row)?;
        self.field_mut(field)
            .ok_or(GridError::UnknownScalarField { id: field.get() })?
            .set_value(index, value)?;
        self.scalar_revision += 1;
        Ok(())
    }
    pub fn row_index(&self, id: GridRowId) -> Result<usize, GridError> {
        self.rows
            .iter()
            .position(|row| row.id == id)
            .ok_or(GridError::UnknownGridRow { id: id.get() })
    }
    pub fn validation_summary(&self) -> GridValidationSummary {
        summarize_rows(&self.rows)
    }
    pub fn revalidate(&mut self, tolerance: Tolerance) {
        let dimension = self.dimension();
        for row in &mut self.rows {
            row.validation.clear();
            row.coordinate = parse_coordinate(
                &row.raw_components,
                dimension,
                self.entry_mode,
                tolerance,
                &mut row.validation,
            );
        }
        apply_duplicates(&mut self.rows, self.duplicate_policy, tolerance);
        self.coordinate_revision += 1;
    }
    fn dimension(&self) -> usize {
        if self.coordinate_space.is_local() {
            3
        } else {
            4
        }
    }
    fn assign_id(&mut self, id: CompositionGridId) {
        self.id = Some(id);
    }
}
impl CompositionGrid {
    pub fn id(&self) -> Option<CompositionGridId> {
        match self {
            Self::Regular(grid) => grid.id(),
            Self::Irregular(grid) => grid.id(),
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Self::Regular(grid) => grid.name(),
            Self::Irregular(grid) => grid.name(),
        }
    }
    pub fn coordinate_space(&self) -> GridCoordinateSpace {
        match self {
            Self::Regular(grid) => grid.coordinate_space(),
            Self::Irregular(grid) => grid.coordinate_space(),
        }
    }
    pub fn rows_len(&self) -> usize {
        match self {
            Self::Regular(grid) => grid.points.len(),
            Self::Irregular(grid) => grid.rows.len(),
        }
    }
    pub fn fields(&self) -> &[ScalarField] {
        match self {
            Self::Regular(grid) => grid.fields(),
            Self::Irregular(grid) => grid.fields(),
        }
    }
    pub fn add_scalar_field(&mut self, name: impl Into<String>) -> ScalarFieldId {
        match self {
            Self::Regular(grid) => grid.add_scalar_field(name),
            Self::Irregular(grid) => grid.add_scalar_field(name),
        }
    }
    pub fn remove_scalar_field(&mut self, id: ScalarFieldId) -> Option<ScalarField> {
        match self {
            Self::Regular(grid) => grid.remove_scalar_field(id),
            Self::Irregular(grid) => grid.remove_scalar_field(id),
        }
    }
    pub fn set_scalar(
        &mut self,
        row: GridRowId,
        field: ScalarFieldId,
        value: Option<f64>,
    ) -> Result<(), GridError> {
        match self {
            Self::Regular(grid) => grid.set_scalar(row, field, value),
            Self::Irregular(grid) => grid.set_scalar(row, field, value),
        }
    }
    pub fn row_ids(&self) -> Vec<GridRowId> {
        match self {
            Self::Regular(grid) => grid.points.iter().map(|point| point.row_id).collect(),
            Self::Irregular(grid) => grid.rows.iter().map(|row| row.id).collect(),
        }
    }
    pub fn row_coordinate(&self, row: GridRowId) -> Result<Option<GridCoordinate>, GridError> {
        match self {
            Self::Regular(grid) => Ok(Some(grid.points[grid.row_index(row)?].coordinate)),
            Self::Irregular(grid) => Ok(grid.rows[grid.row_index(row)?].coordinate),
        }
    }
    pub fn scalar_values(&self, row: GridRowId) -> Result<Vec<Option<f64>>, GridError> {
        match self {
            Self::Regular(grid) => {
                let index = grid.row_index(row)?;
                Ok(grid
                    .fields
                    .iter()
                    .map(|field| field.value(index).flatten())
                    .collect())
            }
            Self::Irregular(grid) => {
                let index = grid.row_index(row)?;
                Ok(grid
                    .fields
                    .iter()
                    .map(|field| field.value(index).flatten())
                    .collect())
            }
        }
    }
    pub fn validation_summary(&self) -> GridValidationSummary {
        match self {
            Self::Regular(grid) => GridValidationSummary {
                valid: grid.points.len(),
                ..GridValidationSummary::default()
            },
            Self::Irregular(grid) => grid.validation_summary(),
        }
    }
    pub fn table(&self, options: &GridExportOptions) -> Result<ClipboardTable, GridError> {
        let selected = select_fields(self.fields(), &options.scalar_fields);
        let dimensions = if self.coordinate_space().is_local() {
            3
        } else {
            4
        };
        let mut headers = Vec::new();
        if options.include_row_ids {
            headers.push("Row".to_owned());
        }
        if options.include_compositions {
            headers.extend(
                component_headers(dimensions)
                    .iter()
                    .map(|name| (*name).to_owned()),
            );
        }
        headers.extend(selected.iter().map(|field| match field.units() {
            Some(units) if !units.is_empty() => format!("{} [{}]", field.name(), units),
            _ => field.name().to_owned(),
        }));
        let rows = self
            .row_ids()
            .into_iter()
            .map(|row| {
                let mut values = Vec::new();
                if options.include_row_ids {
                    values.push(row.get().to_string());
                }
                if options.include_compositions {
                    if let Some(coordinate) = self.row_coordinate(row).unwrap_or(None) {
                        values.extend(
                            coordinate.as_values()[..dimensions]
                                .iter()
                                .map(|value| format_number(*value, options.precision)),
                        );
                    } else {
                        values.resize(values.len() + dimensions, String::new());
                    }
                }
                let scalars = self.scalar_values(row).unwrap_or_default();
                for field in &selected {
                    let index = self
                        .fields()
                        .iter()
                        .position(|current| current.id == field.id)
                        .unwrap_or(usize::MAX);
                    values.push(
                        scalars
                            .get(index)
                            .and_then(|value| *value)
                            .map(|value| format_number(value, options.precision))
                            .unwrap_or_default(),
                    );
                }
                values
            })
            .collect();
        Ok(ClipboardTable {
            headers: options.include_headers.then_some(headers),
            rows,
        })
    }
    pub fn write_tsv<W: Write>(
        &self,
        mut writer: W,
        options: &GridExportOptions,
    ) -> Result<(), GridError> {
        writer
            .write_all(
                self.table(options)?
                    .to_tsv(options.include_headers)
                    .as_bytes(),
            )
            .map_err(|error| GridError::Io {
                message: error.to_string(),
            })
    }
    pub fn read_tsv<R: Read>(
        mut reader: R,
        space: GridCoordinateSpace,
        has_headers: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GridError> {
        let mut input = String::new();
        reader
            .read_to_string(&mut input)
            .map_err(|error| GridError::Io {
                message: error.to_string(),
            })?;
        let table = ClipboardTable::parse_tsv(&input, has_headers);
        let mut grid = if space.is_local() {
            Self::Irregular(IrregularCompositionGrid::local_ternary(
                "Imported data",
                space,
            )?)
        } else {
            Self::Irregular(IrregularCompositionGrid::tetrahedral("Imported data"))
        };
        if let Some(headers) = &table.headers {
            let dimension = if space.is_local() { 3 } else { 4 };
            let mut seen = BTreeMap::<String, String>::new();
            for header in headers {
                let normalized = header.trim().to_ascii_lowercase();
                if normalized == "row"
                    || normalized == "row id"
                    || resolve_component(header, dimension).is_some()
                {
                    continue;
                }
                let (name, units) = split_field_header(header);
                if name.is_empty() {
                    continue;
                }
                if let Some(prior) = seen.insert(name.to_ascii_lowercase(), header.clone()) {
                    return Err(GridError::Tsv {
                        message: format!("ambiguous scalar headers '{}' and '{}'", prior, header),
                    });
                }
                let field = grid.add_scalar_field(name);
                grid.set_scalar_units(field, units)?;
            }
        }
        grid.append_clipboard(&table, tolerance)?;
        Ok(grid)
    }
    /// Pastes the first column into one scalar field, beginning at a stable row ID.
    pub fn paste_scalar_column(
        &mut self,
        field: ScalarFieldId,
        start: GridRowId,
        table: &ClipboardTable,
    ) -> Result<PasteSummary, GridError> {
        let row_ids = self.row_ids();
        let start_index = row_ids
            .iter()
            .position(|id| *id == start)
            .ok_or(GridError::UnknownGridRow { id: start.get() })?;
        if !self.fields().iter().any(|candidate| candidate.id == field) {
            return Err(GridError::UnknownScalarField { id: field.get() });
        }
        let mut summary = PasteSummary {
            inserted: 0,
            updated: 0,
            invalid: 0,
            valid: 0,
            incomplete: 0,
            warnings: Vec::new(),
        };
        for (offset, values) in table.rows.iter().enumerate() {
            let Some(row) = row_ids.get(start_index + offset).copied() else {
                summary
                    .warnings
                    .push("paste extends beyond the grid".to_owned());
                break;
            };
            if values.len() > 1 {
                summary
                    .warnings
                    .push(format!("row {} contains extra columns", offset + 1));
            }
            let text = values.first().map(String::as_str).unwrap_or("").trim();
            let value = if text.is_empty() {
                None
            } else {
                match text.parse::<f64>() {
                    Ok(value) if value.is_finite() => Some(value),
                    _ => {
                        summary.invalid += 1;
                        continue;
                    }
                }
            };
            self.set_scalar(row, field, value)?;
            summary.updated += 1;
            summary.valid += 1;
        }
        Ok(summary)
    }
    pub fn append_clipboard(
        &mut self,
        table: &ClipboardTable,
        tolerance: Tolerance,
    ) -> Result<PasteSummary, GridError> {
        match self {
            Self::Regular(grid) => paste_regular(grid, table),
            Self::Irregular(grid) => paste_irregular(grid, table, tolerance),
        }
    }
    pub fn is_regular(&self) -> bool {
        matches!(self, Self::Regular(_))
    }
    pub fn component_count(&self) -> usize {
        if self.coordinate_space().is_local() {
            3
        } else {
            4
        }
    }
    pub fn entry_mode(&self) -> CompositionEntryMode {
        match self {
            Self::Regular(grid) => grid.entry_mode(),
            Self::Irregular(grid) => grid.entry_mode(),
        }
    }
    pub fn set_entry_mode(&mut self, mode: CompositionEntryMode) -> Result<(), GridError> {
        match self {
            Self::Regular(grid) => grid.set_entry_mode(mode),
            Self::Irregular(grid) => grid.set_entry_mode(mode),
        }
    }
    pub fn duplicate_policy(&self) -> DuplicateCompositionPolicy {
        match self {
            Self::Regular(_) => DuplicateCompositionPolicy::Allow,
            Self::Irregular(grid) => grid.duplicate_policy(),
        }
    }
    pub fn set_duplicate_policy(&mut self, policy: DuplicateCompositionPolicy) {
        if let Self::Irregular(grid) = self {
            grid.set_duplicate_policy(policy);
        }
    }
    pub fn set_name(&mut self, name: impl Into<String>) {
        match self {
            Self::Regular(grid) => grid.set_name(name),
            Self::Irregular(grid) => grid.set_name(name),
        }
    }
    pub fn coordinate_revision(&self) -> u64 {
        match self {
            Self::Regular(grid) => grid.coordinate_revision(),
            Self::Irregular(grid) => grid.coordinate_revision(),
        }
    }
    pub fn scalar_revision(&self) -> u64 {
        match self {
            Self::Regular(grid) => grid.scalar_revision(),
            Self::Irregular(grid) => grid.scalar_revision(),
        }
    }
    pub fn structure_revision(&self) -> u64 {
        match self {
            Self::Regular(grid) => grid.structure_revision(),
            Self::Irregular(grid) => grid.structure_revision(),
        }
    }
    pub fn columns(&self) -> Vec<crate::GridColumn> {
        let dimension = self.component_count();
        let dependent = match self.entry_mode() {
            CompositionEntryMode::AllComponents => None,
            CompositionEntryMode::DependentComponent(component) => Some(component.index()),
        };
        let mut columns = vec![crate::GridColumn {
            id: GridColumnId::new(0),
            label: "Row".to_owned(),
            kind: crate::GridColumnKind::RowId,
            editable: false,
        }];
        for index in 0..dimension {
            let is_dependent = dependent == Some(index);
            let kind = if dimension == 4 {
                crate::GridColumnKind::Component {
                    component: Component::ALL[index],
                    dependent: is_dependent,
                }
            } else {
                crate::GridColumnKind::LocalComponent {
                    component: index,
                    dependent: is_dependent,
                }
            };
            columns.push(crate::GridColumn {
                id: GridColumnId::new(1 + index as u64),
                label: component_headers(dimension)[index].to_owned(),
                kind,
                editable: matches!(self, Self::Irregular(_)) && !is_dependent,
            });
        }
        for field in self.fields() {
            let label = match field.units() {
                Some(units) if !units.is_empty() => format!("{} [{}]", field.name(), units),
                _ => field.name().to_owned(),
            };
            columns.push(crate::GridColumn {
                id: GridColumnId::new(1_000_000 + field.id().get()),
                label,
                kind: crate::GridColumnKind::Scalar { field: field.id() },
                editable: true,
            });
        }
        columns.push(crate::GridColumn {
            id: GridColumnId::new(u64::MAX),
            label: "Validation".to_owned(),
            kind: crate::GridColumnKind::Validation,
            editable: false,
        });
        columns
    }
    pub fn component_text(&self, row: GridRowId, component: usize) -> Result<String, GridError> {
        let dimension = self.component_count();
        if component >= dimension {
            return Err(GridError::UnknownColumn { column: component });
        }
        match self {
            Self::Regular(grid) => {
                let coordinate = grid.points[grid.row_index(row)?].coordinate;
                Ok(format_number(coordinate.as_values()[component], 6))
            }
            Self::Irregular(grid) => {
                let row = &grid.rows[grid.row_index(row)?];
                if row.raw_components.len() >= dimension {
                    return Ok(row.raw_components[component].clone());
                }
                let dependent = match grid.entry_mode {
                    CompositionEntryMode::DependentComponent(value) => Some(value.index()),
                    CompositionEntryMode::AllComponents => None,
                };
                if dependent == Some(component) {
                    return Ok(row
                        .coordinate
                        .map(|coordinate| format_number(coordinate.as_values()[component], 6))
                        .unwrap_or_default());
                }
                let compact =
                    component - usize::from(dependent.is_some_and(|value| value < component));
                Ok(row.raw_components.get(compact).cloned().unwrap_or_default())
            }
        }
    }
    pub fn set_component_text(
        &mut self,
        row: GridRowId,
        component: usize,
        text: impl Into<String>,
        tolerance: Tolerance,
    ) -> Result<(), GridError> {
        let dimension = self.component_count();
        if component >= dimension {
            return Err(GridError::UnknownColumn { column: component });
        }
        let dependent = match self.entry_mode() {
            CompositionEntryMode::DependentComponent(value) => Some(value.index()),
            CompositionEntryMode::AllComponents => None,
        };
        if dependent == Some(component) {
            return Err(GridError::ReadOnlyColumn { column: component });
        }
        if matches!(self, Self::Regular(_)) {
            return Err(GridError::ReadOnlyColumn { column: component });
        }
        let independent: Vec<_> = (0..dimension)
            .filter(|index| dependent != Some(*index))
            .collect();
        let mut components = independent
            .iter()
            .map(|index| self.component_text(row, *index))
            .collect::<Result<Vec<_>, _>>()?;
        let position = independent
            .iter()
            .position(|index| *index == component)
            .ok_or(GridError::ReadOnlyColumn { column: component })?;
        components[position] = text.into();
        match self {
            Self::Irregular(grid) => grid.set_raw_components(row, components, tolerance),
            Self::Regular(_) => unreachable!(),
        }
    }
    pub fn scalar_text(&self, row: GridRowId, field: ScalarFieldId) -> Result<String, GridError> {
        let index = self
            .fields()
            .iter()
            .position(|candidate| candidate.id() == field)
            .ok_or(GridError::UnknownScalarField { id: field.get() })?;
        let precision = self.fields()[index].precision();
        Ok(self
            .scalar_values(row)?
            .get(index)
            .copied()
            .flatten()
            .map(|value| format_number(value, precision))
            .unwrap_or_default())
    }
    pub fn cell_text(&self, row: GridRowId, column: GridColumnId) -> Result<String, GridError> {
        let descriptor = self
            .columns()
            .into_iter()
            .find(|candidate| candidate.id == column)
            .ok_or(GridError::UnknownColumn {
                column: column.get() as usize,
            })?;
        match descriptor.kind {
            crate::GridColumnKind::RowId => Ok(row.get().to_string()),
            crate::GridColumnKind::Component { component, .. } => {
                self.component_text(row, component.index())
            }
            crate::GridColumnKind::LocalComponent { component, .. } => {
                self.component_text(row, component)
            }
            crate::GridColumnKind::Scalar { field } => self.scalar_text(row, field),
            crate::GridColumnKind::Validation => Ok(self.validation_text(row)?),
        }
    }
    pub fn validation_text(&self, row: GridRowId) -> Result<String, GridError> {
        match self {
            Self::Regular(grid) => {
                grid.row_index(row)?;
                Ok("valid".to_owned())
            }
            Self::Irregular(grid) => {
                let row = &grid.rows[grid.row_index(row)?];
                if row.validation.is_empty() {
                    Ok("valid".to_owned())
                } else {
                    Ok(row
                        .validation
                        .iter()
                        .map(|issue| format!("{issue:?}"))
                        .collect::<Vec<_>>()
                        .join("; "))
                }
            }
        }
    }
    pub fn append_empty_row(&mut self, tolerance: Tolerance) -> Result<GridRowId, GridError> {
        match self {
            Self::Irregular(grid) => {
                let count = if matches!(grid.entry_mode(), CompositionEntryMode::AllComponents) {
                    grid.dimension()
                } else {
                    grid.dimension() - 1
                };
                Ok(grid.append_raw_row(vec![String::new(); count], tolerance))
            }
            Self::Regular(_) => Err(GridError::RegularRowsFixed),
        }
    }
    pub fn insert_empty_row(
        &mut self,
        index: usize,
        tolerance: Tolerance,
    ) -> Result<GridRowId, GridError> {
        match self {
            Self::Irregular(grid) => {
                let count = if matches!(grid.entry_mode(), CompositionEntryMode::AllComponents) {
                    grid.dimension()
                } else {
                    grid.dimension() - 1
                };
                grid.insert_raw_row(index, vec![String::new(); count], tolerance)
            }
            Self::Regular(_) => Err(GridError::RegularRowsFixed),
        }
    }
    pub fn delete_rows(&mut self, rows: &[GridRowId]) -> Result<usize, GridError> {
        match self {
            Self::Irregular(grid) => {
                let mut removed = 0;
                for row in rows {
                    if grid.delete_row(*row).is_some() {
                        removed += 1;
                    }
                }
                Ok(removed)
            }
            Self::Regular(_) => Err(GridError::RegularRowsFixed),
        }
    }
    pub fn rename_scalar_field(
        &mut self,
        field: ScalarFieldId,
        name: impl Into<String>,
    ) -> Result<(), GridError> {
        self.field_mut(field)?.set_name(name);
        Ok(())
    }
    pub fn set_scalar_units(
        &mut self,
        field: ScalarFieldId,
        units: Option<String>,
    ) -> Result<(), GridError> {
        self.field_mut(field)?.set_units(units);
        Ok(())
    }
    pub fn set_scalar_precision(
        &mut self,
        field: ScalarFieldId,
        precision: usize,
    ) -> Result<(), GridError> {
        self.field_mut(field)?.set_precision(precision.min(15));
        Ok(())
    }
    pub fn clear_scalar_field(&mut self, field: ScalarFieldId) -> Result<(), GridError> {
        let rows = self.row_ids();
        for row in rows {
            self.set_scalar(row, field, None)?;
        }
        Ok(())
    }
    pub fn selected_table(
        &self,
        state: &crate::DataTableState,
        include_headers: bool,
    ) -> Result<ClipboardTable, GridError> {
        let Some((row_start, row_end, column_start, column_end)) = state.selection_bounds() else {
            return Ok(ClipboardTable::default());
        };
        let selected_columns = &state.columns[column_start..=column_end];
        let rows = state.row_order[row_start..=row_end]
            .iter()
            .map(|row| {
                selected_columns
                    .iter()
                    .map(|column| {
                        let address = crate::GridCellAddress {
                            row: *row,
                            column: column.id,
                        };
                        state
                            .invalid_text(address)
                            .map(str::to_owned)
                            .map(Ok)
                            .unwrap_or_else(|| self.cell_text(*row, column.id))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ClipboardTable {
            headers: include_headers.then(|| {
                selected_columns
                    .iter()
                    .map(|column| column.label.clone())
                    .collect()
            }),
            rows,
        })
    }
    fn field_mut(&mut self, field: ScalarFieldId) -> Result<&mut ScalarField, GridError> {
        match self {
            Self::Regular(grid) => grid.field_mut(field),
            Self::Irregular(grid) => grid.field_mut(field),
        }
        .ok_or(GridError::UnknownScalarField { id: field.get() })
    }
    pub fn assign_id(&mut self, id: CompositionGridId) {
        match self {
            Self::Regular(grid) => grid.assign_id(id),
            Self::Irregular(grid) => grid.assign_id(id),
        }
    }
}

fn paste_regular(
    grid: &mut RegularCompositionGrid,
    table: &ClipboardTable,
) -> Result<PasteSummary, GridError> {
    let field_map = map_scalar_columns(table.headers.as_deref(), &grid.fields, grid.dimension());
    let mut summary = PasteSummary {
        inserted: 0,
        updated: 0,
        invalid: 0,
        valid: 0,
        incomplete: 0,
        warnings: table.shape_warnings(),
    };
    for (row_index, row) in table.rows.iter().enumerate() {
        if row_index >= grid.points.len() {
            summary
                .warnings
                .push(format!("row {} is outside the regular grid", row_index + 1));
            continue;
        }
        for (input, field_index) in row.iter().zip(field_map.iter()) {
            if let Some(field_index) = field_index {
                let value = if input.trim().is_empty() {
                    None
                } else {
                    match input.parse::<f64>() {
                        Ok(value) if value.is_finite() => Some(value),
                        _ => {
                            summary.invalid += 1;
                            continue;
                        }
                    }
                };
                grid.fields[*field_index].set_value(row_index, value)?;
                summary.updated += 1;
                summary.valid += 1;
            }
        }
    }
    if summary.updated > 0 {
        grid.scalar_revision += 1;
    }
    Ok(summary)
}

fn paste_irregular(
    grid: &mut IrregularCompositionGrid,
    table: &ClipboardTable,
    tolerance: Tolerance,
) -> Result<PasteSummary, GridError> {
    let dimension = grid.dimension();
    let component_map = map_component_columns(table.headers.as_deref(), dimension);
    let scalar_map = map_scalar_columns(table.headers.as_deref(), &grid.fields, dimension);
    let mut summary = PasteSummary {
        inserted: 0,
        updated: 0,
        invalid: 0,
        valid: 0,
        incomplete: 0,
        warnings: table.shape_warnings(),
    };
    for row in &table.rows {
        let mut components = vec![String::new(); dimension];
        let mut scalar_inputs: Vec<(usize, String)> = Vec::new();
        for (column, value) in row.iter().enumerate() {
            if let Some(component) = component_map.get(column).and_then(|value| *value) {
                components[component] = value.clone();
            } else if let Some(field) = scalar_map.get(column).and_then(|value| *value) {
                scalar_inputs.push((field, value.clone()));
            } else {
                summary.warnings.push(format!(
                    "unrecognized column {} retained as warning",
                    column + 1
                ));
            }
        }
        let id = grid.append_raw_row(components, tolerance);
        let row_index = grid.row_index(id)?;
        for (field, value) in scalar_inputs {
            let parsed = if value.trim().is_empty() {
                None
            } else {
                value.parse::<f64>().ok().filter(|value| value.is_finite())
            };
            if !value.trim().is_empty() && parsed.is_none() {
                summary.invalid += 1;
            } else {
                grid.fields[field].set_value(row_index, parsed)?;
                summary.updated += 1;
            }
        }
        summary.inserted += 1;
    }
    let validation = grid.validation_summary();
    summary.valid = validation.valid;
    summary.invalid += validation.invalid;
    summary.incomplete += validation.incomplete;
    Ok(summary)
}

fn subdivisions(definition: RegularGridDefinition) -> u32 {
    match definition {
        RegularGridDefinition::Tetrahedral(definition) => definition.subdivisions,
        RegularGridDefinition::LocalTernary(definition) => definition.subdivisions,
    }
}
fn validate_entry_mode(mode: CompositionEntryMode, dimension: usize) -> Result<(), GridError> {
    if let CompositionEntryMode::DependentComponent(component) = mode
        && component.index() >= dimension
    {
        return Err(GridError::InvalidComposition {
            message: "dependent component is outside this coordinate space".to_owned(),
        });
    }
    Ok(())
}
fn component_headers(dimension: usize) -> &'static [&'static str] {
    if dimension == 3 {
        &["u", "v", "w"]
    } else {
        &["A", "B", "C", "D"]
    }
}
fn format_number(value: f64, precision: usize) -> String {
    format!("{value:.precision$}")
}
fn select_fields<'a>(
    fields: &'a [ScalarField],
    requested: &[ScalarFieldId],
) -> Vec<&'a ScalarField> {
    if requested.is_empty() {
        fields.iter().collect()
    } else {
        fields
            .iter()
            .filter(|field| requested.contains(&field.id))
            .collect()
    }
}
fn map_component_columns(headers: Option<&[String]>, dimension: usize) -> Vec<Option<usize>> {
    headers
        .map(|headers| {
            headers
                .iter()
                .map(|header| resolve_component(header, dimension))
                .collect()
        })
        .unwrap_or_else(|| (0..dimension).map(Some).collect())
}
fn split_field_header(header: &str) -> (String, Option<String>) {
    let header = header.trim();
    if let Some(open) = header.rfind('[')
        && header.ends_with(']')
    {
        let name = header[..open].trim().to_owned();
        let units = header[open + 1..header.len() - 1].trim().to_owned();
        return (name, (!units.is_empty()).then_some(units));
    }
    (header.to_owned(), None)
}

fn map_scalar_columns(
    headers: Option<&[String]>,
    fields: &[ScalarField],
    component_offset: usize,
) -> Vec<Option<usize>> {
    headers
        .map(|headers| {
            headers
                .iter()
                .map(|header| {
                    let (name, units) = split_field_header(header);
                    let matches: Vec<_> = fields
                        .iter()
                        .enumerate()
                        .filter(|(_, field)| {
                            field.name.eq_ignore_ascii_case(&name)
                                && units.as_ref().is_none_or(|units| {
                                    field.units().is_some_and(|field_units| {
                                        field_units.eq_ignore_ascii_case(units)
                                    })
                                })
                        })
                        .map(|(index, _)| index)
                        .collect();
                    matches
                        .as_slice()
                        .first()
                        .copied()
                        .filter(|_| matches.len() == 1)
                })
                .collect()
        })
        .unwrap_or_else(|| {
            let mut columns = vec![None; component_offset];
            columns.extend((0..fields.len()).map(Some));
            columns
        })
}
fn resolve_component(header: &str, dimension: usize) -> Option<usize> {
    let normalized = header.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "a" | "u" if dimension >= 1 => Some(0),
        "b" | "v" if dimension >= 2 => Some(1),
        "c" | "w" if dimension >= 3 => Some(2),
        "d" if dimension == 4 => Some(3),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CoordinateKey([i64; 4]);
impl From<GridCoordinate> for CoordinateKey {
    fn from(value: GridCoordinate) -> Self {
        Self(
            value
                .as_values()
                .map(|value| (value * 1_000_000_000.0).round() as i64),
        )
    }
}

fn parse_coordinate(
    raw: &[String],
    dimension: usize,
    entry_mode: CompositionEntryMode,
    tolerance: Tolerance,
    issues: &mut Vec<GridValidationIssue>,
) -> Option<GridCoordinate> {
    let dependent = match entry_mode {
        CompositionEntryMode::AllComponents => None,
        CompositionEntryMode::DependentComponent(component) => Some(component.index()),
    };
    let minimum = if dependent.is_some() {
        dimension - 1
    } else {
        dimension
    };
    if raw.len() < minimum {
        issues.push(GridValidationIssue::MissingComponent { column: raw.len() });
        return None;
    }
    // A complete four/three-column row keeps canonical component positions.
    // An independent-only row is compact, in the remaining canonical order.
    let full_row = raw.len() >= dimension;
    let mut values = vec![None; dimension];
    let mut compact = 0usize;
    for (column, slot) in values.iter_mut().enumerate() {
        if Some(column) == dependent && !full_row {
            continue;
        }
        let source = if full_row { column } else { compact };
        let text = raw.get(source).map(String::as_str).unwrap_or("").trim();
        if text.is_empty() {
            if Some(column) != dependent || full_row {
                issues.push(GridValidationIssue::MissingComponent { column });
            }
        } else {
            match text.parse::<f64>() {
                Ok(value) if value.is_finite() => *slot = Some(value),
                Ok(_) => issues.push(GridValidationIssue::NonFiniteComponent { column }),
                Err(_) => issues.push(GridValidationIssue::InvalidNumber {
                    column,
                    value: text.to_owned(),
                }),
            }
        }
        if !full_row {
            compact += 1;
        }
    }
    if let Some(dependent) = dependent
        && values
            .iter()
            .enumerate()
            .all(|(index, value)| index == dependent || value.is_some())
    {
        let derived = 1.0
            - values
                .iter()
                .enumerate()
                .filter_map(|(index, value)| {
                    (index != dependent).then_some(value.unwrap_or_default())
                })
                .sum::<f64>();
        if derived < -tolerance.absolute {
            issues.push(GridValidationIssue::NegativeDependentComponent {
                component: dependent,
            });
        } else if let Some(provided) = values[dependent] {
            if !tolerance.is_close(provided, derived) {
                issues.push(GridValidationIssue::SumMismatch {
                    actual: provided.to_string(),
                });
            }
        } else {
            values[dependent] = Some(derived.max(0.0));
        }
    }
    for (column, value) in values.iter().enumerate() {
        if value.is_none() && !issues.iter().any(|issue| matches!(issue, GridValidationIssue::MissingComponent { column: prior } if *prior == column)) {
            issues.push(GridValidationIssue::MissingComponent { column });
        }
    }
    if !issues.is_empty() {
        return None;
    }
    let values: Vec<f64> = values.into_iter().map(Option::unwrap).collect();
    if values.iter().enumerate().any(|(column, value)| {
        if *value < -tolerance.absolute {
            issues.push(GridValidationIssue::NegativeComponent { column });
            true
        } else {
            false
        }
    }) {
        return None;
    }
    let sum: f64 = values.iter().sum();
    if !tolerance.is_close(sum, 1.0) {
        issues.push(GridValidationIssue::SumMismatch {
            actual: sum.to_string(),
        });
        return None;
    }
    if dimension == 4 {
        Some(GridCoordinate::Tetrahedral(TetraPoint::from_unchecked([
            values[0].max(0.0),
            values[1].max(0.0),
            values[2].max(0.0),
            values[3].max(0.0),
        ])))
    } else {
        Some(GridCoordinate::LocalTernary(TernaryPoint::from_unchecked(
            [values[0].max(0.0), values[1].max(0.0), values[2].max(0.0)],
        )))
    }
}
fn apply_duplicates(
    rows: &mut [IrregularGridRow],
    policy: DuplicateCompositionPolicy,
    tolerance: Tolerance,
) {
    if matches!(policy, DuplicateCompositionPolicy::Allow) {
        return;
    }
    let mut seen: Vec<(GridRowId, GridCoordinate)> = Vec::new();
    for row in rows {
        if let Some(coordinate) = row.coordinate {
            if let Some((id, _)) = seen
                .iter()
                .find(|(_, prior)| coordinates_close(*prior, coordinate, tolerance))
            {
                match policy {
                    DuplicateCompositionPolicy::Warn => row
                        .validation
                        .push(GridValidationIssue::DuplicateComposition { other: *id }),
                    DuplicateCompositionPolicy::Reject => {
                        row.coordinate = None;
                        row.validation
                            .push(GridValidationIssue::DuplicateCompositionRejected { other: *id });
                    }
                    DuplicateCompositionPolicy::Allow => {}
                }
            } else {
                seen.push((row.id, coordinate));
            }
        }
    }
}
fn coordinates_close(left: GridCoordinate, right: GridCoordinate, tolerance: Tolerance) -> bool {
    left.dimension() == right.dimension()
        && left
            .as_values()
            .iter()
            .zip(right.as_values())
            .take(left.dimension())
            .all(|(left, right)| tolerance.is_close(*left, right))
}
fn summarize_rows(rows: &[IrregularGridRow]) -> GridValidationSummary {
    let mut summary = GridValidationSummary::default();
    for row in rows {
        if row.coordinate.is_some() && row.validation.is_empty() {
            summary.valid += 1;
        } else if row
            .validation
            .iter()
            .all(|issue| matches!(issue, GridValidationIssue::MissingComponent { .. }))
        {
            summary.incomplete += 1;
        } else {
            summary.invalid += 1;
        }
        summary.warnings += row
            .validation
            .iter()
            .filter(|issue| !issue.is_error())
            .count();
    }
    summary
}
