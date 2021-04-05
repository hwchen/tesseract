use std::collections::{HashMap, HashSet};
use anyhow::{Error, format_err};
use log::{info, debug};
use std::time::Instant;

use serde_derive::Deserialize;

use tesseract_core::{Schema, Backend};
use tesseract_core::names::{LevelName, Property};
use tesseract_core::schema::{Level, Cube, InlineTable};

use crate::logic_layer::LogicLayerConfig;


#[derive(Debug, Clone)]
pub enum TimeValue {
    First,
    Last,
    Value(u32),
}


impl TimeValue {
    pub fn from_str(raw: String) -> Result<Self, Error> {
        if raw == "latest" {
            Ok(TimeValue::Last)
        } else if raw == "oldest" {
            Ok(TimeValue::First)
        } else {
            match raw.parse::<u32>() {
                Ok(n) => Ok(TimeValue::Value(n)),
                Err(_) => Err(format_err!("Wrong type for time argument."))
            }
        }
    }
}


#[derive(Debug, Clone)]
pub enum TimePrecision {
    Year,
    Quarter,
    Month,
    Week,
    Day,
    // `Time` is a generic name that holds a column with a combination of time dimensions.
    // For example, a time column my hold the value `201801` for January 2018.
    // This allows for a but on the latest month of the latest year.
    // This implementation is agnostic to what this column holds, as long as its level
    // is named `Time`.
    Time,
}


impl TimePrecision {
    pub fn from_str(raw: String) -> Result<Self, Error> {
        match raw.as_ref() {
            "year" => Ok(TimePrecision::Year),
            "quarter" => Ok(TimePrecision::Quarter),
            "month" => Ok(TimePrecision::Month),
            "week" => Ok(TimePrecision::Week),
            "day" => Ok(TimePrecision::Day),
            "time" => Ok(TimePrecision::Time),
            _ => Err(format_err!("Wrong type for time precision argument."))
        }
    }
}


#[derive(Debug, Clone)]
pub struct Time {
    pub precision: TimePrecision,
    pub value: TimeValue,
}


impl Time {
    pub fn from_str(raw: String) -> Result<Self, Error> {
        let e: Vec<&str> = raw.split(".").collect();

        if e.len() != 2 {
            return Err(format_err!("Wrong format for time argument."));
        }

        let precision = match TimePrecision::from_str(e[0].to_string()) {
            Ok(precision) => precision,
            Err(err) => return Err(err),
        };
        let value = match TimeValue::from_str(e[1].to_string()) {
            Ok(value) => value,
            Err(err) => return Err(err),
        };

        Ok(Time {precision, value})
    }

    pub fn from_key_value(key: String, value: String) -> Result<Self, Error> {
        let precision = match TimePrecision::from_str( key) {
            Ok(precision) => precision,
            Err(err) => return Err(err),
        };
        let value = match TimeValue::from_str(value) {
            Ok(value) => value,
            Err(err) => return Err(err),
        };

        Ok(Time {precision, value})
    }
}


/// Holds cache information.
#[derive(Debug, Clone)]
pub struct Cache {
    pub cubes: Vec<CubeCache>,
}


impl Cache {
    /// Finds the `CubeCache` object for a cube with a given name.
    pub fn find_cube_info(&self, cube: &String) -> Option<&CubeCache> {
        for cube_cache in &self.cubes {
            if cube_cache.name == *cube {
                return Some(cube_cache);
            }
        }
        None
    }
}


/// Holds cache information for a given cube.
#[derive(Debug, Clone, Deserialize)]
pub struct CubeCache {
    pub name: String,

    pub year_level: Option<Level>,
    pub year_values: Option<Vec<String>>,

    pub quarter_level: Option<Level>,
    pub quarter_values: Option<Vec<String>>,

    pub month_level: Option<Level>,
    pub month_values: Option<Vec<String>>,

    pub week_level: Option<Level>,
    pub week_values: Option<Vec<String>>,

    pub day_level: Option<Level>,
    pub day_values: Option<Vec<String>>,

    pub time_level: Option<Level>,
    pub time_values: Option<Vec<String>>,

    pub level_map: HashMap<String, LevelName>,
    pub property_map: HashMap<String, Property>,

    // Maps a level name to a `LevelCache` object
    pub level_caches: HashMap<LevelName, LevelCache>,

