//! SQL-type markers.

use diesel::sql_types::SqlType;
use diesel::QueryId;

/// Diesel SQL-type marker for ParadeDB's `pdb.query` type.
///
/// Most modern `pdb::*` builder functions return a value of this type, and
/// the `@@@` (structured-match) operator accepts it on the right. The
/// Postgres OID is resolved at runtime from the type's name + schema; on
/// ParadeDB versions that ship both `pdb.query` and the legacy
/// `paradedb.searchqueryinput`, this marker only binds to `pdb.query`.
#[derive(SqlType, QueryId, Clone, Copy, Debug)]
#[diesel(postgres_type(name = "query", schema = "pdb"))]
pub struct ParadeQuery;

/// Diesel SQL-type marker for ParadeDB's legacy
/// `paradedb.searchqueryinput` type.
///
/// Most modern query builders return [`ParadeQuery`], but ParadeDB still
/// exposes a few useful builders — notably `pdb.more_like_this(...)` —
/// through this legacy type. The `@@@` operator accepts both types, so this
/// marker is intentionally narrow: it lets those builders compose with the
/// same typed match DSL without exposing callers to raw SQL.
#[derive(SqlType, QueryId, Clone, Copy, Debug)]
#[diesel(postgres_type(name = "searchqueryinput", schema = "paradedb"))]
pub struct SearchQueryInput;
