//! Scalar function bindings.

use diesel::expression::functions::define_sql_function;
use diesel::sql_types::{SingleValue, SqlType};

use crate::types::ParadeQuery;

define_sql_function! {
    /// `pdb.all()` — match-all ParadeDB query. Pair with `@@@` to filter
    /// every row through the BM25 index (e.g. so a window-form
    /// `pdb.agg(...)` has a Top-K context to compute over).
    #[sql_name = "pdb.all"]
    fn pdb_all() -> ParadeQuery;
}

define_sql_function! {
    /// `pdb.score(key)` — BM25 relevance score for the row. Only
    /// meaningful inside a query whose `@@@` / `|||` filter targets the
    /// same BM25 index; outside that context Postgres returns NULL at
    /// runtime. Modelled as `Float4` (non-nullable in the typical hot
    /// path).
    ///
    /// Accepts any column expression — the Postgres function signature
    /// is `anyelement`, and in practice it's always the primary-key
    /// column declared as `key_field` in the BM25 index.
    #[sql_name = "pdb.score"]
    fn pdb_score<T: SqlType + SingleValue>(key: T) -> Float4;
}
