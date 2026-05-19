//! Operators (`|||`, `@@@`) and the extension traits that expose them
//! as fluent method calls.

use diesel::expression::{AsExpression, Expression};
use diesel::pg::Pg;
use diesel::sql_types::Text;

use crate::types::ParadeQuery;

// `||| (anyelement, text) -> bool` — BM25 term-query operator.
//
// Example: `WHERE text ||| 'service'`. The macro generates the AST node
// plus all the trait wiring; nullability of the result follows nullability
// of the arguments (Diesel's `NullableBasedOnArgs` default).
diesel::infix_operator!(TermMatch, " ||| ", backend: Pg);

// `@@@ (anyelement, pdb.query) -> bool` — structured-query match operator.
//
// Example: `WHERE id @@@ pdb.all()`.
diesel::infix_operator!(ParadeMatch, " @@@ ", backend: Pg);

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

/// Fluent `.parade_match(...)` for structured ParadeDB queries against any
/// column.
///
/// `id @@@ pdb.all()` becomes `my_table::id.parade_match(pdb_all())`.
pub trait ParadeMatchDsl: Sized {
    fn parade_match<U>(self, query: U) -> ParadeMatch<Self, U::Expression>
    where
        U: AsExpression<ParadeQuery>;
}

impl<T> ParadeMatchDsl for T
where
    T: Expression,
{
    fn parade_match<U>(self, query: U) -> ParadeMatch<Self, U::Expression>
    where
        U: AsExpression<ParadeQuery>,
    {
        ParadeMatch::new(self, query.as_expression())
    }
}
