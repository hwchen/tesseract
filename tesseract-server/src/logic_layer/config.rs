use anyhow::{Error, format_err};
use std::collections::{HashMap, HashSet};

use serde_derive::Deserialize;
use serde_json;
use tesseract_core::{Schema, CubeHasUniqueLevelsAndProperties};
use tesseract_core::names::{LevelName, Property};


#[derive(Debug, Clone, Deserialize)]
pub struct LogicLayerConfig {
    pub aliases: Option<AliasConfig>,
    pub named_sets: Option<Vec<NamedSetsConfig>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AliasConfig {
    pub cubes: Option<Vec<CubeAliasConfig>>,
    pub shared_dimensions: Option<Vec<SharedDimensionAliasConfig>>
}

// TODO: Remove requirement for `alternatives`
#[derive(Debug, Clone, Deserialize)]
pub struct CubeAliasConfig {
    pub name: String,
    pub alternatives: Vec<String>,
    pub levels: Option<Vec<LevelPropertyConfig>>,
    pub properties: Option<Vec<LevelPropertyConfig>>
}

#[derive(Debug, Clone, Deserialize)]
pub struct SharedDimensionAliasConfig {
    pub name: String,
    pub levels: Option<Vec<LevelPropertyConfig>>,
    pub properties: Option<Vec<LevelPropertyConfig>>
}

#[derive(Debug, Clone, Deserialize)]
pub struct NamedSetsConfig {
    pub level_name: String,
    pub sets: Vec<NamedSetConfig>
}

#[derive(Debug, Clone, Deserialize)]
pub struct NamedSetConfig {
    pub set_name: String,
    pub values: Vec<String>
}

#[derive(Debug, Clone, Deserialize)]
pub struct LevelPropertyConfig {
    pub current_name: String,
    pub unique_name: String
}



pub trait GetLevels {
    fn get_levels(&self) -> Option<Vec<LevelPropertyConfig>>;
}

impl GetLevels for CubeAliasConfig {
    fn get_levels(&self) -> Option<Vec<LevelPropertyConfig>> {
        self.levels.clone()
    }
}

impl GetLevels for SharedDimensionAliasConfig {
    fn get_levels(&self) -> Option<Vec<LevelPropertyConfig>> {
        self.levels.clone()
    }
}

pub trait GetProperties {
    fn get_properties(&self) -> Option<Vec<LevelPropertyConfig>>;
}

impl GetProperties for CubeAliasConfig {
    fn get_properties(&self) -> Option<Vec<LevelPropertyConfig>> {
        self.properties.clone()
    }
}

impl GetProperties for SharedDimensionAliasConfig {
    fn get_properties(&self) -> Option<Vec<LevelPropertyConfig>> {
        self.properties.clone()
    }
}

pub fn find_unique_level_name<T>(
        level_name: &LevelName, levels_obj: &T
    ) -> Result<Option<String>, Error> where T: GetLevels
{
    let levels = levels_obj.get_levels();

    if let Some(levels) = &levels {
        for level in levels {
            let ll_level_name: LevelName = level.current_name.parse()?;

            if &ll_level_name == level_name {
                return Ok(Some(level.unique_name.clone()))
            }
        }
    }

    Ok(None)
}

pub fn find_unique_property_name<T>(
    property_name: &Property, properties_obj: &T
) -> Result<Option<String>, Error> where T: GetProperties
{
    let properties = properties_obj.get_properties();

    if let Some(properties) = &properties {
        for property in properties {
            let ll_property_name: Property = property.current_name.parse()?;

            if &ll_property_name == property_name {
                return Ok(Some(property.unique_name.clone()))
            }
        }
    }

    Ok(None)
}

pub fn read_config_str(config_str: &str) -> Result<LogicLayerConfig, Error> {
    let config = match serde_json::from_str::<LogicLayerConfig>(&config_str) {
        Ok(config) => config,
        Err(err) => {
            return Err(format_err!("Unable to read logic layer config: {}", err))
        }
    };

    if let Some(named_sets) = &config.named_sets {
        let mut set_names = HashSet::new();

        for named_set in named_sets.iter() {
            for set in named_set.sets.iter() {
                if !set_names.insert(set.set_name.clone()) {
                    return Err(format_err!("Make sure the logic layer config has unique set names"))
                }
            }
        }
        return Ok(config)
    } else {
        return Ok(config)
    }
}

/// Reads Logic Layer Config JSON file.
pub fn read_config(config_path: &String) -> Result<LogicLayerConfig, Error> {
    let config_str = std::fs::read_to_string(&config_path)
        .map_err(|_| format_err!("Logic layer config file not found at {}", config_path))?;

    read_config_str(&config_str)
}

impl LogicLayerConfig {
    /// Returns a HashMap of current level signatures to unique level signatures
    /// for a given cube name.
    pub fn get_unique_names_map(&self, cube_name: String) -> HashMap<String, String> {
        let mut unique_header_map: HashMap<String, String> = HashMap::new();

        if let Some(ref aliases) = self.aliases {
            if let Some(ref llc_cubes) = aliases.cubes {
                for llc_cube in llc_cubes {
                    if cube_name == llc_cube.name {
                        if let Some(ref llc_cube_levels) = llc_cube.levels {
                            for llc_cube_level in llc_cube_levels {
                                unique_header_map.insert(
                                    llc_cube_level.current_name.clone(),
                                    llc_cube_level.unique_name.clone()
                                );
                            }
                        }

                        if let Some(ref llc_cube_properties) = llc_cube.properties {
                            for llc_cube_property in llc_cube_properties {
                                unique_header_map.insert(
                                    llc_cube_property.current_name.clone(),
                                    llc_cube_property.unique_name.clone()
                                );
                            }
                        }
                    }
                }
            }
        }

        unique_header_map
    }

