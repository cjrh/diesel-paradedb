//! Diesel ORM bindings for [ParadeDB](https://www.paradedb.com)'s `pg_search`
//! extension.
//!
//! ParadeDB adds Postgres query types (`pdb.query` and, for a few legacy
//! builders, `paradedb.searchqueryinput`), custom search operators (`|||`,
//! `@@@`, and `###`), and a family of query-builder functions in the `pdb`
//! schema.
//! Mainline Diesel does not know about any of these, so queries that use
//! them must otherwise be written with `sql_query`. This crate bridges that
//! gap: once imported, BM25 queries, match-all filters, relevance scoring,
//! and faceting aggregates compose with regular Diesel `select` / `filter`
//! / `order` calls and are checked at compile time.
//!
//! ## What is modelled
//!
//! - [`sql_types::ParadeQuery`] — Diesel SQL-type marker that mirrors
//!   ParadeDB's `pdb.query` type. Most modern `pdb::*` builders return this
//!   type, and the `@@@` operator accepts it on the right.
//! - [`sql_types::SearchQueryInput`] — marker for legacy builders that still
//!   return `paradedb.searchqueryinput`, such as `pdb.more_like_this(...)`.
//! - Infix operators [`TermMatch`] (`|||`), [`ParadeMatch`] (`@@@`), and
//!   [`PhraseMatch`] (`###`), exposed via fluent extension traits
//!   [`TermMatchDsl`] / [`ParadeMatchDsl`] / [`PhraseMatchDsl`]
//!   (`.term_match(...)`, `.parade_match(...)`, and `.phrase_match(...)`).
//! - Scalar function bindings: [`pdb_all`][fn@pdb_all],
//!   [`pdb_parse`][fn@pdb_parse], [`pdb_score`][fn@pdb_score],
//!   [`pdb_snippet`][fn@pdb_snippet], and
//!   [`pdb_more_like_this`][fn@pdb_more_like_this].
//! - Aggregate / window expressions: [`pdb_agg_terms`], [`pdb_agg_avg`],
//!   [`pdb_agg_stats`], [`pdb_agg_value_count`]. Each returns a [`PdbAgg`]
//!   that can be used directly (aggregate form) or via `.over_all()`
//!   (window form, which attaches a facet to every row of a non-grouped
//!   Top-K BM25 query — the shape ParadeDB documents for
//!   search-with-facets).
//!
//! ## What is NOT modelled
//!
//! - The BM25 index access method itself. Indexes do not live in Diesel's
//!   type system — `CREATE INDEX ... USING bm25 (...)` belongs in your
//!   migrations. This crate type-checks queries that *use* a BM25 index;
//!   existence of the index is a runtime contract.
//! - Most of the legacy `paradedb.searchqueryinput` surface. ParadeDB exposes
//!   both `pdb.query` (newer) and `paradedb.searchqueryinput` (legacy). This
//!   crate models the legacy marker only where current ParadeDB still needs it,
//!   such as `pdb.more_like_this(...)`.
//!
//! ## Quick start
//!
//! ```ignore
//! use diesel::prelude::*;
//! use diesel_paradedb::{pdb_all, pdb_score, ParadeMatchDsl, TermMatchDsl};
//!
//! // Top-5 BM25 hits with score, ordered by relevance.
//! let rows: Vec<(i32, String, f32)> = my_table::table
//!     .select((my_table::id, my_table::text, pdb_score(my_table::id)))
//!     .filter(my_table::text.term_match("service outage"))
//!     .order(pdb_score(my_table::id).desc())
//!     .limit(5)
//!     .load(conn)?;
//!
//! // Match-all + sentiment facet via the window form.
//! use diesel_paradedb::pdb_agg_terms;
//! let (id, facet): (i32, serde_json::Value) = my_table::table
//!     .select((my_table::id, pdb_agg_terms("sentiment", 10).over_all()))
//!     .filter(my_table::id.parade_match(pdb_all()))
//!     .order(pdb_score(my_table::id).desc())
//!     .limit(1)
//!     .get_result(conn)?;
//! ```
//!
//! ## Diesel version compatibility
//!
//! This crate supports Diesel `2.2` and `2.3` (`>=2.2, <2.4`). Each new Diesel
//! minor is validated against this crate's test suite before the upper bound is
//! widened.
//!
//! ## Async usage
//!
//! Works unchanged with [`diesel-async`]: this crate only adds new
//! expression types and operators, all of which use Diesel's standard
//! `QueryFragment` / `Expression` traits. No async feature flag is
//! required.
//!
//! [`diesel-async`]: https://crates.io/crates/diesel-async

#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_root_url = "https://docs.rs/diesel-paradedb/1.0.3")]

mod aggregates;
mod dsl;
mod functions;
mod types;

pub use aggregates::{
    pdb_agg_avg, pdb_agg_stats, pdb_agg_terms, pdb_agg_value_count, PdbAgg, PdbAggOver,
};
pub use dsl::{ParadeMatch, ParadeMatchDsl, PhraseMatch, PhraseMatchDsl, TermMatch, TermMatchDsl};
pub use functions::{
    pdb_all, pdb_more_like_this, pdb_more_like_this_in_fields, pdb_parse, pdb_score, pdb_snippet,
    pdb_snippet_with_tags,
};

/// SQL-type markers exposed for use in `Queryable` derives and explicit
/// type annotations on `select(...)` clauses. The convention mirrors
/// pgvector's `pgvector::sql_types::Vector` re-export.
pub mod sql_types {
    pub use crate::types::{ParadeQuery, SearchQueryInput};
}