    // Maps a dimension name to a `DimensionCache` object
    pub dimension_caches: HashMap<String, DimensionCache>,
}


impl CubeCache {
    pub fn get_time_cut(&self, time: Time) -> Result<(String, String), Error> {
        let (val_res, ln_res) = match time.precision {
            TimePrecision::Year => {
                let v = self.get_value(&time, self.year_values.clone());
                let l = self.get_level_name(self.year_level.clone());
                (v, l)
            },
            TimePrecision::Quarter => {
                let v = self.get_value(&time, self.quarter_values.clone());
                let l = self.get_level_name(self.quarter_level.clone());
                (v, l)
            },
            TimePrecision::Month => {
                let v = self.get_value(&time, self.month_values.clone());
                let l = self.get_level_name(self.month_level.clone());
                (v, l)
            },
            TimePrecision::Week => {
                let v = self.get_value(&time, self.week_values.clone());
                let l = self.get_level_name(self.week_level.clone());
                (v, l)
            },
            TimePrecision::Day => {
                let v = self.get_value(&time, self.day_values.clone());
                let l = self.get_level_name(self.day_level.clone());
                (v, l)
            },
            TimePrecision::Time => {
                let v = self.get_value(&time, self.time_values.clone());
                let l = self.get_level_name(self.time_level.clone());
                (v, l)
            }
        };

        let val = match val_res {
            Some(o) => o,
            None => return Err(format_err!("Unable to get requested time precision data."))
        };

        let ln = match ln_res {
            Some(o) => o,
            None => return Err(format_err!("Unable to get requested time precision level name."))
        };

        Ok((ln, val))
    }

    pub fn get_level_name(&self, level: Option<Level>) -> Option<String> {
        match level {
            Some(l) => Some(l.name),
            None => None
        }
    }

    pub fn get_value(&self, time: &Time, opt: Option<Vec<String>>) -> Option<String> {
        match opt {
            Some(v) => {
                match time.value {
                    TimeValue::First => {
                        if v.len() >= 1 {
                            return Some(v[0].clone());
                        }
                        None
                    },
                    TimeValue::Last => {
                        if v.len() >= 1 {
                            return Some(v.last().unwrap().clone())
                        }
                        None
                    },
                    TimeValue::Value(t) => return Some(t.to_string())
                }
            },
            None => None
        }
    }

    // TODO note that this is being used in core tesseract, but that the cache is created using
    // logic layer rules. This means that at the moment of this implementation, this will work in
    // core tesseract but only if the core tesseract schema can also be a logic layer schema (and
    // satisfying all the rules about level, property uniqueness, etc.)
    //
    // The next step is to figure out how to architecture this cache so that it can be used even in
    // non-logic layer setups, but also be used for members endpoint (which requires label also)
    pub fn members_for_level(&self, level_name: &LevelName) -> Option<&HashSet<String>> {
        debug!("Level Caches: {:?}", self.level_caches);
        self.level_caches.get(level_name)
            .map(|level_cache| &level_cache.members)
    }
}


#[derive(Debug, Clone, Deserialize)]
pub struct LevelCache {
    pub unique_name: String,
    pub parent_map: Option<HashMap<String, String>>,
    pub children_map: Option<HashMap<String, Vec<String>>>,
    pub neighbors_map: HashMap<String, Vec<String>>,
    // TODO to be able to use for /members endpoint, this will
    // need both ID and member label. Right now it's just ID
    pub members: HashSet<String>,
}


#[derive(Debug, Clone, Deserialize)]
pub struct DimensionCache {
    pub id_map: HashMap<String, Vec<LevelName>>,
}


