mod backend;
mod dataframe;
mod sql;
pub mod format;

// fix Stream later; it's kind of a pain for now.
//pub mod format_stream;

pub mod names;
pub mod schema;
pub mod query;
pub mod query_ir;

use anyhow::{Error, format_err, bail};
use log::*;
use serde_xml_rs as serde_xml;
use serde_xml::from_reader;
use std::collections::{HashSet, HashMap};
use std::str::FromStr;
use crate::schema::{SchemaConfigJson, SchemaConfigXML};

pub use self::backend::Backend;
pub use self::dataframe::{DataFrame, Column, ColumnData, is_same_columndata_type};

pub static DEFAULT_ALLOWED_ACCESS: i32 = 0;

use self::names::{
    Cut,
    Drilldown,
    Measure,
    Property,
    LevelName,
    Mask,
};
pub use self::schema::{Schema, Cube, Dimension, Table, Aggregator};
use self::schema::metadata::{SchemaMetadata, CubeMetadata};
use self::query_ir::{
    CutSql,
    DrilldownSql,
    MeasureSql,
    HiddenDrilldownSql,
    MemberType,
    TableSql,
    LevelColumn,
    TopSql,
    TopWhereSql,
    SortSql,
    RcaSql,
    GrowthSql,
    RateSql,
    FilterSql,
};
pub use self::query::{Query, MeaOrCalc, FilterQuery};
pub use self::query_ir::QueryIr;
macro_rules! mea_or_calc {
    ($m_or_c:expr, $query:expr) => {
        match $m_or_c {
            MeaOrCalc::Mea(m) => {
                $query.measures.iter()
                    .position(|col| col == m )
                    .map(|idx|{
                        let idx = if $query.rca.is_some() {
                            idx + 1
                        } else {
                            idx
                        };
                        format!("final_m{}", idx)})
                    .ok_or(format_err!("measure {} must be in measures or if sorting on RCA column use \"rca\"", m))
            },
            MeaOrCalc::Calc(c) => {
                Ok(c.sql_string())
            }
        }
    }
}

impl Schema {
    /// Deserializes JSON schema into a `Schema`.
    pub fn from_json(raw_schema: &str) -> Result<Self, Error> {
        let schema_config = serde_json::from_str::<SchemaConfigJson>(raw_schema)?;
        Ok(schema_config.into())
    }

    /// Deserializes XML schema into a `Schema`.
    pub fn from_xml(raw_schema: &str) -> Result<Self, Error> {
        let schema_config: SchemaConfigXML = match from_reader(raw_schema.as_bytes()) {
            Ok(schema_config_xml) => schema_config_xml,
            Err(err) => return Err(format_err!("Error reading XML schema: {}", err))
        };

        // Serialize XML to JSON as intermediary step
        let serialized = serde_json::to_string(&schema_config)?;
        Schema::from_json(&serialized)
    }

    /// schema validation
    pub fn validate(&mut self) -> Result<(), Error> {
        // There should be at least one dimension. Both dim and shared dim are optional,
        // so need to do a validation check here.

        for cube in &self.cubes {
            if cube.dimensions.is_empty() {
                bail!("Between Dimensions and Shared Dimensions, a cube must have a total of at least 1.");
            }
        };

        // There should be no duplicate dimension names in a cube
        for cube in &self.cubes {
            let set = cube.dimensions.iter()
                .map(|dim| {
                    &dim.name
                })
                .collect::<HashSet<_>>();

            if set.len() != cube.dimensions.len() {
                bail!("Duplicate dimension names not allowed");
            }
        };

        // if there's multiple hierarchies in a dim, there must be a default hierarchy.
        // also, the default hierarchy must match names with an actual hierarchy.
        //
        // Also, a single hierarchy should not have a default set
        //
        // This means that later, we can just check whether there is a default
        // hierarchy only, instead of also checking for hierarchy cardinality during
        // a request

        for cube in self.cubes.iter_mut() {
            for dim in cube.dimensions.iter_mut() {
                if dim.hierarchies.len() == 1 {
                    dim.default_hierarchy = None;
                } else if !dim.hierarchies.is_empty() {
                    let cube_name = &cube.name;
                    // first, default_hierarchy must be assigned
                    let default_hierarchy = dim.default_hierarchy
                        .clone()
                        .ok_or_else(|| format_err!("Default hierarchy required for multiple hierarchies in cube: {} dimension: {}", cube_name, &dim.name))?;

                    // if default_hierarchy exists, then check that it's in one of the
                    // hierarchies
                    let contains_default = dim.hierarchies.iter()
                        .map(|hier| &hier.name)
                        .any(|hier_name| *hier_name == default_hierarchy);

                    if !contains_default {
                        bail!("Default hierarchy must exist in multiple hierarchies");
                    }
                }
            }
        }

        Ok(())
    }

    pub fn cube_metadata(&self, cube_name: &str) -> Option<CubeMetadata> {
        // Takes the first cube with the name.
        // TODO we still have to check that the cube names are distinct
        // before this.
        self.cubes.iter().find(|c| c.name == cube_name).map(|c| c.into())
    }

    pub fn metadata(&self, user_auth_level: Option<i32>) -> SchemaMetadata {
        let mut schema_metadata: SchemaMetadata = self.into();
        if let Some(val) = user_auth_level {
            schema_metadata.cubes = schema_metadata.cubes.drain(..).filter(|c| val >= c.min_auth_level && val >= DEFAULT_ALLOWED_ACCESS).collect();
        }
        schema_metadata
    }

    pub fn has_unique_levels_properties(&self) -> CubeHasUniqueLevelsAndProperties {
        for cube in &self.cubes {
            let mut levels = HashSet::new();
            let mut properties = HashSet::new();

            for dimension in &cube.dimensions {
                for hierarchy in &dimension.hierarchies {

                    // Check each cube for unique level and property names
                    for level in &hierarchy.levels {
                        if !levels.insert(&level.name) {
                            info!(
                                "Found repeated level name: {}.{}.{}.{}",
                                cube.name, dimension.name, hierarchy.name, level.name
                            );
                            return CubeHasUniqueLevelsAndProperties::False {
                                cube: cube.name.clone(),
                                name: level.name.clone(),
                            };
                        }

                        if let Some(ref props) = level.properties {
                            for property in props {
                                if !properties.insert(&property.name) {
                                    info!(
                                        "Found repeated property name: {}.{}.{}.{}.{}",
                                        cube.name, dimension.name, hierarchy.name, level.name, property.name
                                    );
                                    return CubeHasUniqueLevelsAndProperties::False {
                                        cube: cube.name.clone(),
                                        name: property.name.clone(),
                                    };
                                }
                            }
                        }
                    }
                }
            }
        }

        CubeHasUniqueLevelsAndProperties::True
    }

    pub fn members_sql(
        &self,
        cube: &str,
        level_name: &LevelName,
        ) -> Result<(String, Vec<String>), Error> // Sql and then Header
    {
        let members_query_ir = self.get_dim_col_table(cube, level_name)?;

        let header = if members_query_ir.name_column.is_some() {
            vec!["ID".into(), "Label".into()]
        } else {
            vec!["ID".into()]
        };

        let name_col = if let Some(ref col) = members_query_ir.name_column {
           col.to_owned()
        } else {
            "".into()
        };

        let sql = format!("select distinct {}{}{} from {}",
            members_query_ir.key_column,
            if members_query_ir.name_column.is_some() { ", " } else { "" },
            name_col,
            members_query_ir.table_sql,
        );

        Ok((sql, header))
    }

