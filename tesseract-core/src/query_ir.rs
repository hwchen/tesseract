use itertools::join;
use serde_derive::{Deserialize, Serialize};

use crate::names::Mask;
use crate::query::{LimitQuery, SortDirection, Constraint, Operator};
use crate::schema::{Table, InlineTable};
use crate::schema::aggregator::Aggregator;


#[derive(Debug)]
pub struct QueryIr {
    pub table: TableSql,
    pub cuts: Vec<CutSql>,
    pub drills: Vec<DrilldownSql>,
    pub meas: Vec<MeasureSql>,
    pub hidden_drills: Vec<HiddenDrilldownSql>,
    pub filters: Vec<FilterSql>,
    // TODO put Filters and Calculations into own structs
    pub top: Option<TopSql>,
    pub top_where: Option<TopWhereSql>,
    pub sort: Option<SortSql>,
    pub limit: Option<LimitSql>,
    pub rca: Option<RcaSql>,
    pub growth: Option<GrowthSql>,
    pub rate: Option<RateSql>,
    pub sparse: bool,
}

#[derive(Debug, Clone)]
pub struct TableSql {
    pub name: String,
    pub primary_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrilldownSql {
    pub alias_postfix: String,
    pub table: Table,
    pub primary_key: String,
    pub foreign_key: String,
    pub level_columns: Vec<LevelColumn>,
    pub property_columns: Vec<String>,
    pub inline_table: Option<InlineTable>,
}

impl DrilldownSql {
    pub fn col_alias_string(&self) -> String {
        let cols = self.col_alias_vec();
        join(cols, ", ")
    }

    // TODO for now it's ok to map 2 into one slot,
    // because it's only used in col_alias_string.
    // In big refactor, push onto col vec, like
    // in col_alias_only_vec to match behavior
    fn col_alias_vec(&self) -> Vec<String> {
        let mut cols: Vec<_> = self.level_columns.iter()
            .map(|l| {
                if let Some(ref name_col) = l.name_column {
                    format!("{} as {}_{}, {} as {}_{}",
                        l.key_column,
                        l.key_column,
                        self.alias_postfix,
                        name_col,
                        name_col,
                        self.alias_postfix,
                    )
                } else {
                    format!("{} as {}_{}",
                        l.key_column,
                        l.key_column,
                        self.alias_postfix,
                    )
                }
            }).collect();

        if self.property_columns.len() != 0 {
            cols.push(
                join(&self.property_columns, ", ")
            );
        }

        cols
    }

    pub fn col_alias_string2(&self) -> String {
        let cols = self.col_alias_vec2();
        join(cols, ", ")
    }

    fn col_alias_vec2(&self) -> Vec<String> {
        let mut cols: Vec<_> = self.level_columns.iter()
            .map(|l| {
                if let Some(ref name_col) = l.name_column {
                    format!("{}.{} as {}_{}, {} as {}_{}",
                        self.table.name,
                        l.key_column,
                        l.key_column,
                        self.alias_postfix,
                        name_col,
                        name_col,
                        self.alias_postfix,
                    )
                } else {
                    format!("{}.{} as {}_{}",
                        self.table.name,
                        l.key_column,
                        l.key_column,
                        self.alias_postfix,
                    )
                }
            }).collect();

        if self.property_columns.len() != 0 {
            cols.push(
                join(&self.property_columns, ", ")
            );
        }

        cols
    }

    pub fn col_alias_only_string(&self) -> String {
        let cols = self.col_alias_only_vec();
        join(cols, ", ")
    }

    pub fn col_alias_only_vec(&self) -> Vec<String> {
        let mut cols = vec![];

        // can't just map the cols, because some levels
        // produce two and some produce one
        // This matters because growth needs the vec
        // version to map onto single cols.
        for l in self.level_columns.iter() {
            cols.push(format!("{}_{}",
                l.key_column,
                self.alias_postfix,
            ));

            if let Some(ref name_col) = l.name_column {
                cols.push(format!("{}_{}",
                    name_col,
                    self.alias_postfix,
                ));
            }
        }

        if self.property_columns.len() != 0 {
            cols.push(
                join(&self.property_columns, ", ")
            );
        }

        cols
    }

    pub fn col_qual_string(&self) -> String {
        let cols = self.col_qual_vec();
        join(cols, ", ")
    }

    fn col_qual_vec(&self) -> Vec<String> {
        let mut cols: Vec<_> = self.level_columns.iter()
            .map(|l| {
                if let Some(ref name_col) = l.name_column {
                    format!("{}.{}, {}.{}", self.table.name, l.key_column, self.table.name, name_col)
                } else {
                    format!("{}.{}", self.table.name, l.key_column)
                }
            }).collect();

        if self.property_columns.len() != 0 {
            let prop_cols_qual = self.property_columns.iter()
                .map(|p| {
                    format!("{}.{}", self.table.name, p)
                });

            cols.push(
                join(prop_cols_qual, ", ")
            );
        }

        cols
    }
}

#[derive(Debug, Clone)]
pub struct HiddenDrilldownSql {
    pub drilldown_sql: DrilldownSql,
}

// TODO make level column an enum, to deal better with
// levels with only key column and no name column?
#[derive(Debug, Clone, PartialEq)]
pub struct LevelColumn {
    pub key_column: String,
    pub name_column: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CutSql {
    pub table: Table,
    pub primary_key: String,
    pub foreign_key: String,
    pub column: String,
    pub members: Vec<String>,
    pub member_type: MemberType,
    // Mask is Includes or Excludes on set of cut members
    pub mask: Mask,
    // if for_match, then use LIKE syntax
    pub for_match: bool,
    pub inline_table: Option<InlineTable>,
}

impl CutSql {
    pub fn members_string(&self) -> String {
        let members = match self.member_type {
            MemberType::NonText => join(&self.members, ", "),
            MemberType::Text => {
                let quoted = self.members.iter()
                .map(|m| format!("'{}'", m));
                join(quoted, ", ")
            }
        };

        format!("{}", members)
    }

