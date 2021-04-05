#![allow(dead_code)]
/// Construct, parse, and display full qualified names
/// for Mondrian schema:
/// - drilldown
/// - measure
/// - cut
/// - property

// Structs for creating fully qualified names
// for query parameters
//
// Implement display for all of them so that they
// can be formatted to a string for joining to
// a url.
//
// Implement FromStr to be able to easily parse
// a small variety of names.
// - [Dimension].[Hierarchy].[Level]
// - Dimension.Hierarchy.Level
// - Dimension.Level
// etc.

use anyhow::{Error, bail, format_err, ensure};
use serde_derive::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;


/// Fully qualified name of Dimension, Hierarchy, and Level
/// Basis for other names.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Eq, Hash)]
pub struct LevelName {
    pub dimension: String,
    pub hierarchy: String,
    pub level: String,
}

impl LevelName {
    pub fn new<S: Into<String>>(dimension: S, hierarchy: S, level: S) -> Self {
        LevelName {
            dimension: dimension.into(),
            hierarchy: hierarchy.into(),
            level: level.into(),
        }
    }

    /// Names must have already been trimmed of [] delimiters.
    pub fn from_vec<S: Into<String> + Clone>(level_name: Vec<S>) -> Result<Self, Error> 
    {
        if level_name.len() == 3 {
            Ok(LevelName {
                dimension: level_name[0].clone().into(),
                hierarchy: level_name[1].clone().into(),
                level: level_name[2].clone().into(),
            })
        } else if level_name.len() == 2 {
            Ok(LevelName {
                dimension: level_name[0].clone().into(),
                hierarchy: level_name[0].clone().into(),
                level: level_name[1].clone().into(),
            })
        } else {
            bail!(
                "Dimension {:?} does not follow naming convention",
                level_name.into_iter().map(|s| s.into()).collect::<Vec<String>>()
            );
        }
    }

    pub fn dimension(&self) -> &str {
        self.dimension.as_str()
    }

    pub fn hierarchy(&self) -> &str {
        self.hierarchy.as_str()
    }

    pub fn level(&self) -> &str {
        self.level.as_str()
    }
}

impl fmt::Display for LevelName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}].[{}].[{}]", self.dimension, self.hierarchy, self.level)
    }
}

impl FromStr for LevelName {
    type Err = Error;

    /// NAIVE IMPL, does not deal with escaped brackets,
    /// unpaired brackets, etc. Just checks whether the
    /// first char is a bracket to determine whether to
    /// parse using brackets or not.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let name_iter = if s.chars().nth(0).unwrap() == '[' {
            // check if starts with '[', then assume
            // that this means that it's a qualified name
            // with [] wrappers. This means that can't just
            // split on any periods, only periods that fall
            // outside the []
            let pattern: &[_] = &['[', ']'];
            let s = s.trim_matches(pattern);
            s.split("].[")
        } else {
            s.split(".")
        };

        LevelName::from_vec(name_iter.collect())
    }
}


#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Drilldown(pub LevelName);

impl Drilldown {
    pub fn new<S: Into<String>>(dimension: S, hierarchy: S, level: S) -> Self {
        Drilldown(
            LevelName::new(dimension, hierarchy, level)
        )
    }

    /// Names must have already been trimmed of [] delimiters.
    pub fn from_vec<S: Into<String> + Clone>(drilldown: Vec<S>) -> Result<Self, Error> 
    {
        LevelName::from_vec(drilldown).map(|x| Drilldown(x))
    }
}

impl fmt::Display for Drilldown {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}].[{}].[{}]", self.0.dimension, self.0.hierarchy, self.0.level)
    }
}

impl FromStr for Drilldown {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<LevelName>().map(|level_name| Drilldown(level_name))
    }
}


/// Naive impl, does not check that [Measure]. is NOT
/// prepended. But does remove brackets on FromStr
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Measure(pub String);

impl Measure {
    pub fn new<S: Into<String>>(measure: S) -> Self {
        Measure(measure.into())
    }
}

impl fmt::Display for Measure{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Measure {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pattern: &[_] = &['[', ']'];
        let s = s.trim_matches(pattern);
        Ok(Measure(s.to_owned()))
    }
}