    /// Generates SQL to resolve a members locale query.
    /// Supports resolving multiple locales at the same time.
    pub fn members_locale_sql(
        &self,
        cube_name: &str,
        level_name: &LevelName,
        locale: &str
    ) -> Result<(String, Vec<String>), Error> // Sql and then Header
    {
        let locales: Vec<String> = locale.split(",").map(|s| s.to_string()).collect();

        let cube = self.cubes.iter()
            .find(|cube| &cube.name == &cube_name)
            .ok_or(format_err!("Could not find cube"))?;

        let dim = cube.dimensions.iter()
            .find(|dim| dim.name == level_name.dimension)
            .ok_or(format_err!("could not find dimension for level name"))?;
        let hier = dim.hierarchies.iter()
            .find(|hier| hier.name == level_name.hierarchy)
            .ok_or(format_err!("could not find hierarchy for level name"))?;
        let level = hier.levels.iter()
            .find(|lvl| lvl.name == level_name.level)
            .ok_or(format_err!("could not find level for level name"))?;

        let table = hier.table.clone().unwrap_or_else(|| cube.table.clone());

        let key_column = level.key_column.clone();
        let mut header = vec!["ID".into()];
        let mut name_columns: Vec<String> = vec![];

        let table_sql = if let Some(ref inline) = hier.inline_table {
            // This level has an inline table
            for locale in &locales {
                for column_def in &inline.column_definitions {
                    if let Some(caption_set) = &column_def.caption_set {
                        if caption_set == locale {
                            header.push(format!("{} Label", caption_set.to_uppercase()));
                            name_columns.push(column_def.name.clone());
                            break;
                        }
                    }
                }
            }

            format!("({})", inline.sql_string())
        } else {

            for locale in &locales {
                if let Some(properties) = &level.properties {
                    for property in properties {
                        if let Some(caption_set) = &property.caption_set {
                            if caption_set == locale {
                                header.push(format!("{} Label", caption_set.to_uppercase()));
                                name_columns.push(property.column.clone());
                                break;
                            }
                        }
                    }
                }

                if locale == &self.default_locale {
                    if let Some(level_name_col_val) = &level.name_column {
                        header.push(format!("{} Label", locale.to_uppercase()));
                        name_columns.push(level_name_col_val.to_owned());
                    }
                }
            }

            table.full_name()
        };

        let sql = format!("select distinct {}{}{} from {} order by {}",
            key_column,
            if name_columns.len() > 0 { ", " } else { "" },
            name_columns.join(", "),
            table_sql,
            key_column
        );

        Ok((sql, header))
    }

    /// Convert user parameters into required default member cuts based on cube definition.
    ///
    /// Given a cube and user supplied Query parameters and a boolean for negate mode, this function will:
    ///
    /// 1. Use the get_dims_for_default_member to determine which dimensions are not involved in
    /// a cut or drilldown for the query and are therefore candidates for default member filtering logic
    /// 2. It will read out the default_member strings from relevant hierarchies and parse them into cuts
    /// 3. Returns a list of the cuts that should be added to the query based on the default members
    ///
    /// If the function runs in negate mode, it will add or remove a ~ from the front of the default member
    /// cut string to flip the logic in order to exclude the default member from being returned in queries.
    fn build_default_member_cuts(&self, schema_cube: &Cube, query: &Query, negate: bool) -> Result<Vec<Cut>, Error> {
        let target_dims = self.get_dims_for_default_member(schema_cube, query, negate);
        let result = target_dims.iter().filter_map(|dim| {
            let target_hierarchy_name = match &dim.default_hierarchy {
                Some(hierarchy_name) => hierarchy_name,
                None => &dim.hierarchies.get(0).unwrap().name
            };
            let hierarchy_obj = &dim.hierarchies.iter()
                .find(|h| &h.name == target_hierarchy_name).expect("bad hierarchy unpacking");
            let default_member = &hierarchy_obj.default_member;

            default_member.as_ref().map(|val| {
                let mut new_cut_str: String = val.to_string();
                if negate {
                    let first_ch = new_cut_str.chars().next().expect("Expected at least one character in default member");
                    new_cut_str = match first_ch {
                        '~' => new_cut_str[1..].to_string(),
                        _ => format!("~{}", new_cut_str)
                    }

                }
                Cut::from_str(&new_cut_str)
            })
        })
        .collect::<Result<Vec<_>,_>>();

        result
    }