    /// Given a cube name, loops over the LogicLayerConfig and returns the
    /// actual cube name if an alias was provided.
    pub fn substitute_cube_name(self, name: String) -> Result<String, Error> {
        match self.aliases {
            Some(aliases) => {
                match aliases.cubes {
                    Some(cubes) => {
                        for cube in cubes {
                            for alt in cube.alternatives {
                                if alt == name {
                                    return Ok(cube.name);
                                }
                            }
                        }
                        return Ok(name)
                    },
                    None => return Ok(name)
                }
            },
            None => return Ok(name)
        };
    }

    /// Given a drilldown level name, try to match that to one of the config
    /// named set names. If there is a match, return the associated level name
    /// for that named set.
    pub fn substitute_drill_value(self, level_name: String) -> Option<String> {
        if let Some(named_sets) = &self.named_sets {
            for named_set in named_sets.iter() {
                for set in named_set.sets.iter() {
                    if set.set_name == level_name {
                        return Some(named_set.level_name.clone())
                    }
                }
            }
        }
        None
    }

    /// Given a cut string, find if that matches any of the substitutions
    /// defined in `named_sets`. If so, substitute the cut value.
    pub fn substitute_cut(self, level_name: String, cut: String) -> String {
        match self.named_sets {
            Some(named_sets) => {
                let cuts: Vec<String> = cut.split(",").map(|s| s.to_string()).collect();

                let mut final_cuts: Vec<String> = vec![];

                for c in cuts.clone() {
                    let mut found = false;

                    for named_set in named_sets.clone() {
                        if named_set.level_name == level_name {
                            for set in named_set.sets.clone() {
                                if c == set.set_name {
                                    final_cuts.extend(set.values);
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }

                    // No substitutions found, so just add the raw cut
                    if found == false {
                        final_cuts.push(c);
                    }
                }

                final_cuts.join(",")
            },
            None => cut
        }
    }

    /// Returns a unique name definition for a given cube level if there is one.
    pub fn find_unique_cube_level_name(
        &self, cube_name: &String, level_name: &LevelName
    ) -> Result<Option<String>, Error> {
        if let Some(aliases) = &self.aliases {
            if let Some(cubes) = &aliases.cubes {
                for cube in cubes {
                    if &cube.name == cube_name {
                        let res = find_unique_level_name(&level_name, cube)?;
                        if let Some(res) = res {
                            return Ok(Some(res))
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    /// Returns a unique name definition for a given shared dimension level if there is one.
    pub fn find_unique_shared_dimension_level_name(
        &self, shared_dimension_name: &String, cube_name: &String, level_name: &LevelName
    ) -> Result<Option<String>, Error> {
        if let Some(aliases) = &self.aliases {
            // Checks if there is a more specific definition in `cubes` first
            if let Some(cubes) = &aliases.cubes {
                for cube in cubes {
                    if &cube.name == cube_name {
                        let res = find_unique_level_name(&level_name, cube)?;
                        if let Some(res) = res {
                            return Ok(Some(res))
                        }
                    }
                }
            }

            if let Some(shared_dimensions) = &aliases.shared_dimensions {
                for shared_dimension in shared_dimensions {
                    if &shared_dimension.name == shared_dimension_name {
                        let res = find_unique_level_name(&level_name, shared_dimension)?;
                        if let Some(res) = res {
                            return Ok(Some(res))
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Returns a  vector of String containing the alternative names for a given cube if there is one
    pub fn find_cube_aliases(
        &self, cube_name: &String,
    ) -> Option<Vec<String>> {
        if let Some(aliases) = &self.aliases {
            if let Some(cubes) = &aliases.cubes{
                for cube in cubes {
                    if &cube.name == cube_name {
                        let res = cube.alternatives.clone();
                        if res.len() != 0{
                            return Some(res)
                        }
                    }
                }
            }
        }
        None
    }

    /// Returns a unique name definition for a given cube property if there is one.
    pub fn find_unique_cube_property_name(
        &self, cube_name: &String, property_name: &Property
    ) -> Result<Option<String>, Error> {
        if let Some(aliases) = &self.aliases {
            if let Some(cubes) = &aliases.cubes {
                for cube in cubes {
                    if &cube.name == cube_name {
                        let res = find_unique_property_name(&property_name, cube)?;
                        if let Some(res) = res {
                            return Ok(Some(res))
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    /// Returns a unique name definition for a given shared dimension property if there is one.
    pub fn find_unique_shared_dimension_property_name(
        &self, shared_dimension_name: &String, cube_name: &String, property_name: &Property
    ) -> Result<Option<String>, Error> {
        if let Some(aliases) = &self.aliases {
            // Checks if there is a more specific definition in `cubes` first
            if let Some(cubes) = &aliases.cubes {
                for cube in cubes {
                    if &cube.name == cube_name {
                        let res = find_unique_property_name(&property_name, cube)?;
                        if let Some(res) = res {
                            return Ok(Some(res))
                        }
                    }
                }
            }

            if let Some(shared_dimensions) = &aliases.shared_dimensions {
                for shared_dimension in shared_dimensions {
                    if &shared_dimension.name == shared_dimension_name {
                        let res = find_unique_property_name(&property_name, shared_dimension)?;
                        if let Some(res) = res {
                            return Ok(Some(res))
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    /// Ensures level and property names are unique inside each cube based on
    /// name substitutions from a logic layer configuration.
    pub fn has_unique_levels_properties(&self, schema: &Schema) -> Result<CubeHasUniqueLevelsAndProperties, Error> {
        for cube in &schema.cubes {
            let mut levels = HashSet::new();
            let mut properties = HashSet::new();

            for dimension in &cube.dimensions {
                for hierarchy in &dimension.hierarchies {

                    // Check each cube for unique level and property names
                    for level in &hierarchy.levels {
                        let level_name = LevelName::new(
                            dimension.name.clone(),
                            hierarchy.name.clone(),
                            level.name.clone()
                        );

                        let unique_level_name_opt = if dimension.is_shared {
                            self.find_unique_shared_dimension_level_name(
                                &dimension.name, &cube.name, &level_name
                            )?
                        } else {
                            self.find_unique_cube_level_name(
                                &cube.name, &level_name
                            )?
                        };

                        let unique_level_name = match unique_level_name_opt {
                            Some(unique_level_name) => unique_level_name,
                            None => level.name.clone()
                        };

                        // TODO remove this clone?
                        if !levels.insert(unique_level_name.clone()) {
                            return Ok(CubeHasUniqueLevelsAndProperties::False {
                                cube: cube.name.clone(),
                                name: unique_level_name.to_string(),
                            })
                        }

                        if let Some(ref props) = level.properties {
                            for property in props {
                                let property_name = Property::new(
                                    dimension.name.clone(),
                                    hierarchy.name.clone(),
                                    level.name.clone(),
                                    property.name.clone()
                                );

                                let unique_property_name_opt = if dimension.is_shared {
                                    self.find_unique_shared_dimension_property_name(
                                        &dimension.name, &cube.name, &property_name
                                    )?
                                } else {
                                    self.find_unique_cube_property_name(
                                        &cube.name, &property_name
                                    )?
                                };

                                let unique_property_name = match unique_property_name_opt {
                                    Some(unique_property_name) => unique_property_name,
                                    None => property.name.clone()
                                };

                                if !properties.insert(unique_property_name) {
                                    return Ok(CubeHasUniqueLevelsAndProperties::False {
                                        cube: cube.name.clone(),
                                        name: property_name.to_string(),
                                    })
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(CubeHasUniqueLevelsAndProperties::True)
    }
}
