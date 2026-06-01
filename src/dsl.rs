//! Operators (`|||`, `@@@`, `###`) and the extension traits that expose
//! them as fluent method calls.

use diesel::expression::{AsExpression, Expression};
use diesel::pg::Pg;
use diesel::sql_types::{SqlType, Text};

use crate::types::{ParadeQuery, SearchQueryInput};

// `||| (anyelement, text) -> bool` — BM25 term-query operator.
//
// Example: `WHERE text ||| 'service'`. The macro generates the AST node
// plus all the trait wiring; nullability of the result follows nullability
// of the arguments (Diesel's `NullableBasedOnArgs` default).
diesel::infix_operator!(TermMatch, " ||| ", backend: Pg);

// `@@@ (anyelement, pdb.query | paradedb.searchqueryinput) -> bool` —
// structured-query match operator.
//
// Example: `WHERE id @@@ pdb.all()`.
diesel::infix_operator!(ParadeMatch, " @@@ ", backend: Pg);

// `### (anyelement, text) -> bool` — phrase-query operator.
//
// Example: `WHERE body ### 'running shoes'`.
diesel::infix_operator!(PhraseMatch, " ### ", backend: Pg);

/// Fluent `.term_match(...)` for BM25 term queries against a `Text` column.
///
/// `text ||| 'service'` becomes `my_table::text.term_match("service")`.
pub trait TermMatchDsl: Sized {
    fn term_match<U>(self, query: U) -> TermMatch<Self, U::Expression>
    where
        U: AsExpression<Text>;
}

impl<T> TermMatchDsl for T
where
    T: Expression<SqlType = Text>,
{
    fn term_match<U>(self, query: U) -> TermMatch<Self, U::Expression>
    where
        U: AsExpression<Text>,
    {
        TermMatch::new(self, query.as_expression())
    }
}

/// SQL types accepted by ParadeDB's structured `@@@` operator.
///
/// Exposed only because it appears in [`ParadeMatchDsl`]'s public bounds; new
/// implementations belong in this crate when ParadeDB adds another query type.
#[doc(hidden)]
pub trait ParadeMatchRhs: SqlType {}

impl ParadeMatchRhs for ParadeQuery {}
impl ParadeMatchRhs for SearchQueryInput {}

/// Fluent `.parade_match(...)` for structured ParadeDB queries against any
/// column.
///
/// `id @@@ pdb.all()` becomes `my_table::id.parade_match(pdb_all())`. The
/// right-hand side must be a ParadeDB query expression such as `pdb_all()`,
/// `pdb_parse(...)`, or `pdb_more_like_this(...)`.
pub trait ParadeMatchDsl: Sized {
    fn parade_match<U>(self, query: U) -> ParadeMatch<Self, U>
    where
        U: Expression,
        U::SqlType: ParadeMatchRhs;
}

impl<T> ParadeMatchDsl for T
where
    T: Expression,
{
    fn parade_match<U>(self, query: U) -> ParadeMatch<Self, U>
    where
        U: Expression,
        U::SqlType: ParadeMatchRhs,
    {
        ParadeMatch::new(self, query)
    }
}

/// Fluent `.phrase_match(...)` for BM25 phrase queries against a `Text` column.
///
/// `body ### 'running shoes'` becomes `my_table::body.phrase_match("running shoes")`.
pub trait PhraseMatchDsl: Sized {
    fn phrase_match<U>(self, query: U) -> PhraseMatch<Self, U::Expression>
    where
        U: AsExpression<Text>;
}

impl<T> PhraseMatchDsl for T
where
    T: Expression<SqlType = Text>,
{
    fn phrase_match<U>(self, query: U) -> PhraseMatch<Self, U::Expression>
    where
        U: AsExpression<Text>,
    {
        PhraseMatch::new(self, query.as_expression())
    }
}
