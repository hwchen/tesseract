use serde_derive::{Serialize, Deserialize};
use std::convert::From;
use anyhow::{Error, format_err};

pub mod aggregator;
pub mod metadata;
mod json;
mod xml;

const DEFAULT_LOCALE_STR: &str = "en";


pub use crate::schema::{
    json::SchemaConfigJson,
    json::DimensionConfigJson,
    json::HierarchyConfigJson,
    json::LevelConfigJson,
    json::MeasureConfigJson,
    json::TableConfigJson,
    json::PropertyConfigJson,
    json::AnnotationConfigJson,
    json::InlineTableJson,
    json::InlineTableColumnDefinitionJson,
    json::InlineTableRowJson,
    json::InlineTableRowValueJson,
    xml::SchemaConfigXML,
    xml::DimensionConfigXML,
    xml::HierarchyConfigXML,
    xml::LevelConfigXML,
    xml::MeasureConfigXML,
    xml::TableConfigXML,
    xml::PropertyConfigXML,
};
use crate::names::{LevelName, Measure as MeasureName, Property as TsProperty};
use crate::query_ir::MemberType;
pub use self::aggregator::Aggregator;
use crate::DEFAULT_ALLOWED_ACCESS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub cubes: Vec<Cube>,
    pub annotations: Option<Vec<Annotation>>,
    pub default_locale: String,
}

impl From<SchemaConfigJson> for Schema {
    fn from(schema_config: SchemaConfigJson) -> Self {
        // TODO
        // check for:
        // - duplicate cube names,
        // - duplicate dim names

        let mut cubes = vec![];

        for cube_config in schema_config.cubes {
            let mut dimensions: Vec<_> = cube_config.dimensions
                .unwrap_or(vec![])
                .into_iter()
                .map(|dim| dim.into())
                .collect();
            let measures = cube_config.measures.into_iter()
                .map(|mea| mea.into())
                .collect();
            let cube_annotations = cube_config.annotations
                .map(|anns| {
                    anns.into_iter()
                        .map(|ann| ann.into())
                        .collect()
                });

            // special case: check for dimension_usages
            //
            // If source field only, use the shared dim name as the name.
            // if optional name field present, use that for name.
            // validation that all dimensions have different names will happen
            // in validate method
            if let Some(dim_usages) = cube_config.dimension_usages {
                for dim_usage in dim_usages {
                    // prep annotations to be merged with shared dim annotations
                    let dim_usage_annotations: Option<Vec<Annotation>> = dim_usage.annotations
                        .map(|anns| {
                            anns.into_iter()
                                .map(|ann| ann.into())
                                .collect()
                        });

                    if let Some(ref shared_dims) = schema_config.shared_dimensions {
                        for shared_dim_config in shared_dims {
                            let dim_name = dim_usage.name.as_ref().unwrap_or(&dim_usage.source);

                            if dim_usage.source == shared_dim_config.name {
                                let hierarchies = shared_dim_config.hierarchies.iter()
                                    .map(|h| h.clone().into())
                                    .collect();
                                let shared_dim_annotations: Option<Vec<Annotation>> = shared_dim_config.annotations.as_ref()
                                    .map(|anns| {
                                        anns.into_iter()
                                            .map(|ann| ann.clone().into())
                                            .collect()
                                    });
                                let dim_annotations = shared_dim_annotations
                                    .and_then(|shared_dim_anns| {
                                        dim_usage_annotations.as_ref().map(|dim_usage_anns| {
                                            let mut dim_anns = shared_dim_anns.clone();
                                            dim_anns.extend_from_slice(&dim_usage_anns);
                                            dim_anns
                                        })
                                    });

                                let dim_type = shared_dim_config.dim_type.clone().unwrap_or(DimensionType::default());

                                dimensions.push(Dimension {
                                    name: dim_name.clone(),
                                    foreign_key: Some(dim_usage.foreign_key.clone()),
                                    hierarchies,
                                    default_hierarchy: shared_dim_config.default_hierarchy.clone(),
                                    dim_type,
                                    annotations: dim_annotations,
                                    is_shared: true
                                });
                            }
                        }
                    }
                }
            }

            // Cubes are public by default
            let public = match cube_config.public {
                Some(public) => public != "false",
                None => true
            };

            let min_auth_level = cube_config.min_auth_level.unwrap_or(DEFAULT_ALLOWED_ACCESS);

            cubes.push(Cube {
                name: cube_config.name,
                public,
                min_auth_level,
                table: cube_config.table.into(),
                can_aggregate: false,
                dimensions,
                measures,
                annotations: cube_annotations,
            });
        }

        let schema_annotations = schema_config.annotations
            .map(|anns| {
                anns.into_iter()
                    .map(|ann| ann.into())
                    .collect()
            });

        Schema {
            name: schema_config.name,
            cubes,
            annotations: schema_annotations,
            default_locale: schema_config.default_locale.unwrap_or_else(|| DEFAULT_LOCALE_STR.to_owned()),
        }
    }
}

