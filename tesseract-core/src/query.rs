use itertools::join;

use anyhow::{Error, format_err, bail};
use std::str::FromStr;

use crate::names::{
    Cut,
    Drilldown,
    Measure,
    Property,
    LevelName,
};

#[derive(Debug, Clone)]
pub struct Query {
    pub cuts: Vec<Cut>,
    pub drilldowns: Vec<Drilldown>,
    pub measures: Vec<Measure>,
    pub properties: Vec<Property>,
    pub filters: Vec<FilterQuery>,
    pub captions: Vec<Property>,
    pub parents: bool,
    pub top: Option<TopQuery>,
    pub top_where: Option<TopWhereQuery>,
    pub sort: Option<SortQuery>,
    pub limit: Option<LimitQuery>,
    pub rca: Option<RcaQuery>,
    pub growth: Option<GrowthQuery>,
    pub rate: Option<RateQuery>,
    pub debug: bool,
    pub sparse: bool,
    pub exclude_default_members: bool,
}

impl Query {
    pub fn new() -> Self {
        Query {
            drilldowns: vec![],
            cuts: vec![],
            measures: vec![],
            properties: vec![],
            filters: vec![],
            captions: vec![],
            parents: false,
            top: None,
            top_where: None,
            sort: None,
            limit: None,
            rca: None,
            growth: None,
            rate: None,
            debug: false,
            sparse: false,
            exclude_default_members: false,
        }
    }
}

// TODO: Move ClickHouse specific queries away from ts-core

/// ClickHouse:
/// select * from table_name order by sort_measures sort_direction
/// limit n by by_dimension
#[derive(Debug, Clone)]
pub struct TopQuery {
    pub n: u64,
    pub by_dimension: LevelName,
    pub sort_mea_or_calc: Vec<MeaOrCalc>,
    pub sort_direction: SortDirection,
}

impl TopQuery  {
    pub fn new(
        n: u64, by_dimension: LevelName, sort_mea_or_calc: Vec<MeaOrCalc>,
        sort_direction: SortDirection
    ) -> Self {
        TopQuery {
            n,
            by_dimension,
            sort_mea_or_calc,
            sort_direction
        }
    }
}

// Currently only allows one sort_measure
impl FromStr for TopQuery {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &s.split(",").collect::<Vec<_>>()[..] {
            [n, by_dimension, sort_measure, sort_direction] => {

                let n = n.parse::<u64>()?;
                let by_dimension = by_dimension.parse::<LevelName>()?;
                let sort_mea_or_calc = vec![sort_measure.parse::<MeaOrCalc>()?];
                let sort_direction = sort_direction.parse::<SortDirection>()?;

                Ok(TopQuery {
                    n,
                    by_dimension,
                    sort_mea_or_calc,
                    sort_direction,
                })
            },
            _ => bail!("Could not parse a top query"),
        }
    }
}

// Just for TopQuery
/// Currently rca and growth will be reserved keywords. This may be changed in the future,
/// to allow measures that are named rca and growth
#[derive(Debug, Clone, PartialEq)]
pub enum MeaOrCalc {
    Mea(Measure),
    Calc(Calculation),
}

impl FromStr for MeaOrCalc {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<Calculation>()
            .map(|c| MeaOrCalc::Calc(c))
            .or_else(|_| {
                s.parse::<Measure>()
                    .map(|m| MeaOrCalc::Mea(m))
            })
            .map_err(|_| format_err!("Could not parse '{}' to measure name or built-in calculation name", s))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Calculation {
    Rca,
    Growth,
}

impl Calculation {
    pub(crate) fn sql_string(&self) -> String {
        match self {
            Calculation::Rca => "rca".to_owned(),
            Calculation::Growth => "growth".to_owned(),
        }
    }
}

impl FromStr for Calculation {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &s.to_lowercase()[..] {
            "rca" => Ok(Calculation::Rca),
            "growth" => Ok(Calculation::Growth),
            _ => Err(format_err!("'{}' is not a supported calculation", s)),
        }
    }
}

/// For filtering on a measure before Top is calculated
#[derive(Debug, Clone)]
pub struct TopWhereQuery {
    pub by_mea_or_calc: MeaOrCalc,
    pub constraint: Constraint,
}

// Currently only allows one sort_measure
impl FromStr for TopWhereQuery {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &s.split(",").collect::<Vec<_>>()[..] {
            [by_mea, constraint] => {

                let by_mea_or_calc = by_mea.parse::<MeaOrCalc>()?;
                let constraint = constraint.parse::<Constraint>()?;

                Ok(TopWhereQuery {
                    by_mea_or_calc,
                    constraint,
                })
            },
            _ => bail!("Could not parse a top_where query"),
        }
    }
}

// Constraint: less than, greater than a number
// This is a little less straightforward, so we should
// probably test this
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub comparison: Comparison,
    pub n: f64,
}

