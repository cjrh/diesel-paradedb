//! SQL-type markers.

use diesel::sql_types::SqlType;
use diesel::QueryId;

/// Diesel SQL-type marker for ParadeDB's `pdb.query` type.
///
/// Every `pdb::*` builder function in the extension returns a value of
/// this type, and the `@@@` (structured-match) operator expects it on
/// the right. The Postgres OID is resolved at runtime from the type's
/// name + schema; on ParadeDB versions that ship both `pdb.query` and
/// the legacy `paradedb.searchqueryinput`, this marker only binds to
/// `pdb.query`.
#[derive(SqlType, QueryId, Clone, Copy, Debug)]
#[diesel(postgres_type(name = "query", schema = "pdb"))]
pub struct ParadeQuery;