    /// Helper function for build_default_member_cuts to get a list of dimensions.
    ///
    /// Negate is a boolean value which indicates if the the list is being built for a negate mode query.
    /// In a negate mode, the idea is to build a cut which will exclude the default member when
    /// drilling down which is why in negate mode the logic for the dimension filter differs.
    fn get_dims_for_default_member<'a>(&self, schema_cube: &'a Cube, query: &Query, negate: bool) -> Vec<&'a Dimension> {
        let dims = schema_cube.dimensions.iter()
            .filter(|dim| {
                // filter out dims that have a drilldown or cut
                let dim_contains_drill = query.drilldowns.iter()
                    .any(|drill| dim.name == drill.0.dimension());
                let dim_contains_cut = query.cuts.iter()
                    .any(|c| dim.name == c.level_name.dimension());
                match negate {
                    false => !(dim_contains_drill || dim_contains_cut),
                    true => dim_contains_drill && !dim_contains_cut
                }
            })
            // Keep only the dims that have a default hierarchy value set
            // OR have only one hierarchy. Note that due to the schema validation process,
            // if a dimension has more than one hierarchy, it must have a default hierarchy specified.
            .filter(|dim| dim.default_hierarchy.is_some() || dim.hierarchies.len() == 1)
            .collect();
        dims
    }

    pub fn sql_query(
        &self,
        cube: &str,
        query: &Query,
        unique_header_map: Option<&HashMap<String, String>>
        ) -> Result<(QueryIr, Vec<String>), Error>
    {
        // TODO check that cuts have members:
        // at the beginning of sql_query, (or maybe on cut parsing?), to make
        // clear that blank members will trigger default hierarchy behavior in sql generation

        // First do checks, like making sure there's a measure, and that there's
        // either a cut or drilldown
        if query.measures.is_empty() && query.rca.is_none() {
            return Err(format_err!("No measure found; please specify at least one"));
        }
        if query.drilldowns.is_empty() && query.cuts.is_empty(){
            return Err(format_err!("Either a drilldown or cut is required"));
        }

        // also check that properties have a matching drilldown
        if let Some(ref rca) = query.rca {
            let rca_drills = [&rca.drill_1, &rca.drill_2];
            for property in &query.properties {
                let has_drill = rca_drills.iter()
                    .any(|d| d.0 == property.level_name);

                if !has_drill {
                    return Err(format_err!("Property {} has no matching drilldown", property));
                }
            }
        } else {
            for property in &query.properties {
                let has_drill = query.drilldowns.iter()
                    .any(|d| d.0 == property.level_name);

                if !has_drill {
                    return Err(format_err!("Property {} has no matching drilldown", property));
                }
            }
        }

        // check for default hierarchy that isn't drilled down on. And create a cut for it.
        // TODO should do this at top, and everything is method on cube, instead of on schema
        let schema_cube = self.cubes.iter()
            .find(|c| c.name == cube)
            .ok_or_else(|| format_err!("schema does not contain cube"))?;

        // Note that the marker for a default hierarchy cuts query is that there are no members
        let default_hierarchy_cuts_query: Result<Vec<_>, Error> = schema_cube.dimensions.iter()
            .filter(|dim| {
                // filter out dims that have a drilldown or cut
                let dim_contains_drill = query.drilldowns.iter()
                    .any(|drill| dim.name == drill.0.dimension());

                let dim_contains_cut = query.cuts.iter()
                    .any(|c| dim.name == c.level_name.dimension());

                !(dim_contains_drill || dim_contains_cut)
            })
            .filter(|dim| dim.default_hierarchy.is_some())
            .map(|dim| {
                // for each default hierarchy, get the lowest level
                // the join will actually be on primary key, which does the actual
                // filtering. But this is to be consistent
                let default_hierarchy = dim.default_hierarchy.clone()
                    .ok_or_else(|| format_err!("logic err, is_some already checked"))?;

                let level_name = dim.hierarchies.iter()
                    .find(|hier| hier.name == default_hierarchy)
                    .ok_or_else(|| format_err!("logic error, validation occurred for matching default hier"))
                    .and_then(|hier| {
                        hier.levels.last()
                            .ok_or_else(|| format_err!("logic error, must have a level in hier"))
                    })
                    .map(|level| level.name.clone())?;

                Ok(Cut::new(dim.name.clone(), default_hierarchy, level_name, vec![], Mask::Include, false))
            })
            .collect();
        let default_hierarchy_cuts_query = default_hierarchy_cuts_query?;


        // TODO check that top dim and mea are in here?
        // TODO check that top_where maps to a mea that's not in top, but is in meas.

        // for growth, check if time dim and mea are in drilldown and measures
        if let Some(ref growth) = query.growth {
            if !query.drilldowns.contains(&growth.time_drill) {
                bail!("Growth time drilldown {} is not in drilldowns", growth.time_drill);
            }
            if !query.measures.contains(&growth.mea) {
                bail!("Growth measure {} is not in measures", growth.mea);
            }
        }

        // for rca, disallow cuts on the second drilldown for now, until better system
        // is figured out.
        // There is internal filtering of cuts internally also, which should follow the
        // pattern of the check here.
        if let Some(ref rca) = query.rca {
            let cuts_contain_drill_2 = query.cuts.iter()
                .any(|c| c.level_name == rca.drill_2.0);

            if cuts_contain_drill_2 {
                return Err(format_err!("Cut on rca drill 2 is not allowed; for rca, \
                    only external cuts and cuts on drill 1 allowed", ));
            }
        }

        // now get the database metadata
        let table = self.cube_table(&cube)
            .ok_or(format_err!("No table found for cube {}", cube))?;

        let mut cut_cols = self.cube_cut_cols(&cube, &query.cuts)
            .map_err(|err| format_err!("Error getting cut cols: {}", err))?;

        let default_hierarchy_cut_cols = self.cube_cut_cols(&cube, &default_hierarchy_cuts_query)
            .map_err(|err| format_err!("Error getting cut cols for default hierarchy: {}", err))?;

        cut_cols.extend_from_slice(&default_hierarchy_cut_cols);

        let default_member_cuts_query = self.build_default_member_cuts(schema_cube, query, false)?;
        let default_member_cut_cols = self.cube_cut_cols(&cube, &default_member_cuts_query)
            .map_err(|err| format_err!("Error creating cuts for default member: {}", err))?;
        cut_cols.extend_from_slice(&default_member_cut_cols);

        if query.exclude_default_members {
            let exclude_default_member_cuts_query = self.build_default_member_cuts(schema_cube, query, true)?;
            let exclude_default_member_cut_cols = self.cube_cut_cols(&cube, &exclude_default_member_cuts_query)
                .map_err(|err| format_err!("Error creating exclude cuts for default member: {}", err))?;
            cut_cols.extend_from_slice(&exclude_default_member_cut_cols);
        }


        let drill_cols = self.cube_drill_cols(&cube, &query.drilldowns, &query.properties, &query.captions, query.parents)
            .map_err(|err| format_err!("Error getting drill cols: {}", err))?;

        let mea_cols = self.cube_mea_cols(&cube, &query.measures)
            .map_err(|err| format_err!("Error getting mea cols: {}", err))?;

        // special case for "hidden dimension" used for grouped median. This is where there
        // is a special grouping, currently at the lowest level, of a dimension that is not
        // specified in the query drilldown
        //
        // Not entirely sure if there needs to be a check for only one hidden dimension per query
        let hidden_dims: Vec<_> = mea_cols.iter()
            .filter_map(|mea_ir| {
                // extract group dimension from basic grouped median dimension
                match mea_ir.aggregator {
                    Aggregator::BasicGroupedMedian { ref group_dimension, .. } => {
                        Some(group_dimension)
                    },
                    _ => None,
                }
            })
            .map(|group_dim| group_dim.parse())
            .collect::<Result<_,_>>()
            .map_err(|err| format_err!("Error parsing hidden grouping drill level: {}", err))?;

        let hidden_drill_cols: Vec<_> = self.cube_drill_cols(&cube, &hidden_dims, &[], &[], false)
            .map_err(|err| format_err!("Error getting hidden grouping drill cols: {}", err))?
            .iter()
            .map(|dim_col| HiddenDrilldownSql { drilldown_sql: dim_col.clone() })
            .collect();

        // Options for sorting and limiting

        let limit = query.limit.clone().map(|l| l.into());

        let top = if let Some(ref t) = query.top {
            // don't want the actual measure column,
            // want the index so that we can use `m0` etc.
            let top_sort_columns: Result<Vec<_>, _> = t.sort_mea_or_calc.iter()
                .map(|m_or_c| {
                    mea_or_calc!(m_or_c, query)
                })
                .collect();
            let top_sort_columns = top_sort_columns?;

            // check that by_dimension is in query.drilldowns
            // TODO check for rca drills too
            if let Some(ref rca) = query.rca {
                let mut d = query.drilldowns.clone();
                d.extend_from_slice(&[rca.drill_1.clone(), rca.drill_2.clone()]);

                d.iter()
                    .map(|d| &d.0)
                    .find(|name| **name == t.by_dimension)
                    .ok_or(format_err!("Top by_dimension must be in drilldowns (including rca)"))?;
            } else {
                query.drilldowns.iter()
                    .map(|d| &d.0)
                    .find(|name| **name == t.by_dimension)
                    .ok_or(format_err!("Top by_dimension must be in drilldowns"))?;
            }

            Some(TopSql {
                n: t.n,
                by_column: self.get_dim_col_alias(&cube, &t.by_dimension)?,
                sort_columns: top_sort_columns,
                sort_direction: t.sort_direction.clone(),
            })
        } else {
            None
        };

        // TopWhere, from Query to Query IR
        let top_where = if let Some(ref tw) = query.top_where {
            let by_column = mea_or_calc!(&tw.by_mea_or_calc, query)?;
            Some(TopWhereSql {
                by_column,
                constraint: tw.constraint.clone(),
            })
        } else {
            None
        };

        // Filter, from Query to Query IR. Should be exactly the same as TopWhere
        let filters = query.filters.iter()
            .map(|filter| {
                let by_column = mea_or_calc!(&filter.by_mea_or_calc, query);

                by_column
                    .map(|by_column| {
                        FilterSql {
                            by_column,
                            constraint: filter.constraint.clone(),
                            operator: filter.operator.clone(),
                            constraint2: filter.constraint2.clone()
                        }
                    })
            })
            .collect::<Result<Vec<_>,_>>();
        let filters = filters?;

        let sort = if let Some(ref s) = query.sort {
            // sort column needs to be named by alias
            let sort_column = mea_or_calc!(&s.measure, query)?;
            Some(SortSql {
                direction: s.direction.clone(),
                column: sort_column,
            })
        } else {
            None
        };

        // TODO check that no overlapping dim or mea cols between rca and others
        let rca = if let Some(ref rca) = query.rca {
            let drill_1 = self.cube_drill_cols(&cube, &[rca.drill_1.clone()], &query.properties, &query.captions, query.parents)?;
            let drill_2 = self.cube_drill_cols(&cube, &[rca.drill_2.clone()], &query.properties, &query.captions, query.parents)?;

            let mea = self.cube_mea_cols(&cube, &[rca.mea.clone()])?
                .get(0)
                .ok_or(format_err!("no measure found for rca"))?
                .clone();

            Some(RcaSql {
                drill_1,
                drill_2,
                mea,
                debug: query.debug,
            })
        } else {
            None
        };

        let growth = if let Some(ref growth) = query.growth {
            let time_drill = self.cube_drill_cols(&cube, &[growth.time_drill.clone()], &query.properties, &query.captions, query.parents)?
                .get(0)
                .ok_or(format_err!("no measure found for growth"))?
                .clone();

            // just want the measure id, not the actual measure col
            let mea = query.measures.iter()
                    .position(|mea| *mea == growth.mea )
                    .map(|idx| format!("final_m{}", idx))
                    .ok_or(format_err!("measure for Growth must be in measures"))?;

            Some(GrowthSql {
                time_drill,
                mea,
            })
        } else {
            None
        };

        let rate = if let Some(ref rate) = query.rate {
            // For now at least, we'll allow drilldowns and cuts on the level
            // used for the rate calculation. Drilldowns will always result in
            // a rate of 1. Cuts allow for rate calculation in a subset of the
            // level universe (for example, calculating the rate of "Non-fiction"
            // sales inside a "Books" named set.

            // Only one measure allowed when getting rates for now
            if mea_cols.len() > 1 {
                return Err(format_err!("Only one measure allowed for rate calculations"));
            }

            match mea_cols[0].aggregator {
                Aggregator::Sum => (),
                Aggregator::Count => (),
                _ => return Err(format_err!("Rate can only be calculated for measures with sum or count aggregations"))
            }

            let drilldown_sql = self.cube_drill_cols(
                &cube, &[Drilldown(rate.level_name.clone())],
                &query.properties, &query.captions, query.parents
            )?;

            Some(RateSql {
                drilldown_sql: drilldown_sql[0].clone(),
                members: rate.values.clone(),
            })
        } else {
            None
        };

        // getting headers, not for sql but needed for formatting
        let mut drill_headers = self.cube_drill_headers(&cube, &query.drilldowns, &query.properties, query.parents, unique_header_map)
            .map_err(|err| format_err!("Error getting drill headers: {}", err))?;

        let mut mea_headers = self.cube_mea_headers(&cube, &query.measures)
            .map_err(|err| format_err!("Error getting mea headers: {}", err))?;

        // rca mea will always be first, so just put
        // in `Mea RCA` second
        if let Some(ref rca) = query.rca {
            let rca_drill_headers = self.cube_drill_headers(&cube, &[rca.drill_1.clone(), rca.drill_2.clone()], &query.properties, query.parents, unique_header_map)
                .map_err(|err| format_err!("Error getting rca drill headers: {}", err))?;

            drill_headers.extend_from_slice(&rca_drill_headers);

            if query.debug {
                drill_headers.extend_from_slice(&["a".into(), "b".into(), "c".into(), "d".into()]);
            }

            mea_headers.insert(0, format!("{} RCA", rca.mea.0.clone()));
        }

        // Be careful with other calculations.
        // TODO figure out a more composable system.
        let mut headers = if let Some(ref growth) = query.growth {
            // swapping around measure headers. growth mea moves to back.
            let g_mea_idx = query.measures.iter()
                    .position(|mea| *mea == growth.mea )
                    .ok_or(format_err!("measure for Growth must be in measures"))?;

            let moved_mea = mea_headers.remove(g_mea_idx);
            mea_headers.push(moved_mea);
            mea_headers.push(format!("{} Growth", growth.mea.0));
            mea_headers.push(format!("{} Growth Value", growth.mea.0));

            // swapping around drilldown headers. Move time to back
            let time_headers = self.cube_drill_headers(&cube, &[growth.time_drill.clone()], &[], query.parents, unique_header_map)
                .map_err(|err| format_err!("Error getting time drill headers for Growth: {}", err))?;

            let time_header_idxs: Result<Vec<_>,_> = time_headers.iter()
                .map(|th| {
                    drill_headers.iter()
                        .position(|h| h == th)
                        .ok_or(format_err!("Growth, cannot find time header {} in drill headers", th))
                })
                .collect();
            let time_header_idxs = time_header_idxs?;

            // TODO figure out a better way to move headers
            let mut temp_time_headers = vec![];
            for idx in time_header_idxs.iter().rev() {
                let moved_hdr = drill_headers.remove(*idx);
                temp_time_headers.insert(0, moved_hdr);
            }
            drill_headers.extend_from_slice(&temp_time_headers);

            [&drill_headers[..], &mea_headers[..]].concat()
        } else {
            [&drill_headers[..], &mea_headers[..]].concat()
        };

        // Rate calculations always come last
        if query.rate.is_some() {
            headers.push("Rate".to_string());
        }

        Ok((
            QueryIr {
                table,
                cuts: cut_cols,
                drills: drill_cols,
                meas: mea_cols,
                hidden_drills: hidden_drill_cols,
                filters,
                top,
                top_where,
                sort,
                limit,
                rca,
                growth,
                rate,
                sparse: query.sparse,
            },
            headers,
        ))
    }
}