    pub fn members_like_string(&self) -> String {
        match self.member_type {
            MemberType::NonText => {
                // this behavior doesn't really make sense; it should be for
                // labels only, which are almost always strings.
                let unquoted = self.members.iter()
                    .map(|m| format!("{} {} {}", self.column, self.mask_sql_like_string(), m));

                match self.mask {
                    Mask::Include => format!("({})", join(unquoted, " or ")),
                    Mask::Exclude => format!("{}", join(unquoted, " and ")),
                }
            },
            MemberType::Text => {
                let quoted = self.members.iter()
                    .map(|m| format!("{} {} '%{}%'", self.column, self.mask_sql_like_string(), m));

                match self.mask {
                    Mask::Include => format!("({})", join(quoted, " or ")),
                    Mask::Exclude => format!("{}", join(quoted, " and ")),
                }
            }
        }
    }

    pub fn col_qual_string(&self) -> String {
        format!("{}.{}", self.table.name, self.column)
    }

    pub fn mask_sql_in_string(&self) -> String {
        match self.mask {
            Mask::Include => "in".into(),
            Mask::Exclude => "not in".into(),
        }
    }

    pub fn mask_sql_like_string(&self) -> String {
        match self.mask {
            Mask::Include => "like".into(),
            Mask::Exclude => "not like".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum MemberType {
    #[serde(rename="text")]
    Text,
    #[serde(rename="nontext")]
    NonText,
}

#[derive(Debug, Clone)]
pub struct MeasureSql {
    pub aggregator: Aggregator,
    pub column: String,
}

// NOTE: This is now specific to each db, because of the custom aggregators
// e.g. median
//impl MeasureSql {
//    pub fn agg_col_string(&self) -> String {
//        format!("{}({})", self.aggregator, self.column)
//    }
//}

#[derive(Debug, Clone)]
pub struct TopSql {
    pub n: u64,
    pub by_column: String,
    pub sort_columns: Vec<String>,
    pub sort_direction: SortDirection,
}

#[derive(Debug, Clone)]
pub struct TopWhereSql {
    pub by_column: String,
    pub constraint: Constraint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilterSql {
    pub by_column: String,
    pub constraint: Constraint,
    pub operator: Option<Operator>,
    pub constraint2: Option<Constraint>

}


#[derive(Debug, Clone)]
pub struct LimitSql {
    pub offset: Option<u64>,
    pub n: u64,
}

impl From<LimitQuery> for LimitSql {
    fn from(l: LimitQuery) -> Self {
        LimitSql {
            offset: l.offset,
            n: l.n,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SortSql {
    pub direction: SortDirection,
    pub column: String,
}

#[derive(Debug, Clone)]
pub struct RcaSql {
    // level col for dim 1
    pub drill_1: Vec<DrilldownSql>,
    // level col for dim 2
    pub drill_2: Vec<DrilldownSql>,
    pub mea: MeasureSql,
    pub debug: bool,
}

#[derive(Debug, Clone)]
pub struct GrowthSql {
    pub time_drill: DrilldownSql,
    pub mea: String,
}

#[derive(Debug, Clone)]
pub struct RateSql {
    pub drilldown_sql: DrilldownSql,
    pub members: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DimSubquery {
    pub sql: String,
    pub foreign_key: String,
    pub dim_cols: Option<String>,
}


// TODO can this be removed, and all cuts put into the fact table scan using `IN`?
/// Collects a drilldown and cut together to create a subquery for the dimension table
/// Does not check for matching name, because that had to have been done
/// before submitting to this fn.
pub fn dim_subquery(drill: Option<&DrilldownSql>, cut: Option<&CutSql>) -> DimSubquery {
    match drill {
        Some(drill) => {
            let drill_table = match &drill.inline_table {
                Some(it) => {
                    let inline_table_sql = it.sql_string();
                    format!("({}) as {}", inline_table_sql, it.alias)
                },
                None => drill.table.full_name()
            };

            // TODO
            // - oops, primary key is mandatory in schema, if not in
            // schema-config, then it takes the lowest level's key_column
            // - make primary key optional and propagate.
            // if primary key exists
            // if primary key == lowest level col,
            // Or will just making an alias for the primary key work?
            // Then don't add primary key here.
            // Also, make primary key optional?
            let sql = format!("select {}, {} as {} from {}",
                drill.col_alias_string(),
                drill.primary_key.clone(),
                drill.foreign_key.clone(),
                drill_table,
            );
            // TODO can I delete this cut?
//            if let Some(cut) = cut {
//                sql.push_str(&format!(" where {} in ({})",
//                    cut.column.clone(),
//                    cut.members_string(),
//                )[..]);
//            }
            return DimSubquery {
                sql,
                foreign_key: drill.foreign_key.clone(),
                dim_cols: Some(drill.col_alias_only_string()),
            };
        },
        // TODO remove this? This path should never be hit now.
        None => {
            if let Some(cut) = cut {
                let sql = format!("select {} as {} from {} where {} in ({})",
                    cut.primary_key.clone(),
                    cut.foreign_key.clone(),
                    cut.table.full_name(),
                    cut.column.clone(),
                    cut.members_string(),
                );

                return DimSubquery {
                    sql,
                    foreign_key: cut.foreign_key.clone(),
                    dim_cols: None,
                }
            }
        }
    }

    DimSubquery {
        sql: "".to_owned(),
        foreign_key: "".to_owned(),
        dim_cols: None,
    }
}