/// No `From<CubeConfig>` because the transition needs to be made at Schema
/// level in order to take into account shared dims.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cube {
    pub name: String,
    pub public: bool,
    pub min_auth_level: i32,
    pub table: Table,
    pub can_aggregate: bool,
    pub dimensions: Vec<Dimension>,
    pub measures: Vec<Measure>,
    pub annotations: Option<Vec<Annotation>>,
}

impl Cube {
    /// Returns a Vec<String> of all the dimension name options for a given Cube.
    pub fn get_all_level_names(&self) -> Vec<LevelName> {
        let mut dimension_names: Vec<LevelName> = vec![];

        for dimension in &self.dimensions {
            let dimension_name = dimension.name.clone();
            for hierarchy in &dimension.hierarchies {
                let hierarchy_name = hierarchy.name.clone();
                for level in &hierarchy.levels {
                    let level_name = level.name.clone();
                    dimension_names.push(
                        LevelName {
                            dimension: dimension_name.clone(),
                            hierarchy: hierarchy_name.clone(),
                            level: level_name,
                        }
                    );
                }
            }
        }

        dimension_names
    }

    /// Returns a Vec<String> of all the measure names for a given Cube.
    pub fn get_all_measure_names(&self) -> Vec<MeasureName> {
        let mut measure_names: Vec<MeasureName> = vec![];

        for measure in &self.measures {
            measure_names.push(
                MeasureName::new(measure.name.clone())
            );
        }

        measure_names
    }

    /// Finds the dimension and hierarchy names for a given level.
    /// Also returns the Level object matched.
    /// (it's the first level matched; for logic layer,
    /// it's assumed that all levels are unique)
    pub fn identify_level(&self, level_name: String) -> Result<(String, String, Level), Error> {
        for dimension in self.dimensions.clone() {
            for hierarchy in dimension.hierarchies.clone() {
                for level in hierarchy.levels.clone() {
                    if level.name == level_name {
                        return Ok((dimension.name, hierarchy.name, level))
                    }
                }
            }
        }

        Err(format_err!("'{}' not found", level_name))
    }

    /// gets parents levels (not including the level itself)
    /// (it's the first level matched; for logic layer,
    /// it's assumed that all levels are unique)
    pub fn get_level_parents(&self, level_name: &LevelName) -> Result<Vec<Level>, Error> {
        for dimension in &self.dimensions {
            if dimension.name == level_name.dimension {
                for hierarchy in &dimension.hierarchies {
                    if hierarchy.name == level_name.hierarchy {
                        for (level_idx, level) in hierarchy.levels.iter().enumerate() {
                            if level.name == level_name.level {
                                return Ok(hierarchy.levels.clone().into_iter().take(level_idx).collect())
                            }
                        }
                    }
                }
            }
        }

        Err(format_err!("'{}' not found", level_name))
    }

    /// Finds the dimension, hierarchy, and level names for a given property.
    pub fn identify_property(&self, property_name: String) -> Result<(String, String, String), Error> {
        for dimension in self.dimensions.clone() {
            for hierarchy in dimension.hierarchies.clone() {
                for level in hierarchy.levels.clone() {
                    match level.properties {
                        Some(props) => {
                            for property in props {
                                if property.name == property_name {
                                    return Ok((dimension.name, hierarchy.name, level.name))
                                }
                            }
                        },
                        None => continue
                    }
                }
            }
        }

        Err(format_err!("'{}' not found", property_name))
    }

    /// Returns a Hierarchy object corresponding to a provided LevelName.
    pub fn get_hierarchy(&self, level_name: &LevelName) -> Option<Hierarchy> {
        for dimension in &self.dimensions {
            if dimension.name == level_name.dimension {
                for hierarchy in &dimension.hierarchies {
                    if hierarchy.name == level_name.hierarchy {
                        return Some(hierarchy.clone())
                    }
                }
            }
        }
        None
    }