impl Schema {
    fn cube_table(&self, cube_name: &str) -> Option<TableSql> {
        self.cubes.iter()
            .find(|cube| &cube.name == &cube_name)
            .map(|cube| {
                TableSql {
                    name: cube.table.name.clone(),
                    primary_key: cube.table.primary_key.clone(),
                }
            })
    }

    fn cube_cut_cols(&self, cube_name: &str, cuts: &[Cut]) -> Result<Vec<CutSql>, Error> {
        let cube = self.cubes.iter()
            .find(|cube| &cube.name == &cube_name)
            .ok_or(format_err!("Could not find cube"))?;

        let mut res = vec![];

        for cut in cuts {
            let dim = cube.dimensions.iter()
                .find(|dim| dim.name == cut.level_name.dimension)
                .ok_or(format_err!("could not find dimension for cut {}", cut.level_name))?;
            let hier = dim.hierarchies.iter()
                .find(|hier| hier.name == cut.level_name.hierarchy)
                .ok_or(format_err!("could not find hierarchy for cut {}", cut.level_name))?;
            let level = hier.levels.iter()
                .find(|lvl| lvl.name == cut.level_name.level)
                .ok_or(format_err!("could not find level for cut {}", cut.level_name))?;

            // No table (means inline table) will replace with fact table
            let table = hier.table
                .clone()
                .unwrap_or(cube.table.clone());

            // primary key is currently required in hierarchy. because inline dim is not yet
            // allowed
            let primary_key = hier.primary_key.clone();

            let foreign_key = dim.foreign_key
                .clone()
                .ok_or(format_err!("No foreign key; it's required for now (until inline dim implemented)"))?;

            let column = if cut.for_match {
                level.name_column.clone().unwrap_or(level.key_column.clone())
            } else {
                level.key_column.clone()
            };

            let member_type = if cut.for_match {
                MemberType::Text
            } else {
                level.key_type.clone().unwrap_or(MemberType::NonText)
            };

            res.push(CutSql {
                table,
                primary_key,
                foreign_key,
                column,
                member_type,
                members: cut.members.clone(),
                mask: cut.mask.clone(),
                for_match: cut.for_match,
                inline_table: hier.inline_table.clone(),
            });
        }

        Ok(res)
    }

