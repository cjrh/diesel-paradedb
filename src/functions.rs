//! Scalar function bindings.

use diesel::expression::functions::define_sql_function;
use diesel::sql_types::{Array, Integer, SingleValue, SqlType, Text};

use crate::types::{ParadeQuery, SearchQueryInput};

define_sql_function! {
    /// `pdb.all()` — match-all ParadeDB query. Pair with `@@@` to filter
    /// every row through the BM25 index (e.g. so a window-form
    /// `pdb.agg(...)` has a Top-K context to compute over).
    #[sql_name = "pdb.all"]
    fn pdb_all() -> ParadeQuery;
}

define_sql_function! {
    /// `pdb.score(key)` — BM25 relevance score for the row. Only
    /// meaningful inside a query whose `@@@` / `|||` / `###` filter targets
    /// the same BM25 index; outside that context Postgres returns NULL at
    /// runtime. Modelled as `Float4` (non-nullable in the typical hot
    /// path).
    ///
    /// Accepts any column expression — the Postgres function signature
    /// is `anyelement`, and in practice it's always the primary-key
    /// column declared as `key_field` in the BM25 index.
    #[sql_name = "pdb.score"]
    fn pdb_score<T: SqlType + SingleValue>(key: T) -> Float4;
}

define_sql_function! {
    /// `pdb.parse(query_string)` — parse a Lucene-style query string into
    /// a structured ParadeDB query. Use it with [`ParadeMatchDsl`][crate::ParadeMatchDsl],
    /// for example `id.parade_match(pdb_parse("description:shoes AND category:outdoors"))`.
    #[sql_name = "pdb.parse"]
    fn pdb_parse(query_string: Text) -> ParadeQuery;
}

define_sql_function! {
    /// `pdb.snippet(field)` — highlighted snippet for the active BM25 query.
    /// Use it in the same `SELECT` that filters with ParadeDB, typically next
    /// to [`pdb_score`][fn@pdb_score].
    #[sql_name = "pdb.snippet"]
    fn pdb_snippet<T: SqlType + SingleValue>(field: T) -> Text;
}

define_sql_function! {
    /// `pdb.snippet(field, start_tag, end_tag)` — highlighted snippet with
    /// custom opening and closing tags.
    #[sql_name = "pdb.snippet"]
    fn pdb_snippet_with_tags<T: SqlType + SingleValue>(field: T, start_tag: Text, end_tag: Text) -> Text;
}

define_sql_function! {
    /// `pdb.more_like_this(key_value)` — build a query that finds rows similar
    /// to the row identified by an integer BM25 index key. ParadeDB currently
    /// returns the legacy `paradedb.searchqueryinput` type for this builder;
    /// the `@@@` operator accepts it, so it composes with
    /// [`ParadeMatchDsl`][crate::ParadeMatchDsl].
    #[sql_name = "pdb.more_like_this"]
    fn pdb_more_like_this(key_value: Integer) -> SearchQueryInput;
}

define_sql_function! {
    /// `pdb.more_like_this(key_value, fields)` — `more_like_this` restricted
    /// to selected indexed text fields.
    #[sql_name = "pdb.more_like_this"]
    fn pdb_more_like_this_in_fields(key_value: Integer, fields: Array<Text>) -> SearchQueryInput;
}
