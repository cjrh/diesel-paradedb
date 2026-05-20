# diesel-paradedb

[Diesel](https://diesel.rs) ORM bindings for [ParadeDB](https://www.paradedb.com)'s `pg_search` extension — BM25 search, structured queries, scoring, and aggregates expressed as ordinary, type-checked Diesel `select` / `filter` / `order` clauses.

[![crates.io](https://img.shields.io/crates/v/diesel-paradedb.svg)](https://crates.io/crates/diesel-paradedb)
[![docs.rs](https://img.shields.io/docsrs/diesel-paradedb)](https://docs.rs/diesel-paradedb)
[![license: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

ParadeDB adds a Postgres type (`pdb.query`), two custom operators (`|||` and `@@@`), and a family of query-builder functions in the `pdb` schema. Without this crate, queries that use any of them have to be written as raw `sql_query` strings, sacrificing Diesel's compile-time checking. With it, every operator, function, and aggregate composes naturally with the rest of your Diesel query:

```rust
use diesel::prelude::*;
use diesel_paradedb::{pdb_all, pdb_score, pdb_agg_terms, ParadeMatchDsl, TermMatchDsl};

// Top-5 BM25 hits ordered by relevance, with a sentiment facet attached
// to every row via the OVER () window form of pdb.agg(...).
let rows: Vec<(i32, String, f32, serde_json::Value)> = feedback::table
    .select((
        feedback::id,
        feedback::text,
        pdb_score(feedback::id),
        pdb_agg_terms("sentiment", 10).over_all(),
    ))
    .filter(feedback::text.term_match("service outage"))
    .order(pdb_score(feedback::id).desc())
    .limit(5)
    .load(conn)?;
```

## Installation

```toml
[dependencies]
diesel = { version = "2.2", features = ["postgres"] }
diesel-paradedb = "1.0"
```

The crate works with both [`diesel`](https://crates.io/crates/diesel) (sync) and [`diesel-async`](https://crates.io/crates/diesel-async). No feature flag is required for async — this crate only adds new expression types, all of which use Diesel's standard `QueryFragment` traits.

You also need a Postgres database with the [ParadeDB `pg_search` extension](https://docs.paradedb.com/) installed and a BM25 index on the columns you want to search. Indexes are created with raw SQL in your migrations:

```sql
CREATE INDEX my_idx ON feedback
  USING bm25 (id, text, (sentiment::pdb.literal))
  WITH (key_field = 'id');
```

## What's modelled

| Item | Postgres form | Rust form |
|------|---------------|-----------|
| Match-type marker | `pdb.query` | `diesel_paradedb::sql_types::ParadeQuery` |
| Term match | `text \|\|\| 'query'` | `text.term_match("query")` |
| Structured match | `id @@@ pdb.all()` | `id.parade_match(pdb_all())` |
| Match-all builder | `pdb.all()` | `pdb_all()` |
| BM25 score | `pdb.score(id)` | `pdb_score(id)` |
| Terms facet | `pdb.agg('{"terms":...}')` | `pdb_agg_terms("field", 10)` |
| Avg facet | `pdb.agg('{"avg":...}')` | `pdb_agg_avg("field")` |
| Stats facet | `pdb.agg('{"stats":...}')` | `pdb_agg_stats("field")` |
| Value-count facet | `pdb.agg('{"value_count":...}')` | `pdb_agg_value_count("field")` |
| Window form | `... OVER ()` | `.over_all()` on any of the above |

## What's NOT modelled

- **BM25 indexes themselves.** Indexes live in your migrations as raw SQL. This crate type-checks queries that *use* an index; existence of the index is a runtime contract.
- **The legacy `paradedb.searchqueryinput` type.** ParadeDB exposes both `pdb.query` (newer) and `paradedb.searchqueryinput` (legacy). This crate models only the newer one.

## Patterns

### BM25 search with score-ordered results

```rust
let hits: Vec<(i32, String, f32)> = feedback::table
    .select((feedback::id, feedback::text, pdb_score(feedback::id)))
    .filter(feedback::text.term_match("billing issue"))
    .order(pdb_score(feedback::id).desc())
    .limit(20)
    .load(conn)?;
```

### Faceting alongside a Top-K BM25 query

The `OVER ()` window form is required: the planner uses ParadeDB's custom scan path to compute facets in one pass, attaching the same facet payload to every row of the Top-K result.

```rust
let (hits, facet): (Vec<(i32, String, f32)>, serde_json::Value) = {
    let rows: Vec<(i32, String, f32, serde_json::Value)> = feedback::table
        .select((
            feedback::id,
            feedback::text,
            pdb_score(feedback::id),
            pdb_agg_terms("sentiment", 10).over_all(),
        ))
        .filter(feedback::text.term_match("login"))
        .order(pdb_score(feedback::id).desc())
        .limit(5)
        .load(conn)?;
    let facet = rows.first().map(|r| r.3.clone()).unwrap_or_default();
    let hits = rows.into_iter().map(|(id, t, s, _)| (id, t, s)).collect();
    (hits, facet)
};
```

### Aggregate form (one row, multiple facets)

When you don't need per-row data, drop the `.over_all()` and the result reduces to one row of facets:

```rust
let (sentiment, ratings, total): (
    serde_json::Value,
    serde_json::Value,
    serde_json::Value,
) = feedback::table
    .select((
        pdb_agg_terms("sentiment", 50),
        pdb_agg_stats("rating"),
        pdb_agg_value_count("id"),
    ))
    .filter(feedback::id.parade_match(pdb_all()))
    .get_result(conn)?;
```

### Trait-resolver overflow on wide selects

Diesel's typed AST composes via tuples; the trait resolver gets quadratically slow once a `select(...)` tuple grows past about 8 `PdbAgg` expressions. If you hit a `LimitDsl` overflow at compile time, split the SELECT into two queries inside the same transaction so the snapshot stays consistent — see [the upstream issue](https://github.com/diesel-rs/diesel/issues/2479) for context. Don't enable Diesel's `32-column-tables` feature without measuring; it bumps the bound but compounds compile times across the workspace.

## Version compatibility

`diesel-paradedb` pins to a specific Diesel minor:

| `diesel-paradedb` | `diesel` |
|-------------------|----------|
| `1.0.x`           | `~2.2`   |

Every Diesel minor bump (e.g. `2.2` → `2.3`) ships as a new `diesel-paradedb` major, since the pin in `Cargo.toml` is `~2.x` and Diesel has broken extension crates on minor bumps before. Pin `diesel-paradedb` in your `Cargo.toml` accordingly.

## Testing

Unit tests cover the JSON shapes that the `pdb_agg_*` builders construct and run with a plain `cargo test`.

End-to-end tests under `tests/live_paradedb.rs` exercise every operator, function, and aggregate against a real ParadeDB container. They're tagged `#[ignore]` so a plain `cargo test` skips them. To run them locally:

```bash
docker run -d --name paradedb -p 5432:5432 \
    -e POSTGRES_PASSWORD=parade \
    paradedb/paradedb:latest

TEST_DATABASE_URL=postgres://postgres:parade@localhost:5432/postgres \
    cargo test --test live_paradedb -- --ignored
```

## Contributing

PRs welcome. The crate's scope is narrow on purpose — only the ParadeDB primitives that don't yet have a Diesel equivalent. New primitives should ship with:
- A short doc comment explaining the Postgres semantics and any non-obvious wiring (look at `aggregates.rs::push_jsonb_literal` for the level of detail expected).
- A unit test for any compile-time-checkable shape (JSON config, SQL rendering via the `QueryFragment` `walk_ast` path).
- An assertion in `tests/live_paradedb.rs` so drift in operator names or function schemas is caught against the real extension.

## License

Dual-licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual-licensed as above, without any additional terms or conditions.