/// Note: FromStr impl aggressively left trims ampersands
/// from the beginning of member list and from the
/// beginning of each member
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Cut {
    // if mask is include, includes indicated members in the cut.
    // if mask is Exclude (exclude, negation), excludes members in the cut and includes all others.
    pub level_name: LevelName,
    pub members: Vec<String>,
    pub mask: Mask,
    pub for_match: bool,
}

impl Cut {
    pub fn new<S: Into<String>>(
        dimension: S,
        hierarchy: S,
        level: S,
        members: Vec<S>,
        mask: Mask,
        for_match: bool,
        ) -> Self
    {
        Cut {
            level_name: LevelName::new(dimension, hierarchy, level),
            members: members.into_iter().map(|s| s.into()).collect(),
            mask,
            for_match,
        }
    }

    /// Names must have already been trimmed of [] delimiters.
    pub fn from_vec<S: Into<String> + Clone>(cut_level: Vec<S>, members: Vec<S>, mask: Mask, for_match: bool) -> Result<Self, Error>
    {
        ensure!(members.len() > 0, "No members found");

        // TODO get rid of clones
        Ok(LevelName::from_vec(cut_level.clone())
            .map(|level_name| {
                Cut {
                    level_name,
                    members: members.clone().into_iter().map(|s| s.into()).collect(),
                    mask,
                    for_match,
                }
            })
            .map_err(|err| {
                err.context(format_err!(
                    "Dimension {:?}, {:?} does not follow naming convention",
                    cut_level.into_iter().map(|s| s.into()).collect::<Vec<String>>(),
                    members.into_iter().map(|s| s.into()).collect::<Vec<String>>(),
                ))
            })?)
    }

    /// Parses a cut string and returns a boolean containing a Mask, for_match
    /// bool and the final cut string.
    pub fn parse_cut(cut: &str) -> (Mask, bool, String) {
        // Check for mask
        let is_exclude = cut.chars().nth(0).unwrap() == '~';
        let mask = if is_exclude {
            Mask::Exclude
        } else {
            Mask::Include
        };
        let cut = if is_exclude {
            // ok to slice string, because '~' is definitely one char
            &cut[1..]
        } else {
            cut
        };

        // Then check for match (*)
        let for_match = cut.chars().nth(0).unwrap() == '*';
        let cut = if for_match {
            // ok to slice string, because '*' is definitely one char
            &cut[1..]
        } else {
            cut
        };

        (mask, for_match, cut.to_owned())
    }
}

// TODO fix this, it only displays "keys" and not "labels"
impl fmt::Display for Cut {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // members must be more than 0, checked by assert on serialization
        if self.members.len() == 1 {
            write!(f, "{}{}.&[{}]", self.mask, self.level_name, self.members[0])
        } else {
            let mut out = String::new();
            out.push_str(&format!("{}", self.mask));
            out.push('{');

            let mut members = self.members.iter();
            out.push_str(
                format!(
                    "{}.&[{}]",
                    self.level_name, members.next().unwrap()
                ).as_str()
            );

            for member in members {
                out.push_str(",");
                out.push_str(format!("{}.&[{}]", self.level_name, member).as_str());
            }
            out.push('}');

            write!(f, "{}", out)
        }
    }
}

// TODO I should use a parser for this, just splitting is kind of
// a pain
impl FromStr for Cut {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // first check for mask value (~)
        let is_exclude = s.chars().nth(0).unwrap() == '~';
        let mask = if is_exclude {
            Mask::Exclude
        } else {
            Mask::Include
        };
        let s = if is_exclude {
            // ok to slice string, because '~' is definitely one char
            &s[1..]
        } else {
            s
        };

        // then check for match (*)
        let for_match = s.chars().nth(0).unwrap() == '*';
        let s = if for_match {
            // ok to slice string, because '~' is definitely one char
            &s[1..]
        } else {
            s
        };

        // then do rest of processing normally
        let name_vec: Vec<_> = if s.chars().nth(0).unwrap() == '[' {
            // check if starts with '[', then assume
            // that this means that it's a qualified name
            // with [] wrappers. This means that can't just
            // split on any periods, only periods that fall
            // outside the []
            let pattern: &[_] = &['[', ']'];

            // TODO maybe this should be split into
            // left and right trims
            //
            // This is to deal with case where
            // & is inserted before member, which
            // "].[" won't match
            let s = s.trim_matches(pattern);
            s.split("].")
                .map(|s| s.trim_start_matches('['))
                .collect()
        } else {
            s.split(".")
                .collect()
        };

