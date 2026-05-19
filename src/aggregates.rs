//! `pdb.agg(...)` aggregate / window expressions.
//!
//! Two AST nodes: [`PdbAgg`] is the true aggregate (grouping rules
//! apply), [`PdbAggOver`] carries the `OVER ()` clause and behaves as a
//! non-aggregate so it composes with row-level `SELECT` columns. Use the
//! `pdb_agg_*` constructors to build them — they take typed arguments
//! and serialise the JSON config internally, so callers never touch raw
//! JSON.

use diesel::expression::{is_aggregate, Expression, ValidGrouping};
use diesel::pg::Pg;
use diesel::query_builder::{AstPass, QueryFragment, QueryId};
use diesel::result::QueryResult;
use diesel::sql_types::Jsonb;
use diesel::{AppearsOnTable, SelectableExpression};

use serde_json::{json, Value as JsonValue};

/// Inline a `JsonValue` as a `'{...}'::jsonb` Postgres literal.
///
/// `pdb.agg(...)` in window form (`OVER ()`) routes through ParadeDB's
/// custom-scan planner, which inspects the aggregate's config JSON at
/// plan time to wire up the Top-K execution path. A `Jsonb` *bind*
/// parameter hides the value from the planner and produces a runtime
/// error of the form "pdb.agg() must be handled by ParadeDB's custom
/// scan". The aggregate form (no `OVER`) doesn't have this requirement,
/// but emitting the literal in both cases keeps one code path and one
/// prepared-statement shape.
///
/// All callers reach this through `pdb_agg_terms` / `pdb_agg_avg` /
/// etc., which take `field: &str` and a numeric tuning knob. Single
/// quotes are doubled per PostgreSQL's standard literal rules; the JSON
/// serialiser guarantees no other shell-meta characters need escaping.
fn push_jsonb_literal<'b>(out: &mut AstPass<'_, 'b, Pg>, value: &'b str) -> QueryResult<()> {
    out.push_sql("'");
    // Double any single quotes — defence in depth. Field names in our
    // `pdb_agg_*` builders are static identifiers, so in practice this
    // loop never finds a quote to escape.
    let mut remaining = value;
    while let Some(idx) = remaining.find('\'') {
        out.push_sql(&remaining[..idx]);
        out.push_sql("''");
        remaining = &remaining[idx + 1..];
    }
    out.push_sql(remaining);
    out.push_sql("'::jsonb");
    Ok(())
}

/// Aggregate form of `pdb.agg(<json>)`. Use the `pdb_agg_*` constructors
/// rather than building this directly — they keep the JSON config
/// private and consistent across call sites.
///
/// The serialised JSON is cached on construction so the per-render
/// `walk_ast` call is allocation-free and can return a borrow.
#[derive(Debug, Clone)]
pub struct PdbAgg {
    config_sql: String,
}

/// Window form of `pdb.agg(<json>) OVER ()`. Produced by
/// [`PdbAgg::over_all`]. Behaves as a non-aggregate expression so it can
/// coexist with normal `SELECT` columns — which is the entire point of
/// using `OVER ()` in the search-with-facets pattern.
#[derive(Debug, Clone)]
pub struct PdbAggOver {
    config_sql: String,
}

impl PdbAgg {
    fn new(config: JsonValue) -> Self {
        let config_sql = serde_json::to_string(&config)
            .expect("pdb_agg config: JsonValue always serialises");
        Self { config_sql }
    }

    /// Convert to the window-function form, emitting
    /// `pdb.agg(...) OVER ()`. Required for the search-with-facets
    /// pattern, where ParadeDB attaches the aggregate result to every
    /// row of a Top-K query.
    pub fn over_all(self) -> PdbAggOver {
        PdbAggOver {
            config_sql: self.config_sql,
        }
    }

    /// Serialised JSON body emitted into the SQL literal. Exposed for
    /// tests asserting on the shape; not intended for production callers
    /// (use the `pdb_agg_*` constructors instead of poking at internals).
    #[doc(hidden)]
    pub fn config_sql(&self) -> &str {
        &self.config_sql
    }
}

impl PdbAggOver {
    /// See [`PdbAgg::config_sql`].
    #[doc(hidden)]
    pub fn config_sql(&self) -> &str {
        &self.config_sql
    }
}

// QueryId — both forms are stable. The config JSON is part of the
// rendered SQL text (literal), not a runtime bind, but every distinct
// JSON shape produces a different rendering. Marking `HAS_STATIC_QUERY_ID
// = false` lets Diesel skip the prepared-statement cache, which is the
// right behaviour for a planner that inspects the literal at plan time.
impl QueryId for PdbAgg {
    type QueryId = ();
    const HAS_STATIC_QUERY_ID: bool = false;
}
impl QueryId for PdbAggOver {
    type QueryId = ();
    const HAS_STATIC_QUERY_ID: bool = false;
}

