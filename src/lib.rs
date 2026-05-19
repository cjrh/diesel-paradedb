//! Diesel ORM bindings for [ParadeDB](https://www.paradedb.com)'s `pg_search`
//! extension.
//!
//! ParadeDB adds a Postgres type (`pdb.query`), two custom operators (`|||`
//! and `@@@`), and a family of query-builder functions in the `pdb` schema.
//! Mainline Diesel does not know about any of these, so queries that use
//! them must otherwise be written with `sql_query`. This crate bridges that
//! gap: once imported, BM25 queries, match-all filters, relevance scoring,
//! and faceting aggregates compose with regular Diesel `select` / `filter`
//! / `order` calls and are checked at compile time.
//!
//! ## What is modelled
//!
//! - [`sql_types::ParadeQuery`] — Diesel SQL-type marker that mirrors
//!   ParadeDB's `pdb.query` type. Every `pdb::*` builder function returns
//!   this type, and the `@@@` operator expects it on the right.
//! - Infix operators [`TermMatch`] (`|||`) and [`ParadeMatch`] (`@@@`),
//!   exposed via the fluent extension traits [`TermMatchDsl`] /
//!   [`ParadeMatchDsl`] (`.term_match(...)` and `.parade_match(...)`).
//! - Scalar function bindings: [`pdb_all`][fn@pdb_all],
//!   [`pdb_score`][fn@pdb_score].
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
//! - The legacy `paradedb.searchqueryinput` type. ParadeDB exposes both
//!   `pdb.query` (newer) and `paradedb.searchqueryinput` (legacy). This
//!   crate models only the newer `pdb.query`.
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
//! This crate pins to a single Diesel minor. `diesel-paradedb` `0.1.x`
//! targets `diesel = "~2.2"`. A Diesel `2.3` release will trigger
//! `diesel-paradedb` `0.2`; a Diesel `3.0` will trigger
//! `diesel-paradedb` `3.0` (the crate's major tracks Diesel's).
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
#![doc(html_root_url = "https://docs.rs/diesel-paradedb/0.1.0")]

mod aggregates;
mod dsl;
mod functions;
mod types;

pub use aggregates::{
    pdb_agg_avg, pdb_agg_stats, pdb_agg_terms, pdb_agg_value_count, PdbAgg, PdbAggOver,
};
pub use dsl::{ParadeMatch, ParadeMatchDsl, TermMatch, TermMatchDsl};
pub use functions::{pdb_all, pdb_score};

/// SQL-type markers exposed for use in `Queryable` derives and explicit
/// type annotations on `select(...)` clauses. The convention mirrors
/// pgvector's `pgvector::sql_types::Vector` re-export.
pub mod sql_types {
    pub use crate::types::ParadeQuery;
}