/// Populates a `Cache` object that will be shared through `AppState`.
pub async fn populate_cache(
        schema: Schema,
        ll_config: Option<&LogicLayerConfig>,
        backend: Box<dyn Backend + Sync + Send>,
) -> Result<Cache, Error> {
    info!("Populating cache...");
    let time_start = Instant::now();

    let time_column_names = vec![
        "Year".to_string(),
        "Quarter".to_string(),
        "Month".to_string(),
        "Week".to_string(),
        "Day".to_string()
    ];

    let mut cubes: Vec<CubeCache> = vec![];

    for cube in schema.cubes {
        let mut year_level: Option<Level> = None;
        let mut year_values: Option<Vec<String>> = None;
        let mut quarter_level: Option<Level> = None;
        let mut quarter_values: Option<Vec<String>> = None;
        let mut month_level: Option<Level> = None;
        let mut month_values: Option<Vec<String>> = None;
        let mut week_level: Option<Level> = None;
        let mut week_values: Option<Vec<String>> = None;
        let mut day_level: Option<Level> = None;
        let mut day_values: Option<Vec<String>> = None;
        let mut time_level: Option<Level> = None;
        let mut time_values: Option<Vec<String>> = None;

        let mut level_caches: HashMap<LevelName, LevelCache> = HashMap::new();
        let mut dimension_caches: HashMap<String, DimensionCache> = HashMap::new();

        for dimension in &cube.dimensions {
            let mut id_map: HashMap<String, Vec<LevelName>> = HashMap::new();

            for hierarchy in &dimension.hierarchies {
                let table = match &hierarchy.table {
                    Some(t) => &t.name,
                    None => &cube.table.name
                };

                for level in &hierarchy.levels {
                    if time_column_names.contains(&level.name) {
                        let val = get_distinct_values(
                            level.key_column.clone(), table.clone(), backend.clone()
                        ).await?;

                        if level.name == "Year" {
                            year_level = Some(level.clone());
                            year_values = Some(val);
                        } else if level.name == "Quarter" {
                            quarter_level = Some(level.clone());
                            quarter_values = Some(val);
                        } else if level.name == "Month" {
                            month_level = Some(level.clone());
                            month_values = Some(val);
                        } else if level.name == "Week" {
                            week_level = Some(level.clone());
                            week_values = Some(val);
                        } else if level.name == "Day" {
                            day_level = Some(level.clone());
                            day_values = Some(val);
                        }
                    } else if level.name == "Time" {
                        // Identify what time of level this is based on the annotation name
                        let mut found_time = false;

                        // This is a hack for now. It handles the case where you
                        // have a level called Time that is actually at a more
                        // specific depth. It allows to cut on that depth using the
                        // .latest/.oldest feature.
                        match &level.annotations {
                            Some(annotations) => {
                                for annotation in annotations {
                                    if annotation.name == "level" && time_column_names.contains(&annotation.text) {
                                        let val = get_distinct_values(
                                            level.key_column.clone(), table.clone(), backend.clone()
                                        ).await?;

                                        if annotation.text == "Year" {
                                            year_level = Some(level.clone());
                                            year_values = Some(val);
                                            found_time = true;
                                        } else if annotation.text == "Quarter" {
                                            quarter_level = Some(level.clone());
                                            quarter_values = Some(val);
                                            found_time = true;
                                        } else if annotation.text == "Month" {
                                            month_level = Some(level.clone());
                                            month_values = Some(val);
                                            found_time = true;
                                        } else if annotation.text == "Week" {
                                            week_level = Some(level.clone());
                                            week_values = Some(val);
                                            found_time = true;
                                        } else if annotation.text == "Day" {
                                            day_level = Some(level.clone());
                                            day_values = Some(val);
                                            found_time = true;
                                        } else if annotation.text == "Time" {
                                            time_level = Some(level.clone());
                                            time_values = Some(val);
                                            found_time = true;
                                        }
                                    }
                                }
                            },
                            None => ()
                        }

                        // Consider this to be a time generic Time level
                        if !found_time {
                            // Want to get distinct time values from the fact table
                            let val = get_distinct_values(
                                level.key_column.clone(), cube.table.name.clone(), backend.clone()
                            ).await?;

                            time_level = Some(level.clone());
                            time_values = Some(val);
                        }
                    }

                    let level_name = LevelName::new(
                        dimension.name.clone(),
                        hierarchy.name.clone(),
                        level.name.clone()
                    );

                    // Get unique name for this level
                    let unique_name = match get_unique_level_name(&cube, ll_config, &level_name)? {
                        Some(name) => name,
                        None => return Err(format_err!("Couldn't find unique name for {}", level.name.clone()))
                    };

                    let mut parent_map: Option<HashMap<String, String>> = None;
                    let mut children_map: Option<HashMap<String, Vec<String>>> = None;

                    let parent_levels = cube.get_level_parents(&level_name)?;
                    let child_level = cube.get_child_level(&level_name)?;

                    let mut distinct_ids: Vec<String> = vec![];

                    if hierarchy.inline_table.is_some() {
                        // Inline table

                        let inline_table = match &hierarchy.inline_table {
                            Some(t) => t,
                            None => return Err(format_err!("Could not get inline table for {}", level.name.clone()))
                        };

                        if parent_levels.len() >= 1 {
                            parent_map = Some(get_inline_parent_data(
                                &parent_levels[parent_levels.len() - 1], &level,
                                &inline_table
                            ));
                        }

                        match child_level {
                            Some(child_level) => {
                                children_map = Some(get_inline_children_data(
                                    &level, &child_level, &inline_table
                                ));
                            },
                            None => ()
                        }

                        // Get all IDs for this level
                        for row in &inline_table.rows {
                            for row_value in &row.row_values {
                                if row_value.column == level.key_column {
                                    distinct_ids.push(row_value.value.clone());
                                }
                            }
                        }
                    } else {
                        // Database table

                        if parent_levels.len() >= 1 {
                            parent_map = Some(get_parent_data(
                                parent_levels[parent_levels.len() - 1].clone(), level.clone(),
                                table, backend.clone()
                            ).await?);
                        }

                        match child_level {
                            Some(child_level) => {
                                children_map = Some(get_children_data(
                                    level.clone(), child_level.clone(),
                                    table, backend.clone()
                                ).await?);
                            },
                            None => ()
                        }

                        // Get all IDs for this level
                        distinct_ids = get_distinct_values(
                            level.key_column.clone(), table.clone(), backend.clone()
                        ).await?;
                    }

                    let neighbors_map = get_neighbors_map(&distinct_ids);

                    // Add each distinct ID to the id_map HashMap
                    for distinct_id in distinct_ids {
                        id_map.entry(distinct_id.clone()).or_insert(vec![]);
                        let map_entry = id_map.get_mut(&distinct_id).unwrap();
                        map_entry.push(level_name.clone());
                    }

                    // neighbors are not optional, iterate over the keys of neighbors to get all
                    // members.
                    let members = neighbors_map.keys().cloned().collect();

                    level_caches.insert(
                        level_name,
                        LevelCache {
                            unique_name: unique_name.clone(),
                            parent_map,
                            children_map,
                            neighbors_map,
                            members
                        }
                    );
                }
            }

            dimension_caches.insert(dimension.name.clone(), DimensionCache { id_map });
        }

        let level_map = get_level_map(&cube, ll_config)?;
        let property_map = get_property_map(&cube, ll_config)?;

        cubes.push(CubeCache {
            name: cube.name,
            year_level,
            year_values,
            quarter_level,
            quarter_values,
            month_level,
            month_values,
            week_level,
            week_values,
            day_level,
            day_values,
            time_level,
            time_values,
            level_map,
            property_map,
            level_caches,
            dimension_caches,
        })
    }

    let timing = time_start.elapsed();
    info!("Cache ready! (Time elapsed: {}.{:03})", timing.as_secs(), timing.subsec_millis());
    Ok(Cache { cubes })
}