impl Expression for PdbAgg {
    type SqlType = Jsonb;
}
impl Expression for PdbAggOver {
    type SqlType = Jsonb;
}

// Aggregate-vs-not is the critical distinction here:
// - `PdbAgg` is a true aggregate; grouping rules apply.
// - `PdbAggOver` carries the `OVER ()` clause, so as far as Diesel's
//   grouping checker is concerned it's a non-aggregate expression that
//   composes freely with row-level selects.
impl<GB> ValidGrouping<GB> for PdbAgg {
    type IsAggregate = is_aggregate::Yes;
}
impl<GB> ValidGrouping<GB> for PdbAggOver {
    type IsAggregate = is_aggregate::No;
}

impl<QS> AppearsOnTable<QS> for PdbAgg where Self: Expression {}
impl<QS> AppearsOnTable<QS> for PdbAggOver where Self: Expression {}
impl<QS> SelectableExpression<QS> for PdbAgg where Self: AppearsOnTable<QS> {}
impl<QS> SelectableExpression<QS> for PdbAggOver where Self: AppearsOnTable<QS> {}

impl QueryFragment<Pg> for PdbAgg {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Pg>) -> QueryResult<()> {
        out.push_sql("pdb.agg(");
        push_jsonb_literal(&mut out, &self.config_sql)?;
        out.push_sql(")");
        Ok(())
    }
}

impl QueryFragment<Pg> for PdbAggOver {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Pg>) -> QueryResult<()> {
        out.push_sql("pdb.agg(");
        push_jsonb_literal(&mut out, &self.config_sql)?;
        out.push_sql(") OVER ()");
        Ok(())
    }
}

// ---- Builders -------------------------------------------------------------
//
// Deep wrappers: callers pass a field name + tuning knob, never raw JSON.
// The exact JSON shapes mirror ParadeDB's documented aggregate config.

/// `pdb.agg('{"terms": {"field": <field>, "size": <size>}}')` —
/// bucket-count aggregate over the distinct values of a fast field.
pub fn pdb_agg_terms(field: &str, size: u32) -> PdbAgg {
    PdbAgg::new(json!({"terms": {"field": field, "size": size}}))
}

/// `pdb.agg('{"avg": {"field": <field>}}')` — average of a numeric fast
/// field.
pub fn pdb_agg_avg(field: &str) -> PdbAgg {
    PdbAgg::new(json!({"avg": {"field": field}}))
}

/// `pdb.agg('{"stats": {"field": <field>}}')` — count / min / max / sum
/// / avg in one pass.
pub fn pdb_agg_stats(field: &str) -> PdbAgg {
    PdbAgg::new(json!({"stats": {"field": field}}))
}

/// `pdb.agg('{"value_count": {"field": <field>}}')` — non-null value
/// count.
pub fn pdb_agg_value_count(field: &str) -> PdbAgg {
    PdbAgg::new(json!({"value_count": {"field": field}}))
}

// ---- Tests ----------------------------------------------------------------

// SQL-rendering of these AST nodes is verified end-to-end in
// `tests/live_paradedb.rs` against a running ParadeDB container —
// that's the only check that catches drift in the operator name,
// function schema, or window-clause spelling. Unit tests here cover
// only the parts that are independent of Postgres: the JSON config
// shapes that the `pdb_agg_*` builders construct.
#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> JsonValue {
        serde_json::from_str(s).expect("config_sql must be valid JSON")
    }

    #[test]
    fn pdb_agg_terms_shape() {
        assert_eq!(
            parse(pdb_agg_terms("sentiment", 10).config_sql()),
            json!({"terms": {"field": "sentiment", "size": 10}})
        );
    }

    #[test]
    fn pdb_agg_avg_shape() {
        assert_eq!(
            parse(pdb_agg_avg("rating").config_sql()),
            json!({"avg": {"field": "rating"}})
        );
    }

    #[test]
    fn pdb_agg_stats_and_value_count_shapes() {
        assert_eq!(
            parse(pdb_agg_stats("rating").config_sql()),
            json!({"stats": {"field": "rating"}})
        );
        assert_eq!(
            parse(pdb_agg_value_count("id").config_sql()),
            json!({"value_count": {"field": "id"}})
        );
    }

    #[test]
    fn over_all_preserves_config() {
        let agg = pdb_agg_terms("source_type", 50);
        let before = agg.config_sql().to_owned();
        assert_eq!(agg.over_all().config_sql(), before);
    }
}