    /// Returns a Level object corresponding to a provided LevelName.
    pub fn get_level(&self, level_name: &LevelName) -> Option<Level> {
        for dimension in &self.dimensions {
            if dimension.name == level_name.dimension {
                for hierarchy in &dimension.hierarchies {
                    if hierarchy.name == level_name.hierarchy {
                        for level in &hierarchy.levels {
                            if level.name == level_name.level {
                                return Some(level.clone())
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn get_child_level(&self, level_name: &LevelName) -> Result<Option<Level>, Error> {
        let hierarchy = self.get_hierarchy(level_name)
            .ok_or_else(|| format_err!("Could not find parent hierarchy for level: {}", level_name.level))?;

        let mut child_level: Option<Level> = None;
        let mut is_next: bool = false;

        for level in &hierarchy.levels {
            if is_next {
                child_level = Some(level.clone());
                break;
            }

            if level.name == level_name.level {
                is_next = true;
                continue;
            }
        }

        Ok(child_level)
    }

    /// Returns a Dimension object corresponding to a provided LevelName.
    pub fn get_dimension(&self, level_name: &LevelName) -> Option<Dimension> {
        for dimension in &self.dimensions {
            if dimension.name == level_name.dimension {
                return Some(dimension.clone())
            }
        }
        None
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dimension {
    pub name: String,
    pub foreign_key: Option<String>,
    pub hierarchies: Vec<Hierarchy>,
    pub default_hierarchy: Option<String>,
    pub dim_type: DimensionType,
    pub annotations: Option<Vec<Annotation>>,
    pub is_shared: bool,
}

impl From<DimensionConfigJson> for Dimension {
    fn from(dimension_config: DimensionConfigJson) -> Self {
        let hierarchies = dimension_config.hierarchies.into_iter()
            .map(|h| h.into())
            .collect();
        let annotations = dimension_config.annotations
            .map(|anns| {
                anns.into_iter()
                    .map(|ann| ann.into())
                    .collect()
            });

        let dim_type = dimension_config.dim_type.unwrap_or(DimensionType::default());

        Dimension {
            name: dimension_config.name,
            foreign_key: dimension_config.foreign_key,
            default_hierarchy: dimension_config.default_hierarchy,
            hierarchies,
            dim_type,
            annotations,
            is_shared: false
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DimensionType {
    #[serde(rename="standard")]
    Standard,
    #[serde(rename="time")]
    Time,
    #[serde(rename="geo")]
    Geo,
}

impl std::default::Default for DimensionType {
    fn default() -> Self { DimensionType::Standard }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hierarchy {
    pub name: String,
    pub table: Option<Table>,
    pub primary_key: String,
    pub levels: Vec<Level>,
    pub annotations: Option<Vec<Annotation>>,
    pub inline_table: Option<InlineTable>,
    pub default_member: Option<String>,
}

impl From<HierarchyConfigJson> for Hierarchy {
    fn from(hierarchy_config: HierarchyConfigJson) -> Self {
        let levels: Vec<Level> = hierarchy_config.levels.into_iter()
            .map(|l| l.into())
            .collect();

        let annotations = hierarchy_config.annotations
            .map(|anns| {
                anns.into_iter()
                    .map(|ann| ann.into())
                    .collect()
            });

        let primary_key = hierarchy_config.primary_key
            .unwrap_or_else(|| {
                levels.iter()
                    .last()
                    .expect("TODO check that there's at least 1 level")
                    .key_column
                    .clone()
            });

        Hierarchy {
            name: hierarchy_config.name,
            table: hierarchy_config.table.map(|t| t.into()),
            primary_key,
            levels,
            annotations,
            inline_table: hierarchy_config.inline_table.map(|t| t.into()),
            default_member: hierarchy_config.default_member
        }
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineTable {
    pub alias: String,
    pub column_definitions: Vec<InlineTableColumnDefinition>,
    pub rows: Vec<InlineTableRow>,
}

impl InlineTable {
    /// Transforms an InlineTable object into a SQL string.
    pub fn sql_string(&self) -> String {
        let mut curr_sql = "".to_string();

        for (i, table_row) in self.rows.iter().enumerate() {
            curr_sql += &"select ".to_string();

            for (j, row) in table_row.row_values.iter().enumerate() {
                for col_def in self.column_definitions.iter() {
                    if col_def.name == row.column {
                        match col_def.key_type {
                            MemberType::Text => curr_sql += &format!("'{}'", row.value),
                            MemberType::NonText => match &col_def.key_column_type {
                                Some(t) => curr_sql += &format!("cast({} as {})", row.value, t),
                                None => curr_sql += &format!("{}", row.value)
                            }
                        }
                        break
                    }
                }

                if i == 0 {
                    curr_sql += &format!(" as {}", row.column);
                }

                if j < table_row.row_values.len() - 1 {
                    curr_sql += &", ".to_string();
                }
            }

            if i < self.rows.len() - 1 {
                curr_sql += &" union all ".to_string()
            }
        }

        curr_sql
    }
}

impl From<InlineTableJson> for InlineTable {
    fn from(inline_table_config: InlineTableJson) -> Self {
        InlineTable {
            alias: inline_table_config.alias,
            column_definitions: inline_table_config.column_definitions.into_iter()
                .map(|l| l.into())
                .collect(),
            rows: inline_table_config.rows.into_iter()
                .map(|l| l.into())
                .collect(),
        }
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineTableColumnDefinition {
    pub name: String,
    pub key_type: MemberType,
    pub key_column_type: Option<String>,
    pub caption_set: Option<String>,
}

impl From<InlineTableColumnDefinitionJson> for InlineTableColumnDefinition {
    fn from(column_definition_config: InlineTableColumnDefinitionJson) -> Self {
        InlineTableColumnDefinition {
            name: column_definition_config.name,
            key_type: column_definition_config.key_type,
            key_column_type: column_definition_config.key_column_type,
            caption_set: column_definition_config.caption_set,
        }
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineTableRow {
    pub row_values: Vec<InlineTableRowValue>,
}

impl From<InlineTableRowJson> for InlineTableRow {
    fn from(row_config: InlineTableRowJson) -> Self {
        InlineTableRow {
            row_values: row_config.row_values.into_iter()
                .map(|l| l.into())
                .collect(),
        }
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineTableRowValue {
    pub column: String,
    pub value: String,
}

impl From<InlineTableRowValueJson> for InlineTableRowValue {
    fn from(row_value_config: InlineTableRowValueJson) -> Self {
        InlineTableRowValue {
            column: row_value_config.column,
            value: row_value_config.value,
        }
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Level {
    pub name: String,
    pub key_column: String,
    pub name_column: Option<String>,
    pub properties: Option<Vec<Property>>,
    pub key_type: Option<MemberType>,
    pub annotations: Option<Vec<Annotation>>,
}

impl Level {
    pub fn get_captions(&self, level_name: &LevelName, locales: &Vec<String>) -> Vec<TsProperty> {
        let mut captions: Vec<TsProperty> = vec![];

        if let Some(props) = self.properties.clone() {
            for prop in props {
                if let Some(cap) = prop.caption_set {
                    for locale in locales.clone() {
                        if locale == cap {
                            captions.push(
                                TsProperty::new(
                                    level_name.dimension.clone(),
                                    level_name.hierarchy.clone(),
                                    level_name.level.clone(),
                                    prop.name.clone()
                                )
                            )
                        }
                    }
                }
            }
        }

        captions
    }
}

impl From<LevelConfigJson> for Level {
    fn from(level_config: LevelConfigJson) -> Self {
        let properties = level_config.properties
            .map(|ps| {
                ps.into_iter()
                    .map(|p| p.into())
                    .collect()
            });
        let annotations = level_config.annotations
            .map(|anns| {
                anns.into_iter()
                    .map(|ann| ann.into())
                    .collect()
            });

        Level {
            name: level_config.name,
            key_column: level_config.key_column,
            name_column: level_config.name_column,
            properties,
            key_type: level_config.key_type,
            annotations,
        }
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measure{
    pub name: String,
    pub column: String,
    pub aggregator: Aggregator,
    pub measure_type: MeasureType,
    pub annotations: Option<Vec<Annotation>>,
}

impl From<MeasureConfigJson> for Measure {
    fn from(measure_config: MeasureConfigJson) -> Self {
        let annotations = measure_config.annotations
            .map(|anns| {
                anns.into_iter()
                    .map(|ann| ann.into())
                    .collect()
            });

        Measure {
            name: measure_config.name,
            column: measure_config.column,
            aggregator: measure_config.aggregator,
            measure_type: measure_config.measure_type.unwrap_or_else(|| MeasureType::default()),
            annotations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MeasureType {
    #[serde(rename="standard")]
    Standard {
        units: Option<String>,
    },
    #[serde(rename="error")]
    Error {
        for_measure: String,
        err_type: String,
    },
}

impl Default for MeasureType {
    fn default() -> Self {
        MeasureType::Standard {
            units: None,
        }
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table{
    pub name: String,
    pub schema: Option<String>,
    pub primary_key: Option<String>,
}

impl From<TableConfigJson> for Table {
    fn from(table_config: TableConfigJson) -> Self {
        Table {
            name: table_config.name,
            schema: table_config.schema,
            primary_key: table_config.primary_key,
        }
    }
}

impl Table {
    pub fn full_name(&self) -> String {
        if let Some(ref schema) = self.schema {
            format!("{}.{}", schema, self.name)
        } else {
            self.name.to_owned()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Property {
    pub name: String,
    pub column: String,
    pub caption_set: Option<String>,
    pub annotations: Option<Vec<Annotation>>,
}

impl From<PropertyConfigJson> for Property {
    fn from(property_config: PropertyConfigJson) -> Self {
        let annotations = property_config.annotations
            .map(|anns| {
                anns.into_iter()
                    .map(|ann| ann.into())
                    .collect()
            });

        Property {
            name: property_config.name,
            column: property_config.column,
            caption_set: property_config.caption_set,
            annotations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation{
    pub name: String,
    pub text: String,
}

impl From<AnnotationConfigJson> for Annotation {
    fn from(annotation_config: AnnotationConfigJson) -> Self {
        Annotation {
            name: annotation_config.name,
            text: annotation_config.text,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::schema::{
        json::SchemaConfigJson,
        json::SharedDimensionConfigJson,
        json::DimensionUsageJson,
        json::HierarchyConfigJson,
        json::LevelConfigJson,
        json::TableConfigJson,
        json::CubeConfigJson,
    };

    #[test]
    fn test_dimension_usage() {
        let schema_config = SchemaConfigJson {
            default_locale: Some(DEFAULT_LOCALE_STR.into()),
            name: "test".into(),
            shared_dimensions: Some(vec![
                SharedDimensionConfigJson {
                    name: "geo".into(),
                    hierarchies: vec![
                        HierarchyConfigJson {
                            name: "geo".into(),
                            table: Some(TableConfigJson {
                                name: "geo_table".into(),
                                schema: None,
                                primary_key: None,
                            }),
                            primary_key: Some("geoid".into()),
                            levels: vec![
                                LevelConfigJson {
                                    name: "tract".into(),
                                    key_column: "geoid".into(),
                                    name_column: None,
                                    properties: None,
                                    key_type: None,
                                    annotations: None,
                                },
                            ],
                            annotations: None,
                            inline_table: None,
                            default_member: None,
                        },
                    ],
                    default_hierarchy: None,
                    annotations: None,
                    dim_type: None,
                }
            ]),
            cubes: vec![
                CubeConfigJson {
                    name: "test_cube".into(),
                    public: Some("true".into()),
                    min_auth_level: None,
                    table: TableConfigJson {
                        name: "fact_table".into(),
                        schema: None,
                        primary_key: None,
                    },
                    dimensions: Some(vec![]),
                    dimension_usages: Some(vec![
                        DimensionUsageJson {
                            source: "geo".into(),
                            name: Some("geo".into()),
                            foreign_key: "fact_geoid".into(),
                            annotations: None,
                        }
                    ]),
                    measures: vec![],
                    annotations: None,
                }
            ],
            annotations: None,
        };

        let schema: Schema = schema_config.into();
        println!("{:#?}", schema);
        assert_eq!(schema.cubes[0].dimensions.len(), 1);
    }

    // End to end, from xml
    use serde_xml_rs::from_reader;

    #[test]
    fn xml_to_config() {
        let s = r##"
            <Schema name="my_schema">
                <SharedDimension name="Geo">
                    <Hierarchy name="Geo">
                        <Level name="Tract" key_column="geoid" />
                    </Hierarchy>
                </SharedDimension>
                <Cube name="my_cube">
                    <Table name="my_table" />
                    <Dimension name="my_dim">
                        <Hierarchy name="my_hier">
                            <Level name="my_level" key_column="key" />
                        </Hierarchy>
                    </Dimension>
                    <Measure name="my_mea" column="mea" aggregator="sum" />
                </Cube>
            </Schema>
        "##;
        let xml_schema_config: SchemaConfigXML = from_reader(s.as_bytes()).unwrap();
        let json_str = serde_json::to_string(&xml_schema_config).unwrap();
        let json_schema_config: SchemaConfigJson = serde_json::from_str(&json_str).unwrap();
        println!("{:#?}", json_schema_config);
    }
}