pub fn get_unique_level_name(cube: &Cube, ll_config: Option<&LogicLayerConfig>, level_name: &LevelName) -> Result<Option<String>, Error> {
    for dimension in &cube.dimensions {
        for hierarchy in &dimension.hierarchies {
            for level in &hierarchy.levels {
                let curr_level_name = LevelName::new(
                    dimension.name.clone(),
                    hierarchy.name.clone(),
                    level.name.clone()
                );

                if curr_level_name == level_name.clone() {
                    let unique_level_name = match ll_config {
                        Some(ll_config) => {
                            let unique_level_name_opt = if dimension.is_shared {
                                ll_config.find_unique_shared_dimension_level_name(
                                    &dimension.name, &cube.name, &level_name
                                )?
                            } else {
                                ll_config.find_unique_cube_level_name(
                                    &cube.name, &level_name
                                )?
                            };

                            match unique_level_name_opt {
                                Some(unique_level_name) => unique_level_name,
                                None => level.name.clone()
                            }
                        },
                        None => level.name.clone()
                    };

                    return Ok(Some(unique_level_name))
                }
            }
        }
    }

    Ok(None)
}


pub fn get_level_map(cube: &Cube, ll_config: Option<&LogicLayerConfig>) -> Result<HashMap<String, LevelName>, Error> {
    let mut level_name_map = HashMap::new();

    for dimension in &cube.dimensions {
        for hierarchy in &dimension.hierarchies {
            for level in &hierarchy.levels {
                let level_name = LevelName::new(
                    dimension.name.clone(),
                    hierarchy.name.clone(),
                    level.name.clone()
                );

                let unique_level_name = match ll_config {
                    Some(ll_config) => {
                        let unique_level_name_opt = if dimension.is_shared {
                            ll_config.find_unique_shared_dimension_level_name(
                                &dimension.name, &cube.name, &level_name
                            )?
                        } else {
                            ll_config.find_unique_cube_level_name(
                                &cube.name, &level_name
                            )?
                        };

                        match unique_level_name_opt {
                            Some(unique_level_name) => unique_level_name,
                            None => level.name.clone()
                        }
                    },
                    None => level.name.clone()
                };

                level_name_map.insert(
                    unique_level_name.to_string(),
                    level_name
                );
            }
        }
    }

    Ok(level_name_map)
}