    // TODO as currently written, properties that don't get picked up by a drilldown
    // will just silently fail.
    fn cube_drill_cols(
        &self,
        cube_name: &str,
        drills: &[Drilldown],
        properties: &[Property],
        captions: &[Property],
        parents: bool,
        ) -> Result<Vec<DrilldownSql>, Error>
    {
        let cube = self.cubes.iter()
            .find(|cube| &cube.name == &cube_name)
            .ok_or(format_err!("Could not find cube"))?;

        let mut res = vec![];

        // now iterate throw drill/property tuples
        for drill in drills {
            let dim = cube.dimensions.iter()
                .find(|dim| dim.name == drill.0.dimension)
                .ok_or(format_err!("could not find dimension for drill {}", drill.0))?;
            let hier = dim.hierarchies.iter()
                .find(|hier| hier.name == drill.0.hierarchy)
                .ok_or(format_err!("could not find hierarchy for drill {}", drill.0))?;
            let levels = &hier.levels;

            // for this drill, get related properties.
            // - filter by properties for this drilldown
            // - for each property, get the level
            let property_columns: Result<Vec<_>, _>= properties.iter()
                .filter(|p| p.level_name == drill.0)
                .map(|p| {
                    levels.iter()
                        .find(|lvl| lvl.name == p.level_name.level)
                        .and_then(|lvl| {
                            if let Some(ref properties) = lvl.properties {
                                properties.iter()
                                    .find(|schema_p| schema_p.name == p.property)

                            } else {
                                None
                            }
                        })
                        .map(|p| p.column.clone())
                        .ok_or(format_err!("cannot find property for {}", p))
                })
                .collect();
            let property_columns = property_columns?;

            // for this drill, get caption.
            // each caption must be specified, but can refer
            // either to an explicit drilldown or parent
            // - filter by properties for this drilldown
            // - for each property, get the level

            // let through parent captions only if parent == true
            let captions_filtered = if parents {
                Box::new(captions.iter()
                    .filter(|p| p.level_name.dimension == drill.0.dimension)
                ) as Box<dyn Iterator<Item=&Property>>
            } else {
                Box::new(captions.iter()
                    .filter(|p| p.level_name == drill.0)
                ) as Box<dyn Iterator<Item=&Property>>
            };

            let caption_cols: Result<HashMap<_, _>, _> = captions_filtered
                .map(|p| {
                    levels.iter()
                        .find(|lvl| lvl.name == p.level_name.level)
                        .and_then(|lvl| {
                            if let Some(ref properties) = lvl.properties {
                                properties.iter()
                                    .find(|schema_p| schema_p.name == p.property)
                                    .map(|p| (lvl, p))

                            } else {
                                None
                            }
                        })
                        .map(|(lvl, p)| (lvl.name.clone(), p.column.clone()) )
                        .ok_or(format_err!("cannot find property-caption for {}", p))
                })
                .collect();
            let caption_cols = caption_cols?;
            if !parents {
                assert!(caption_cols.len() <= 1);
            }

            // No table (means inline table) will replace with fact table
            let table = hier.table
                .clone()
                .unwrap_or(cube.table.clone());

            // primary key is currently required in hierarchy. because inline dim is not yet
            // allowed
            let primary_key = hier.primary_key.clone();

            let foreign_key = dim.foreign_key
                .clone()
                .ok_or(format_err!("No foreign key; it's required for now (until inline dim implemented)"))?;

            // logic for getting level columns.
            // if parents = true, then get all columns down to level
            // if not,then just level
            let level_idx = levels.iter()
                .position(|lvl| lvl.name == drill.0.level)
                .ok_or(format_err!("could not find level for drill {}", drill.0))?;

            let mut level_columns = vec![];

            if parents {
                for i in 0..=level_idx {
                    // caption replaces name_column with the col from property.
                    let caption = if let Some(caption_col) = caption_cols.get(&levels[i].name) {
                        Some(caption_col.clone())
                    } else {
                        levels[i].name_column.clone()
                    };
                    level_columns.push(LevelColumn {
                        key_column: levels[i].key_column.clone(),
                        name_column: caption,
                    });
                }
            } else {
                // caption replaces name_column with the col from property.
                // assertion that caption_col <= 1 above
                let caption = if let Some(caption_col) = caption_cols.get(&levels[level_idx].name) {
                    Some(caption_col.clone())
                } else {
                    levels[level_idx].name_column.clone()
                };
                level_columns.push(LevelColumn {
                    key_column: levels[level_idx].key_column.clone(),
                    name_column: caption,
                });
            }

            let alias_postfix = dim.name.replace(" ", "_");

            res.push(DrilldownSql {
                alias_postfix,
                table,
                primary_key,
                foreign_key,
                level_columns,
                property_columns,
                inline_table: hier.inline_table.clone()
            });
        }

        Ok(res)
    }

    fn cube_mea_cols(&self, cube_name: &str, meas: &[Measure]) -> Result<Vec<MeasureSql>, Error> {
        let cube = self.cubes.iter()
            .find(|cube| &cube.name == &cube_name)
            .ok_or(format_err!("Could not find cube"))?;

        let mut res = vec![];

        for measure in meas {
            let mea = cube.measures.iter()
                .find(|m| m.name == measure.0)
                .ok_or(format_err!("could not find measure for {}", measure.0))?;

            res.push(MeasureSql {
                column: mea.column.clone(),
                aggregator: mea.aggregator.clone(),
            });
        }

        Ok(res)
    }

    /// order should mirror DrillSql col_string,
    /// which should be levels first and then properties after
    /// (for each drilldown)
    fn cube_drill_headers(
        &self,
        cube_name: &str,
        drills: &[Drilldown],
        properties: &[Property],
        parents: bool,
        unique_header_map: Option<&HashMap<String, String>>,
        ) -> Result<Vec<String>, Error>
    {
        let cube = self.cubes.iter()
            .find(|cube| &cube.name == &cube_name)
            .ok_or(format_err!("Could not find cube"))?;

        let mut level_headers = vec![];
        let mut unique_level_headers = vec![];

        for drill in drills {
            let dim = cube.dimensions.iter()
                .find(|dim| dim.name == drill.0.dimension)
                .ok_or(format_err!("could not find dimension for drill"))?;
            let hier = dim.hierarchies.iter()
                .find(|hier| hier.name == drill.0.hierarchy)
                .ok_or(format_err!("could not find hierarchy for drill"))?;
            let levels = &hier.levels;

            // logic for getting level names.
            // if parents = true, then get all columns down to level
            // if not,then just level name
            let level_idx = hier.levels.iter()
                .position(|lvl| lvl.name == drill.0.level)
                .ok_or(format_err!("could not find hierarchy for drill"))?;


            // In this section, need to watch out for whether there's both a
            // key column and a name column and add ID to the first if necessary
            if parents {
                for i in 0..=level_idx {
                    let level_str = format!("{}.{}.{}", dim.name, hier.name, levels[i].name).to_string();

                    if levels[i].name_column.is_some() {
                        let default_header_name = levels[i].name.clone() + " ID";

                        level_headers.push(default_header_name.clone());

                        match unique_header_map {
                            Some(unique_header_map) => {
                                match unique_header_map.get(&level_str) {
                                    Some(unique_header) => unique_level_headers.push(unique_header.clone() + " ID"),
                                    None => unique_level_headers.push(default_header_name.clone())
                                }
                            },
                            None => unique_level_headers.push(default_header_name.clone())
                        }
                    }

                    let default_header_name = &levels[i].name;

                    level_headers.push(default_header_name.clone());

                    match unique_header_map {
                        Some(unique_header_map) => {
                            match unique_header_map.get(&level_str) {
                                Some(unique_header) => unique_level_headers.push(unique_header.clone()),
                                None => unique_level_headers.push(default_header_name.clone())
                            }
                        },
                        None => unique_level_headers.push(default_header_name.clone())
                    }
                }
            } else {
                let level_str = format!("{}.{}.{}", dim.name, hier.name, levels[level_idx].name).to_string();

                if levels[level_idx].name_column.is_some() {
                    let default_header_name = levels[level_idx].name.clone() + " ID";

                    level_headers.push(default_header_name.clone());

                    match unique_header_map {
                        Some(unique_header_map) => {
                            match unique_header_map.get(&level_str) {
                                Some(unique_header) => unique_level_headers.push(unique_header.clone() + " ID"),
                                None => unique_level_headers.push(default_header_name.clone())
                            }
                        },
                        None => unique_level_headers.push(default_header_name.clone())
                    }
                }

                let default_header_name = &levels[level_idx].name;

                level_headers.push(default_header_name.clone());

                match unique_header_map {
                    Some(unique_header_map) => {
                        match unique_header_map.get(&level_str) {
                            Some(unique_header) => unique_level_headers.push(unique_header.clone()),
                            None => unique_level_headers.push(default_header_name.clone())
                        }
                    },
                    None => unique_level_headers.push(default_header_name.clone())
                }
            }

            // for this drill, get related properties.
            // - filter by properties for this drilldown
            // - for each property, get the level
            let property_columns: Result<Vec<_>, _>= properties.iter()
                .filter(|p| p.level_name == drill.0)
                .map(|p| {
                    levels.iter()
                        .find(|lvl| lvl.name == p.level_name.level)
                        .and_then(|lvl| {
                            if let Some(ref properties) = lvl.properties {
                                properties.iter()
                                    .find(|schema_p| schema_p.name == p.property)

                            } else {
                                None
                            }
                        })
                        .map(|p| {
                            p.name.clone()
                        })
                        .ok_or(format_err!("cannot find property for {}", p))
                })
                .collect();
            let property_columns = property_columns?;

            level_headers.extend(property_columns);
        }

        let hash_set: HashSet<String> = level_headers.clone().into_iter().collect();

        if hash_set.len() != level_headers.len() {
            level_headers = unique_level_headers;
        }

        Ok(level_headers)
    }

