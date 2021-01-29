use itertools::join;

use super::{
    TableSql,
    CutSql,
    DrilldownSql,
    MeasureSql,
    RateSql,
};

use crate::sql::primary_agg::primary_agg;

use tesseract_core::{Aggregator};


pub fn rate_calculation(
    table: &TableSql,
    cuts: &[CutSql],
    drills: &[DrilldownSql],
    meas: &[MeasureSql],
    rate: &RateSql
) -> (String, String)
{
    // Add a drilldown on the level we are getting the rate for
    let mut new_drills: Vec<DrilldownSql> = vec![];
    let found_rate_drill = false;

    for drill in drills {
        if drill == &rate.drilldown_sql {
            continue;
        }
        new_drills.push(drill.clone());
    }

    if !found_rate_drill {
        new_drills.push(rate.drilldown_sql.clone());
    }

    // Call primary agg
    let (final_sql, _final_drill_cols) = {
        primary_agg(table, cuts, &new_drills, meas, None)
    };

    let mut rate_sql;

    // Wrap that around a pivot
    let original_drill_cols = drills.iter().map(|drill| drill.col_alias_only_string());
    let original_drill_cols = join(original_drill_cols, ", ");

    let rate_aggregator = match meas[0].aggregator {
        Aggregator::Count => "count".to_string(),
        _ => "sum".to_string()
    };

    rate_sql = format!("select {}, {}(final_m0) as final_m0_agg, groupArray(final_m0) as final_m0_rate",
        original_drill_cols, rate_aggregator
    );

    let rate_drill_cols = rate.drilldown_sql.col_alias_only_vec();
    for rate_drill_col in &rate_drill_cols {
        rate_sql = format!("{}, groupArray({}) as {}_group", rate_sql, rate_drill_col, rate_drill_col);
    }

    rate_sql = format!("{} from ({}) group by {}", rate_sql, final_sql, original_drill_cols);

    // Unpivot
    let mut rate_sql_unpivot = format!("select {}, ", original_drill_cols);

    for rate_drill_col in &rate_drill_cols {
        rate_sql_unpivot = format!("{}{}_group, ", rate_sql_unpivot, rate_drill_col);
    }

    rate_sql = format!("{}final_m0_agg as final_m0, final_m0_rate from ({}) array join",
        rate_sql_unpivot, rate_sql
    );

    for rate_drill_col in &rate_drill_cols {
        rate_sql = format!("{} {}_group as {}_group,", rate_sql, rate_drill_col, rate_drill_col);
    }

    rate_sql = format!("{} final_m0_rate as final_m0_rate", rate_sql);

    // Final aggregation
    rate_sql = format!("select {}, final_m0, {}(final_m0_rate) / avg(final_m0) from ({}) where {}_group in ({}) group by {}, final_m0",
        original_drill_cols,
        rate_aggregator,
        rate_sql,
        rate_drill_cols[0],
        join(rate.members.clone(), ", "),
        original_drill_cols
    );

    (rate_sql, original_drill_cols)
}