pub fn get_property_map(cube: &Cube, ll_config: Option<&LogicLayerConfig>) -> Result<HashMap<String, Property>, Error> {
    let mut property_map = HashMap::new();

    for dimension in &cube.dimensions {
        for hierarchy in &dimension.hierarchies {
            for level in &hierarchy.levels {
                if let Some(ref props) = level.properties {
                    for prop in props {
                        let property = Property::new(
                            dimension.name.clone(),
                            hierarchy.name.clone(),
                            level.name.clone(),
                            prop.name.clone()
                        );

                        let unique_property_name = match ll_config {
                            Some(ll_config) => {
                                let unique_property_name_opt = if dimension.is_shared {
                                    ll_config.find_unique_shared_dimension_property_name(
                                        &dimension.name, &cube.name, &property
                                    )?
                                } else {
                                    ll_config.find_unique_cube_property_name(
                                        &cube.name, &property
                                    )?
                                };

                                match unique_property_name_opt {
                                    Some(unique_property_name) => unique_property_name,
                                    None => prop.name.clone()
                                }
                            },
                            None => prop.name.clone()
                        };

                        property_map.insert(
                            unique_property_name.to_string(),
                            property
                        );
                    }
                }
            }
        }
    }

    Ok(property_map)
}


pub fn get_inline_parent_data(
        parent_level: &Level,
        current_level: &Level,
        inline_table: &InlineTable,
) -> HashMap<String, String> {
    let mut parent_data: HashMap<String, String> = HashMap::new();

    let mut parent_column: Vec<String> = vec![];
    let mut current_column: Vec<String> = vec![];

    for row in &inline_table.rows {
        for row_value in &row.row_values {
            if row_value.column == parent_level.key_column {
                parent_column.push(row_value.value.clone());
            } else if row_value.column == current_level.key_column {
                current_column.push(row_value.value.clone());
            }
        }
    }

    for i in 0..current_column.len() {
        parent_data.insert(current_column[i].clone(), parent_column[i].clone());
    }

    parent_data
}


pub fn get_inline_children_data(
        current_level: &Level,
        child_level: &Level,
        inline_table: &InlineTable,
) -> HashMap<String, Vec<String>> {
    let mut children_data: HashMap<String, Vec<String>> = HashMap::new();

    let mut current_column: Vec<String> = vec![];
    let mut children_column: Vec<String> = vec![];

    for row in &inline_table.rows {
        for row_value in &row.row_values {
            if row_value.column == current_level.key_column {
                current_column.push(row_value.value.clone());
            } else if row_value.column == child_level.key_column {
                children_column.push(row_value.value.clone());
            }
        }
    }

    let mut current_value: String = "".to_string();
    let mut current_children: Vec<String> = vec![];

    for i in 0..current_column.len() {
        if current_value == "".to_string() {
            current_value = current_column[i].clone();
            current_children.push(children_column[i].clone())
        } else {
            if current_column[i].clone() != current_value {
                children_data.insert(current_value.clone(), current_children.clone());
                current_children = vec![];
            }
            current_value = current_column[i].clone();
            current_children.push(children_column[i].clone())
        }
    }

    // Add last set of IDs
    children_data.insert(current_value, current_children);

    children_data
}