    fn cube_mea_headers(&self, cube_name: &str, meas: &[Measure]) -> Result<Vec<String>, Error> {
        let cube = self.cubes.iter()
            .find(|cube| &cube.name == &cube_name)
            .ok_or(format_err!("Could not find cube"))?;

        let mut res = vec![];

        for measure in meas {
            let mea = cube.measures.iter()
                .find(|m| m.name == measure.0)
                .ok_or(format_err!("could not find measure in cube"))?;

            res.push(mea.name.clone());
        }

        Ok(res)
    }

    fn get_dim_col_table(&self, cube_name: &str, level_name: &LevelName) -> Result<MembersQueryIR, Error> {
        let cube = self.cubes.iter()
            .find(|cube| &cube.name == &cube_name)
            .ok_or(format_err!("Could not find cube"))?;

        let dim = cube.dimensions.iter()
            .find(|dim| dim.name == level_name.dimension)
            .ok_or(format_err!("could not find dimension for level name"))?;
        let hier = dim.hierarchies.iter()
            .find(|hier| hier.name == level_name.hierarchy)
            .ok_or(format_err!("could not find hierarchy for level name"))?;
        let level = hier.levels.iter()
            .find(|lvl| lvl.name == level_name.level)
            .ok_or(format_err!("could not find level for level name"))?;

        let table = hier.table.clone().unwrap_or_else(|| cube.table.clone());

        // TODO: have a check that there can't be inline table and regular table at the same time.
        // Inline table has highest precedence.
        let table_sql = if let Some(ref inline) = hier.inline_table {
            format!("({})", inline.sql_string())
        } else {
            table.full_name()
        };

        let key_column = level.key_column.clone();
        let name_column = level.name_column.clone();

        Ok(MembersQueryIR {
            table_sql,
            key_column,
            name_column,
        })
    }

    fn get_dim_col_alias(&self, cube_name: &str, level_name: &LevelName) -> Result<String, Error> {
        let cube = self.cubes.iter()
            .find(|cube| &cube.name == &cube_name)
            .ok_or(format_err!("Could not find cube"))?;

        let dim = cube.dimensions.iter()
            .find(|dim| dim.name == level_name.dimension)
            .ok_or(format_err!("could not find dimension for level name"))?;
        let hier = dim.hierarchies.iter()
            .find(|hier| hier.name == level_name.hierarchy)
            .ok_or(format_err!("could not find hierarchy for level name"))?;
        let level = hier.levels.iter()
            .find(|lvl| lvl.name == level_name.level)
            .ok_or(format_err!("could not find level for level name"))?;

        // TODO centralize where to get the alias
        let column = format!("{}_{}", level.key_column, dim.name.replace(" ", "_"));

        Ok(column)
    }

    #[allow(dead_code)]
    fn get_mea_col(&self, cube_name: &str, measure: &Measure) -> Result<String, Error> {
        let cube = self.cubes.iter()
            .find(|cube| &cube.name == &cube_name)
            .ok_or(format_err!("Could not find cube"))?;

        let mea = cube.measures.iter()
            .find(|m| m.name == measure.0)
            .ok_or(format_err!("could not find level for level name"))?;

        let column = mea.column.clone();

        Ok(column)
    }

    pub fn get_cube_by_name(&self, cube_name: &str) -> Result<&Cube, Error> {
        self.cubes.iter()
            .find(|c| &c.name == &cube_name)
            .ok_or(format_err!("Could not find cube"))
    }
}

#[derive(Debug)]
struct MembersQueryIR {
    table_sql: String,
    key_column: String,
    name_column: Option<String>,
}