        let members: Vec<_> = name_vec[name_vec.len()-1]
            .trim_start_matches('&')
            .trim_start_matches('[')
            .split(',')
            .map(|s| s.trim_start_matches('&').to_owned())
            .collect();

        Ok(Cut {
            level_name: LevelName::from_vec(name_vec[0..name_vec.len()-1].to_vec())?,
            members,
            mask,
            for_match,
        })
    }
}


#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum Mask {
    Include,
    Exclude,
}

impl fmt::Display for Mask {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Mask::Include => write!(f, ""),
            Mask::Exclude => write!(f, "~"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Property {
    pub level_name: LevelName,
    pub property: String,
}

impl Property {
    pub fn new<S: Into<String>>(
        dimension: S,
        hierarchy: S,
        level: S,
        property: S,
        ) -> Self
    {
        Property {
            level_name: LevelName::new(dimension, hierarchy, level),
            property: property.into(),
        }
    }

    /// Names must have already been trimmed of [] delimiters.
    pub fn from_vec<S: Into<String> + Clone>(property: Vec<S>) -> Result<Self, Error> 
    {
        Ok(LevelName::from_vec(property[0..property.len()-1].to_vec())
            .map(|level_name| {
                Property {
                    level_name,
                    property: property[property.len()-1].clone().into(),
                }
            })
            .map_err(|err| {
                err.context(format_err!(
                    "Dimension {:?} does not follow naming convention",
                    property.into_iter().map(|s| s.into()).collect::<Vec<String>>()
                ))
            })?)
    }

    /// returns the level in Drilldown form.
    /// Used in testing to be able to drilldown and get property
    /// simultaneously
    pub fn drill_level(&self) -> Drilldown {
        Drilldown(self.level_name.clone())
    }
}

impl fmt::Display for Property {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.[{}]", self.level_name, self.property)
    }
}

impl FromStr for Property {
    type Err = Error;