impl Constraint {
    pub fn sql_string(&self) -> String {
        format!("{} {}", self.comparison.sql_string(), self.n)
    }
}

impl FromStr for Constraint {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &s.splitn(2, ".").collect::<Vec<_>>()[..] {
            [comparison, n] => {

                let comparison = comparison.parse::<Comparison>()?;
                let n = n.parse::<f64>()?;

                Ok(Constraint {
                    comparison,
                    n,
                })
            },
            _ => bail!("Could not parse a Constraint"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Comparison {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

impl Comparison {
    pub fn sql_string(&self) -> String {
        match self {
            Comparison::Equal => "=".to_owned(),
            Comparison::NotEqual => "<>".to_owned(),
            Comparison::LessThan => "<".to_owned(),
            Comparison::LessThanOrEqual => "<=".to_owned(),
            Comparison::GreaterThan => ">".to_owned(),
            Comparison::GreaterThanOrEqual => ">=".to_owned(),
        }
    }
}

impl FromStr for Comparison {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "eq" => Ok(Comparison::Equal),
            "neq" => Ok(Comparison::NotEqual),
            "lt" => Ok(Comparison::LessThan),
            "lte" => Ok(Comparison::LessThanOrEqual),
            "gt" => Ok(Comparison::GreaterThan),
            "gte" => Ok(Comparison::GreaterThanOrEqual),
            _ => bail!("Could not parse a comparison operator"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LimitQuery {
    pub offset: Option<u64>,
    pub n: u64,
}

impl FromStr for LimitQuery {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &s.split(",").collect::<Vec<_>>()[..] {
            [offset, n] => {
                Ok(LimitQuery {
                    offset: Some(offset.parse::<u64>()?),
                    n: n.parse::<u64>()?,
                })
            },
            [n] => {
                Ok(LimitQuery {
                    offset: None,
                    n: n.parse::<u64>()?,
                })
            },
            _ => bail!("Could not parse a limit query"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SortQuery {
    pub direction: SortDirection,
    pub measure: MeaOrCalc,
}

impl FromStr for SortQuery {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &s.split(".").collect::<Vec<_>>()[..] {
            [measure, direction] => {
                let measure = measure.parse::<MeaOrCalc>()?;
                let direction = direction.parse::<SortDirection>()?;
                Ok(SortQuery {
                    direction,
                    measure,
                })
            },
            _ => bail!("Could not parse a sort query"),
        }

    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    pub fn sql_string(&self) -> String {
        match *self {
            SortDirection::Asc => "asc".to_owned(),
            SortDirection::Desc => "desc".to_owned(),
        }
    }
}

impl FromStr for SortDirection {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "asc" => SortDirection::Asc,
            "desc" => SortDirection::Desc,
            _ => bail!("Could not parse sort direction"),
        })
    }
}

#[derive(Debug, Clone)]
pub struct RcaQuery {
    pub drill_1: Drilldown,
    pub drill_2: Drilldown,
    pub mea: Measure,
}

impl RcaQuery {
    pub fn new<S: Into<String>>(
        dim_1: S, hier_1: S, level_1: S,
        dim_2: S, hier_2: S, level_2: S,
        measure: S
    ) -> Self {
        let drill_1 = Drilldown::new(dim_1, hier_1, level_1);
        let drill_2 = Drilldown::new(dim_2, hier_2, level_2);
        let mea = Measure::new(measure);

        RcaQuery {
            drill_1,
            drill_2,
            mea,
        }
    }
}

impl FromStr for RcaQuery {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &s.split(",").collect::<Vec<_>>()[..] {
            [drill_1, drill_2, measure] => {
                let drill_1 = drill_1.parse::<Drilldown>()?;
                let drill_2 = drill_2.parse::<Drilldown>()?;
                let mea = measure.parse::<Measure>()?;

                Ok(RcaQuery {
                    drill_1,
                    drill_2,
                    mea,
                })
            },
            _ => bail!("Could not parse an rca query, wrong number of args"),
        }

    }
}

#[derive(Debug, Clone)]
pub struct GrowthQuery {
    pub time_drill: Drilldown,
    pub mea: Measure,
}

impl GrowthQuery {
    pub fn new<S: Into<String>>(dimension: S, hierarchy: S, level: S, measure: S) -> Self {
        let time_drill = Drilldown::new(dimension, hierarchy, level);
        let mea = Measure::new(measure);

        GrowthQuery {
            time_drill,
            mea,
        }
    }
}

impl FromStr for GrowthQuery {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &s.split(",").collect::<Vec<_>>()[..] {
            [time_drill, measure] => {
                let time_drill = time_drill.parse::<Drilldown>()?;
                let mea = measure.parse::<Measure>()?;

                Ok(GrowthQuery {
                    time_drill,
                    mea,
                })
            },
            _ => bail!("Could not parse a growth query, wrong number of args"),
        }

    }
}

/// For using an operator such as AND and OR in a sql query
/// Currently used for the Filter and inner queries only
#[derive(Debug, Clone, PartialEq)]
pub enum Operator{
    And,
    Or,
}

impl Operator {
    pub fn sql_string(&self) -> String {
        match self {
            Operator::And => "and".to_owned(),
            Operator::Or => "or".to_owned(),
        }
    }
}

impl FromStr for Operator {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "and" => Operator::And,
            "or" => Operator::Or,
            _ => bail!("Could not parse sort direction"),
        })
    }
}

fn get_filter(filter_split: Vec<String>, split_int: usize) -> Result<FilterQuery, Error> {
    let by_mea_or_calc = filter_split[0].parse::<MeaOrCalc>()?;
    let constraint = join(&filter_split[1..split_int], ".").parse::<Constraint>()?;
    let operator = Some(filter_split[split_int].parse::<Operator>()?);
    let constraint2 = Some(join(&filter_split[split_int+1..], ".").parse::<Constraint>()?);
    Ok(FilterQuery {
        by_mea_or_calc,
        constraint,
        operator,
        constraint2,
    })
}

/// For filtering on a measure after Top is calculated (wrapper around end aggregation)
#[derive(Debug, Clone)]
pub struct FilterQuery {
    pub by_mea_or_calc: MeaOrCalc,
    pub constraint: Constraint,
    pub operator: Option<Operator>,
    pub constraint2: Option<Constraint>
}

impl PartialEq for FilterQuery {
    fn eq(&self, other: &Self) -> bool {
        self.constraint.n == other.constraint.n &&
        self.constraint.comparison == other.constraint.comparison &&
        self.by_mea_or_calc == other.by_mea_or_calc
    }
}

impl Eq for FilterQuery {}


// Currently only allows one sort_measure
impl FromStr for FilterQuery {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains(".and.") || s.contains(".or.") {
            let filter_split: Vec<String> = s.split(".").map(|f| f.to_string()).collect();
            let length = filter_split.len();
            if length >= 6 || length <= 8 {
                let op_index = filter_split.iter().position(|s| s == "and" || s=="or").unwrap();
                get_filter(filter_split, op_index)
            } else {
                bail!("Could not parse a filter query")
            }
        } else {
            match &s.splitn(2, ".").collect::<Vec<_>>()[..] {
                [by_mea, constraint] => {
                    let by_mea_or_calc = by_mea.parse::<MeaOrCalc>()?;
                    let constraint = constraint.parse::<Constraint>()?;

                    Ok(FilterQuery {
                        by_mea_or_calc,
                        constraint,
                        operator: None,
                        constraint2: None
                    })
                },
                _ => bail!("Could not parse a filter query"),
            }
        }
    }
}