#[derive(Debug, Clone)]
pub enum CubeHasUniqueLevelsAndProperties {
    True,
    False {
        cube: String,
        name: String,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    // use serde_json;
    use crate::query::*;

    const SCHEMA_STR_MULTIPLE_HIER_NO_DEFAULT: &str = r#"{ "name": "test", "cubes": [ { "name": "sales", "table": { "name": "sales", "primary_key": "product_id" }, "dimensions": [{ "name": "Geography", "foreign_key": "customer_id", "hierarchies": [ { "name": "Tract", "table": { "name": "customer_geo" }, "primary_key": "customer_id", "levels": [ { "name": "State", "key_column": "state_id", "name_column": "state_name", "key_type": "text" }, { "name": "County", "key_column": "county_id", "name_column": "county_name", "key_type": "text" }, { "name": "Tract", "key_column": "tract_id", "name_column": "tract_name", "key_type": "text" } ] }, { "name": "Place", "table": { "name": "customer_geo" }, "primary_key": "customer_id", "levels": [ { "name": "Place", "key_column": "place_id", "name_column": "place_name", "key_type": "text" } ] } ] } ], "measures": [ { "name": "Quantity", "column": "quantity", "aggregator": "sum" } ] } ] }"#;
    const SCHEMA_STR_MULTIPLE_HIER_DEFAULT: &str = r#"{ "name": "test", "cubes": [ { "name": "sales", "table": { "name": "sales", "primary_key": "product_id" }, "dimensions": [{ "name": "Geography", "foreign_key": "customer_id", "default_hierarchy": "Tract", "hierarchies": [ { "name": "Tract", "table": { "name": "customer_geo" }, "primary_key": "customer_id", "levels": [ { "name": "State", "key_column": "state_id", "name_column": "state_name", "key_type": "text" }, { "name": "County", "key_column": "county_id", "name_column": "county_name", "key_type": "text" }, { "name": "Tract", "key_column": "tract_id", "name_column": "tract_name", "key_type": "text" } ] }, { "name": "Place", "table": { "name": "customer_geo" }, "primary_key": "customer_id", "levels": [ { "name": "Place", "key_column": "place_id", "name_column": "place_name", "key_type": "text" } ] } ] } ], "measures": [ { "name": "Quantity", "column": "quantity", "aggregator": "sum" } ] } ] }"#;
    const SCHEMA_STR_SINGLE_HIER_NO_DEFAULT: &str = r#"{ "name": "test", "cubes": [ { "name": "sales", "table": { "name": "sales", "primary_key": "product_id" }, "dimensions": [{ "name": "Geography", "foreign_key": "customer_id", "hierarchies": [ { "name": "Tract", "table": { "name": "customer_geo" }, "primary_key": "customer_id", "levels": [ { "name": "State", "key_column": "state_id", "name_column": "state_name", "key_type": "text" }, { "name": "County", "key_column": "county_id", "name_column": "county_name", "key_type": "text" }, { "name": "Tract", "key_column": "tract_id", "name_column": "tract_name", "key_type": "text" } ] } ] } ], "measures": [ { "name": "Quantity", "column": "quantity", "aggregator": "sum" } ] } ] }"#;
    const SCHEMA_STR_SINGLE_HIER_DEFAULT: &str = r#"{ "name": "test", "cubes": [ { "name": "sales", "table": { "name": "sales", "primary_key": "product_id" }, "dimensions": [{ "name": "Geography", "foreign_key": "customer_id", "default_hierarchy": "Tract", "hierarchies": [ { "name": "Tract", "table": { "name": "customer_geo" }, "primary_key": "customer_id", "levels": [ { "name": "State", "key_column": "state_id", "name_column": "state_name", "key_type": "text" }, { "name": "County", "key_column": "county_id", "name_column": "county_name", "key_type": "text" }, { "name": "Tract", "key_column": "tract_id", "name_column": "tract_name", "key_type": "text" } ] } ] } ], "measures": [ { "name": "Quantity", "column": "quantity", "aggregator": "sum" } ] } ] }"#;
    const SCHEMA_NO_DIM: &str = r#"{ "name": "test", "cubes": [ { "name": "sales", "table": { "name": "sales", "primary_key": "product_id" }, "dimensions": [], "measures": [ { "name": "Quantity", "column": "quantity", "aggregator": "sum" } ] } ] }"#;

    #[test]
    #[should_panic]
    fn test_validate_schema_multiple_hier_no_default() {
        let mut schema: Schema = Schema::from_json(SCHEMA_STR_MULTIPLE_HIER_NO_DEFAULT).unwrap();
        schema.validate().unwrap();
    }

    #[test]
    fn test_validate_schema_multiple_hier_default() {
        let mut schema: Schema = Schema::from_json(SCHEMA_STR_MULTIPLE_HIER_DEFAULT).unwrap();
        schema.validate().unwrap();
    }

    #[test]
    fn test_validate_schema_single_hier_no_default() {
        let mut schema: Schema = Schema::from_json(SCHEMA_STR_SINGLE_HIER_NO_DEFAULT).unwrap();
        schema.validate().unwrap();
    }

    #[test]
    #[should_panic]
    fn test_validate_schema_single_hier_default() {
        let mut schema: Schema = Schema::from_json(SCHEMA_STR_SINGLE_HIER_DEFAULT).unwrap();

        assert_eq!(schema.cubes[0].dimensions[0].default_hierarchy.clone().unwrap(), "Tract".to_owned());

        schema.validate().unwrap();

        // should be rewritten to NONE
        schema.cubes[0].dimensions[0].default_hierarchy.clone().unwrap();
    }

    #[test]
    #[should_panic]
    fn test_validate_schema_dimension_number() {
        let mut schema: Schema = Schema::from_json(SCHEMA_NO_DIM).unwrap();
        schema.validate().unwrap();
    }

    #[test]
    fn shared_dim_two_dims() {
        let s = r##"
            <Schema name="my_schema">
                <SharedDimension name="Geo">
                    <Hierarchy name="Country">
                        <Level name="Country" key_column="id" />
                    </Hierarchy>
                </SharedDimension>
                <Cube name="my_cube">
                    <Table name="my_table" />
                    <DimensionUsage name="Import Countries" source="Geo" foreign_key="country_id" />
                    <DimensionUsage name="Export Countries" source="Geo" foreign_key="country_id" />
                    <Measure name="my_mea" column="mea" aggregator="sum" />
                </Cube>
            </Schema>
        "##;
        let schema: Schema = Schema::from_xml(s).unwrap();
        println!("{:#?}", schema);

        assert_eq!(schema.cubes[0].dimensions[0].hierarchies[0].name, "Country".to_owned());
        assert_eq!(schema.cubes[0].dimensions[1].hierarchies[0].name, "Country".to_owned());

        assert_eq!(schema.cubes[0].dimensions[0].name, "Import Countries".to_owned());
        assert_eq!(schema.cubes[0].dimensions[1].name, "Export Countries".to_owned());
    }

    #[test]
    #[should_panic]
    fn shared_dim_validate_duplicate_name() {
        let s = r##"
            <Schema name="my_schema">
                <SharedDimension name="Geo">
                    <Hierarchy name="Country">
                        <Level name="Country" key_column="id" />
                    </Hierarchy>
                </SharedDimension>
                <Cube name="my_cube">
                    <Table name="my_table" />
                    <DimensionUsage source="Geo" foreign_key="country_id" />
                    <DimensionUsage source="Geo" foreign_key="country_id" />
                    <Measure name="my_mea" column="mea" aggregator="sum" />
                </Cube>
            </Schema>
        "##;
        let mut schema: Schema = Schema::from_xml(s).unwrap();
        schema.validate().unwrap();
        println!("{:#?}", schema);
    }

    #[test]
    fn test_basic_default_member() {
        let s = r##"
            <Schema name="my_schema">
                <Cube name="my_cube">
                    <Table name="my_table" />
                    <Dimension foreign_key="race" name="Race">
                        <Hierarchy name="Race" primary_key="race" default_member="Race.Race.Race.Total">
                            <Level name="Race" key_column="race" key_type="text"/>
                        </Hierarchy>
                    </Dimension>
                    <Measure name="my_mea" column="mea" aggregator="sum" />
                </Cube>
            </Schema>
        "##;
        let schema: Schema = Schema::from_xml(s).unwrap();
        println!("{:#?}", schema);
        let dm = schema.cubes[0].dimensions[0].hierarchies[0].default_member.clone();
        assert_eq!(dm.unwrap(), "Race.Race.Race.Total".to_owned());
    }

    #[test]
    #[should_panic]
    fn test_sort_rca() {
        let s = r##"
        <Schema name="Webshop">
            <SharedDimension name="Geography" type="geo">
                <Hierarchy name="Geography">
                    <Table name="tesseract_webshop_geographies" />


                    <Level name="Continent" key_column="continent_id" name_column="continent_name"
                            key_type="text">
                        <Property name="Continent PT" column="continent_name_pt" caption_set="pt" />
                        <Property name="Continent ES" column="continent_name_es" caption_set="es" />
                    </Level>
                    <Level name="Country" key_column="country_id" name_column="country_name"
                            key_type="nontext">
                        <Property name="Country PT" column="country_name_pt" caption_set="pt" />
                        <Property name="Country ES" column="country_name_es" caption_set="es" />
                    </Level>
                </Hierarchy>
            </SharedDimension>

            <Cube name="Sales">
                <Table name="tesseract_webshop_sales" />

                <DimensionUsage foreign_key="country_id" name="Geography" source="Geography" />

                <Dimension name="Year" foreign_key="year">
                    <Hierarchy name="Year">
                        <Level name="Year" key_column="year" />
                    </Hierarchy>
                </Dimension>

                <Dimension name="Month" foreign_key="month_id">
                    <Hierarchy name="Month">
                        <Table name="tesseract_webshop_time" />

                        <Level name="Month" key_column="month_id" name_column="month_name">
                            <Property name="Month PT" column="month_name_pt" caption_set="pt" />
                        </Level>
                    </Hierarchy>
                </Dimension>

                <Dimension name="Category" foreign_key="category_id">
                    <Hierarchy name="Category">
                        <InlineTable alias="tesseract_webshop_categories">
                            <ColumnDef name="category_name" key_type="text" />
                            <ColumnDef name="category_name_pt" key_type="text" caption_set="pt" />
                            <ColumnDef name="category_name_es" key_type="text" caption_set="es" />
                            <ColumnDef name="category_idx" key_type="nontext" key_column_type="Int32" />

                            <Row>
                                <Value column="category_name">Books</Value>
                                <Value column="category_name_pt">Livros</Value>
                                <Value column="category_name_es">Libros</Value>
                                <Value column="category_idx">1</Value>
                            </Row>
                            <Row>
                                <Value column="category_name">Sports</Value>
                                <Value column="category_name_pt">Esportes</Value>
                                <Value column="category_name_es">Deportes</Value>
                                <Value column="category_idx">2</Value>
                            </Row>
                            <Row>
                                <Value column="category_name">Various</Value>
                                <Value column="category_name_pt">Vários</Value>
                                <Value column="category_name_es">Varios</Value>
                                <Value column="category_idx">3</Value>
                            </Row>
                            <Row>
                                <Value column="category_name">Videos</Value>
                                <Value column="category_name_pt">Vídeos</Value>
                                <Value column="category_name_es">Videos</Value>
                                <Value column="category_idx">4</Value>
                            </Row>
                        </InlineTable>

                        <!-- <Level name="Category" key_column="category_id" name_column="category_name" key_type="nontext" /> -->

                        <Level name="Category" key_column="category_idx" name_column="category_name" key_type="nontext" />
                    </Hierarchy>
                </Dimension>

                <Measure name="Price Total" column="price_total" aggregator="sum" />
                <Measure name="Quantity" column="quantity" aggregator="sum" />
            </Cube>
        </Schema>
        "##;
        let query = Query {
            drilldowns: [Drilldown(LevelName{
                dimension: "Year".to_string(),
                hierarchy: "Year".to_string(),
                level: "Year".to_string(),
            })].to_vec(),
            cuts: vec![],
            measures: [Measure("Price Total".to_string())].to_vec(),
            properties: vec![],
            filters: vec![],
            captions: vec![],
            parents: false,
            top: None,
            top_where: None,
            sort: Some(SortQuery{
                direction: SortDirection::Asc,
                measure: MeaOrCalc::Mea(Measure("Price Total".to_string()))
            }),
            limit: None,
            rca: Some(RcaQuery{
                drill_1: Drilldown(LevelName{
                    dimension: "Year".to_string(),
                    hierarchy: "Year".to_string(),
                    level: "Year".to_string(),
                }),
                drill_2: Drilldown(LevelName{
                    dimension: "Year".to_string(),
                    hierarchy: "Year".to_string(),
                    level: "Year".to_string(),
                }),
                mea: Measure("Price Total".to_string())
            }),
            growth: None,
            rate: None,
            debug: false,
            sparse: false,
            exclude_default_members: false,
        };
        let query_ir_headers = Schema::from_xml(s).unwrap().sql_query("Sales", &query, None);
        let (query_ir, _headers) = query_ir_headers.unwrap();
        assert_eq!(query_ir.sort, Some(SortSql{direction: SortDirection::Asc, column: "final_m0".to_string()}))
    }

    #[test]
    fn test_filter_query() {
        let s = r##"
        <Schema name="Webshop">
            <SharedDimension name="Geography" type="geo">
                <Hierarchy name="Geography">
                    <Table name="tesseract_webshop_geographies" />


                    <Level name="Continent" key_column="continent_id" name_column="continent_name"
                            key_type="text">
                        <Property name="Continent PT" column="continent_name_pt" caption_set="pt" />
                        <Property name="Continent ES" column="continent_name_es" caption_set="es" />
                    </Level>
                    <Level name="Country" key_column="country_id" name_column="country_name"
                            key_type="nontext">
                        <Property name="Country PT" column="country_name_pt" caption_set="pt" />
                        <Property name="Country ES" column="country_name_es" caption_set="es" />
                    </Level>
                </Hierarchy>
            </SharedDimension>

            <Cube name="Sales">
                <Table name="tesseract_webshop_sales" />

                <DimensionUsage foreign_key="country_id" name="Geography" source="Geography" />

                <Dimension name="Year" foreign_key="year">
                    <Hierarchy name="Year">
                        <Level name="Year" key_column="year" />
                    </Hierarchy>
                </Dimension>

                <Dimension name="Month" foreign_key="month_id">
                    <Hierarchy name="Month">
                        <Table name="tesseract_webshop_time" />

                        <Level name="Month" key_column="month_id" name_column="month_name">
                            <Property name="Month PT" column="month_name_pt" caption_set="pt" />
                        </Level>
                    </Hierarchy>
                </Dimension>

                <Dimension name="Category" foreign_key="category_id">
                    <Hierarchy name="Category">
                        <InlineTable alias="tesseract_webshop_categories">
                            <ColumnDef name="category_name" key_type="text" />
                            <ColumnDef name="category_name_pt" key_type="text" caption_set="pt" />
                            <ColumnDef name="category_name_es" key_type="text" caption_set="es" />
                            <ColumnDef name="category_idx" key_type="nontext" key_column_type="Int32" />

                            <Row>
                                <Value column="category_name">Books</Value>
                                <Value column="category_name_pt">Livros</Value>
                                <Value column="category_name_es">Libros</Value>
                                <Value column="category_idx">1</Value>
                            </Row>
                            <Row>
                                <Value column="category_name">Sports</Value>
                                <Value column="category_name_pt">Esportes</Value>
                                <Value column="category_name_es">Deportes</Value>
                                <Value column="category_idx">2</Value>
                            </Row>
                            <Row>
                                <Value column="category_name">Various</Value>
                                <Value column="category_name_pt">Vários</Value>
                                <Value column="category_name_es">Varios</Value>
                                <Value column="category_idx">3</Value>
                            </Row>
                            <Row>
                                <Value column="category_name">Videos</Value>
                                <Value column="category_name_pt">Vídeos</Value>
                                <Value column="category_name_es">Videos</Value>
                                <Value column="category_idx">4</Value>
                            </Row>
                        </InlineTable>

                        <!-- <Level name="Category" key_column="category_id" name_column="category_name" key_type="nontext" /> -->

                        <Level name="Category" key_column="category_idx" name_column="category_name" key_type="nontext" />
                    </Hierarchy>
                </Dimension>

                <Measure name="Price Total" column="price_total" aggregator="sum" />
                <Measure name="Quantity" column="quantity" aggregator="sum" />
            </Cube>
        </Schema>
        "##;
        let query = Query {
            drilldowns: [Drilldown(LevelName{
                dimension: "Year".to_string(),
                hierarchy: "Year".to_string(),
                level: "Year".to_string(),
            })].to_vec(),
            cuts: vec![],
            measures: [Measure("Price Total".to_string()), Measure("Quantity".to_string())].to_vec(),
            properties: vec![],
            filters: [FilterQuery{
                by_mea_or_calc: MeaOrCalc::Mea(Measure("Price Total".to_string())),
                constraint: Constraint{
                    comparison: Comparison::LessThan,
                    n: 100.0
                },
                operator: Some(Operator::Or),
                constraint2: Some(Constraint{
                    comparison: Comparison::GreaterThan,
                    n: 200.0
                }),
            },
            FilterQuery{
                by_mea_or_calc: MeaOrCalc::Mea(Measure("Quantity".to_string())),
                constraint: Constraint{
                    comparison: Comparison::GreaterThan,
                    n: 40.0
                },
                operator: None,
                constraint2: None,
            },
            FilterQuery{
                by_mea_or_calc: MeaOrCalc::Calc(Calculation::Rca),
                constraint: Constraint{
                    comparison: Comparison::GreaterThan,
                    n: 1.0
                },
                operator: None,
                constraint2: None,
            }
            ].to_vec(),
            captions: vec![],
            parents: false,
            top: None,
            top_where: None,
            sort: Some(SortQuery{
                direction: SortDirection::Asc,
                measure: MeaOrCalc::Mea(Measure("Price Total".to_string()))
            }),
            limit: None,
            rca: Some(RcaQuery{
                drill_1: Drilldown(LevelName{
                    dimension: "Year".to_string(),
                    hierarchy: "Year".to_string(),
                    level: "Year".to_string(),
                }),
                drill_2: Drilldown(LevelName{
                    dimension: "Year".to_string(),
                    hierarchy: "Year".to_string(),
                    level: "Year".to_string(),
                }),
                mea: Measure("Price Total".to_string())
            }),
            growth: None,
            rate: None,
            debug: false,
            sparse: false,
            exclude_default_members: false,
        };
        let query_ir_headers = Schema::from_xml(s).unwrap().sql_query("Sales", &query, None);
        let (query_ir, _headers) = query_ir_headers.unwrap();
        assert_eq!(query_ir.filters, [FilterSql {
            by_column: "final_m1".to_string(),
            constraint: Constraint {
                comparison: Comparison::LessThan,
                n: 100.0,
            },
            operator: Some(
                Operator::Or,
            ),
            constraint2: Some(
                Constraint {
                    comparison: Comparison::GreaterThan,
                    n: 200.0,
                },
            ),
        },
        FilterSql {
            by_column: "final_m2".to_string(),
            constraint: Constraint {
                comparison: Comparison::GreaterThan,
                n: 40.0,
            },
            operator: None,
            constraint2: None,
        },
        FilterSql {
            by_column: "rca".to_string(),
            constraint: Constraint {
                comparison: Comparison::GreaterThan,
                n: 1.0,
            },
            operator: None,
            constraint2: None,
        }].to_vec())
    }
}