// Level moved because of some bug in async fn
// https://github.com/rust-lang/rust/issues/63033
async fn get_parent_data(
        parent_level: Level,
        current_level: Level,
        table: &str,
        backend: Box<dyn Backend + Sync + Send>,
) -> Result<HashMap<String, String>, Error> {
    let mut parent_data: HashMap<String, String> = HashMap::new();

    let df = backend
        .exec_sql(
            format!(
                "select distinct {0}, {1} from {2} group by {0}, {1} order by {0}, {1}",
                parent_level.key_column, current_level.key_column, table,
            ).to_string()
        ).await
        .map_err(|err| {
            format_err!("Error populating cache with backend data: {}", err)
        })?;

    let parent_column = df.columns[0].stringify_column_data();
    let current_column = df.columns[1].stringify_column_data();

    for i in 0..current_column.len() {
        parent_data.insert(current_column[i].clone(), parent_column[i].clone());
    }

    Ok(parent_data)
}


// Level moved because of some bug in async fn
// https://github.com/rust-lang/rust/issues/63033
async fn get_children_data(
        current_level: Level,
        child_level: Level,
        table: &str,
        backend: Box<dyn Backend + Sync + Send>,
) -> Result<HashMap<String, Vec<String>>, Error> {
    let mut children_data: HashMap<String, Vec<String>> = HashMap::new();

    let df = backend
        .exec_sql(
            format!(
                "select distinct {0}, {1} from {2} group by {0}, {1} order by {0}, {1}",
                current_level.key_column, child_level.key_column, table,
            ).to_string()
        ).await
        .map_err(|err| {
            format_err!("Error populating cache with backend data: {}", err)
        })?;

    let current_column = df.columns[0].stringify_column_data();
    let children_column = df.columns[1].stringify_column_data();

    let mut current_value: String = "".to_string();
    let mut current_children: Vec<String> = vec![];

    for i in 0..current_column.len() {
        if current_value == "".to_string() {
            current_value = current_column[i].clone();
            current_children.push(children_column[i].clone())
        } else {
            if current_column[i].clone() != current_value {
                children_data.insert(current_value.clone(), current_children.clone());
                current_children = vec![];
            }
            current_value = current_column[i].clone();
            current_children.push(children_column[i].clone())
        }
    }

    // Add last set of IDs
    children_data.insert(current_value, current_children);

    Ok(children_data)
}


// String moved because of some bug in async fn
// https://github.com/rust-lang/rust/issues/63033
/// Queries the database to get all the distinct values for a given level.
async fn get_distinct_values(
        column: String,
        table: String,
        backend: Box<dyn Backend + Sync + Send>,
) -> Result<Vec<String>, Error> {
    let mut df = backend
        .exec_sql(
            format!("select distinct {} from {}", column, table).to_string()
        ).await
        .map_err(|err| {
            format_err!("Error populating cache with backend data: {}", err)
        })?;

    if df.columns.len() >= 1 {
        df.columns[0].sort_column_data()?;
        let values: Vec<String> = df.columns[0].stringify_column_data();
        return Ok(values);
    }

    return Ok(vec![]);
}


pub fn get_neighbors_map(distinct_ids: &Vec<String>) -> HashMap<String, Vec<String>> {
    let mut neighbors_map: HashMap<String, Vec<String>> = HashMap::new();

    // Populate neighbors map
    let mut prev = 0;
    let mut curr = 0;
    let mut next = 2;

    let max_index = distinct_ids.len();

    let mut done = false; // mut done: bool

    while !done {
        // Before
        let before = if prev == 0 && curr <= 1 {
            distinct_ids[0..curr].to_vec()
        } else {
            distinct_ids[prev..curr].to_vec()
        };

        // After
        let after = if next >= max_index {
            distinct_ids[curr+1..].to_vec()
        } else {
            distinct_ids[curr+1..next+1].to_vec()
        };

        neighbors_map.insert(distinct_ids[curr].clone(), [&before[..], &after[..]].concat());

        if curr >= 2 {
            prev += 1;
        }
        curr += 1;
        next += 1;

        if curr == max_index {
            done = true;
        }
    }

    neighbors_map
}