    /// NAIVE IMPL, does not deal with escaped brackets,
    /// unpaired brackets, etc. Just checks whether the
    /// first char is a bracket to determine whether to
    /// parse using brackets or not.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let name_vec: Vec<_> = (if s.chars().nth(0).unwrap() == '[' {
            // check if starts with '[', then assume
            // that this means that it's a qualified name
            // with [] wrappers. This means that can't just
            // split on any periods, only periods that fall
            // outside the []
            let pattern: &[_] = &['[', ']'];
            let s = s.trim_matches(pattern);
            s.split("].[")
        } else {
            s.split(".")
        }).collect();

        Ok(Property {
            level_name: LevelName::from_vec(name_vec[0..name_vec.len() - 1].to_vec())?,
            property: name_vec[name_vec.len() - 1].to_owned(),
        })
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_level_name() {
        let level = LevelName::new("Geography", "Geography", "County");
        let level_from_vec_1 = LevelName::from_vec(vec!["Geography", "Geography", "County"]).unwrap();
        let level_from_vec_2 = LevelName::from_vec(vec!["Geography", "County"]).unwrap();

        assert_eq!(level, level_from_vec_1);
        assert_eq!(level, level_from_vec_2);
    }

    #[test]
    #[should_panic]
    fn test_level_name_bad_1() {
        LevelName::from_vec(vec!["Geography", "Geography", "County", "County"]).unwrap();
    }
    #[test]
    #[should_panic]
    fn test_level_name_bad_2() {
        LevelName::from_vec(vec!["County"]).unwrap();
    }

    #[test]
    fn test_drilldown() {
        let drilldown = Drilldown::new("Geography", "Geography", "County");
        let drilldown_from_vec = Drilldown::from_vec(
            vec!["Geography", "County"],
            ).unwrap();

        assert_eq!(drilldown, drilldown_from_vec);
    }

    #[test]
    fn test_cut() {
        let cut = Cut::new("Geography", "Geography", "County", vec!["1", "2"], Mask::Include, false);
        let cut_from_vec = Cut::from_vec(
            vec!["Geography", "County"],
            vec!["1", "2"],
            Mask::Include,
            false
            ).unwrap();

        assert_eq!(cut, cut_from_vec);
    }

    #[test]
    fn test_property() {
        let property = Property::new("Geography", "Geography", "County", "name_en");
        let property_from_vec = Property::from_vec(
            vec!["Geography", "County", "name_en"],
            ).unwrap();

        assert_eq!(property, property_from_vec);
    }

    #[test]
    #[ignore]
    fn test_display() {
        let level = LevelName::new("Geography", "Geography", "County");
        let drilldown = Drilldown::new("Geography", "Geography", "County");
        let cut1 = Cut::new("Geography", "Geography", "County", vec!["1"], Mask::Include, false);
        let cut2 = Cut::new("Geography", "Geography", "County", vec!["1", "2"], Mask::Include, false);
        let cut2_not = Cut::new("Geography", "Geography", "County", vec!["1", "2"], Mask::Exclude, false);
        let property = Property::new("Geography", "Geography", "County", "name_en");

        println!("{}", level);
        println!("{}", drilldown);
        println!("{}", cut1);
        println!("{}", cut2);
        println!("{}", cut2_not);
        println!("{}", property);

        panic!();
    }

    #[test]
    fn test_parse() {
        // Currently supported syntaxes. No guarantees for more complex cases

        let level = LevelName::new("Geography", "Geography", "County");
        let drilldown = Drilldown::new("Geography", "Geography", "County");
        let cut1 = Cut::new("Geography", "Geography", "County", vec!["1"], Mask::Include, false);
        let cut2 = Cut::new("Geography", "Geography", "County", vec!["1", "2"], Mask::Include, false);
        let cut2_not = Cut::new("Geography", "Geography", "County", vec!["1", "2"], Mask::Exclude, false);
        let property = Property::new("Geography", "Geography", "County", "name_en");

        // test level_name
        let level_test_1 = "Geography.Geography.County".parse::<LevelName>().unwrap();
        let level_test_2 = "[Geography].[Geography].[County]".parse::<LevelName>().unwrap();
        let level_test_3 = "Geography.County".parse::<LevelName>().unwrap();

        assert_eq!(level, level_test_1);
        assert_eq!(level, level_test_2);
        assert_eq!(level, level_test_3);

        // test_drilldown
        let drilldown_test_1 = "Geography.Geography.County".parse::<Drilldown>().unwrap();
        let drilldown_test_2 = "[Geography].[Geography].[County]".parse::<Drilldown>().unwrap();
        let drilldown_test_3 = "Geography.County".parse::<Drilldown>().unwrap();

        assert_eq!(drilldown, drilldown_test_1);
        assert_eq!(drilldown, drilldown_test_2);
        assert_eq!(drilldown, drilldown_test_3);

        // test cut1
        let cut1_test_1 = "Geography.Geography.County.1".parse::<Cut>().unwrap();
        let cut1_test_2 = "[Geography].[Geography].[County].&[1]".parse::<Cut>().unwrap();
        let cut1_test_3 = "Geography.County.1".parse::<Cut>().unwrap();

        assert_eq!(cut1, cut1_test_1);
        assert_eq!(cut1, cut1_test_2);
        assert_eq!(cut1, cut1_test_3);

        // test cut2
        let cut2_test_1 = "Geography.Geography.County.1,2".parse::<Cut>().unwrap();
        let cut2_test_2 = "[Geography].[Geography].[County].&[1,2]".parse::<Cut>().unwrap();
        let cut2_test_3 = "Geography.County.1,2".parse::<Cut>().unwrap();
        let cut2_test_4 = "Geography.County.&1,2".parse::<Cut>().unwrap();
        let cut2_test_5 = "Geography.County.&1,&2".parse::<Cut>().unwrap();

        assert_eq!(cut2, cut2_test_1);
        assert_eq!(cut2, cut2_test_2);
        assert_eq!(cut2, cut2_test_3);
        assert_eq!(cut2, cut2_test_4);
        assert_eq!(cut2, cut2_test_5);

        let cut2_test_1_not = "~Geography.Geography.County.1,2".parse::<Cut>().unwrap();
        assert_eq!(cut2_not, cut2_test_1_not);

        // test property
        let property_test_1 = "Geography.Geography.County.name_en".parse::<Property>().unwrap();
        let property_test_2 = "[Geography].[Geography].[County].[name_en]".parse::<Property>().unwrap();
        let property_test_3 = "Geography.County.name_en".parse::<Property>().unwrap();

        assert_eq!(property, property_test_1);
        assert_eq!(property, property_test_2);
        assert_eq!(property, property_test_3);
    }
}