#[derive(Debug, Clone)]
pub struct RateQuery {
    pub level_name: LevelName,
    pub values: Vec<String>,
}

impl RateQuery {
    pub fn new(level_name: LevelName, values: Vec<String>) -> Self {
        RateQuery {
            level_name,
            values,
        }
    }
}

impl FromStr for RateQuery {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let rate_split: Vec<String> = s.split(".").map(|x| x.to_string()).collect();
        let n = rate_split.len();

        if n <= 2 || n >= 5 {
            return Err(format_err!("Malformatted RateQuery"));
        }

        let level = join(rate_split[0..n-1].iter(), ".");
        let level_name = level.parse::<LevelName>()?;
        let values: Vec<String> = rate_split[n-1].split(",").map(|s| s.to_string()).collect();

        Ok(RateQuery{
            level_name,
            values
        })
    }
}


#[cfg(test)]
mod tests {
    use super::FilterQuery;
    use super::Measure;
    use crate::query::MeaOrCalc;
    use crate::query::{Constraint, Comparison};
    use std::str::FromStr;

    #[test]
    fn test_simple_filter() {
        let m = Measure("Hello".to_owned());
        let filter = FilterQuery::from_str("Hello.eq.45.2").unwrap();

        let target = FilterQuery {
            by_mea_or_calc: MeaOrCalc::Mea(m),
            constraint: Constraint {
                comparison: Comparison::Equal,
                n: 45.2,
            },
            operator: None,
            constraint2: None,
        };
        assert_eq!(filter, target);
    }

    #[test]
    fn test_simple_gt_filter() {
        let m = Measure("Hello".to_owned());
        let filter = FilterQuery::from_str("Hello.gt..2").unwrap();

        let target = FilterQuery {
            by_mea_or_calc: MeaOrCalc::Mea(m),
            constraint: Constraint {
                comparison: Comparison::GreaterThan,
                n: 0.2,
            },
            operator: None,
            constraint2: None,
        };
        assert_eq!(filter, target);
    }
}
