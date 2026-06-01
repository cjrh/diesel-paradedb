<p align="center">
  <b>Diesel ORM bindings for ParadeDB full-text search</b><br/>
</p>

<p align="center">
  <a href="https://crates.io/crates/diesel-paradedb"><img src="https://img.shields.io/crates/v/diesel-paradedb.svg" alt="crates.io"></a>&nbsp;
  <a href="https://docs.rs/diesel-paradedb"><img src="https://img.shields.io/docsrs/diesel-paradedb" alt="docs.rs"></a>&nbsp;
  <a href="https://github.com/cjrh/diesel-paradedb/actions/workflows/ci.yml"><img src="https://github.com/cjrh/diesel-paradedb/actions/workflows/ci.yml/badge.svg" alt="CI"></a>&nbsp;
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg" alt="License"></a>&nbsp;
</p>

---

## ParadeDB for Diesel

`diesel-paradedb` lets [Diesel](https://diesel.rs) queries use [ParadeDB](https://www.paradedb.com)'s `pg_search` extension without dropping into raw `sql_query` strings for every search. BM25 search, structured query parsing, phrase matching, scoring, snippets, more-like-this, and facets compose as ordinary typed Diesel `select` / `filter` / `order` clauses.

```rust
use diesel::prelude::*;
use diesel_paradedb::{
    pdb_agg_terms, pdb_score, pdb_snippet_with_tags, TermMatchDsl,
};

// Top-5 BM25 hits ordered by relevance, with a category facet attached
// to every row via the OVER () window form of pdb.agg(...).
let rows: Vec<(i32, String, f32, String, serde_json::Value)> = products::table
    .select((
        products::id,
        products::name,
        pdb_score(products::id),
        pdb_snippet_with_tags(products::description, "<mark>", "</mark>"),
        pdb_agg_terms("category", 10).over_all(),
    ))
    .filter(products::description.term_match("waterproof hiking"))
    .order(pdb_score(products::id).desc())
    .limit(5)
    .load(conn)?;
```

## Requirements & Compatibility

| Component | Supported |
|-----------|-----------|
| Rust | 1.86+ |
| Diesel | `>=2.2, <2.4` |
| ParadeDB | `pg_search` with the `pdb` schema (`paradedb/paradedb:latest` in tests) |
| PostgreSQL | A PostgreSQL server that can load ParadeDB's `pg_search` extension; examples also use `pgvector` |
| Docker | Required only for the testcontainers-powered live tests and examples |

## Installation

```toml
[dependencies]
diesel = { version = "2.2", features = ["postgres"] }
diesel-paradedb = "1.0"
```

The crate works with both [`diesel`](https://crates.io/crates/diesel) (sync) and [`diesel-async`](https://crates.io/crates/diesel-async). No feature flag is required for async — this crate only adds expression types that use Diesel's standard `QueryFragment` traits.

You also need a Postgres database with the [ParadeDB `pg_search` extension](https://docs.paradedb.com/) installed and a BM25 index on the columns you want to search. Indexes belong in migrations as raw SQL:

```sql
CREATE EXTENSION IF NOT EXISTS pg_search;

CREATE INDEX products_search_idx ON products
  USING bm25 (
    id,
    name,
    (description::pdb.unicode_words),
    (description::pdb.ngram(3, 8, 'alias=description_ngram')),
    (category::pdb.literal),
    rating
  ) WITH (key_field = 'id');
```

## Examples

The examples mirror the Django integration's example set and start a ParadeDB container with [`testcontainers`](https://testcontainers.com/) by default, so no local database has to be pre-provisioned. Set `TEST_DATABASE_URL` if you want to reuse an existing server instead; that server must have both `pg_search` and `vector` available because the shared example fixture includes a fake embedding column for the hybrid/RAG demos.

- [Quickstart](examples/quickstart.rs) — keyword search, phrase search, structured parsing, scoring, and snippets
- [Faceted Search](examples/faceted_search.rs) — Top-K rows plus `pdb.agg(...) OVER ()` facets
- [Autocomplete](examples/autocomplete.rs) — ngram tokenizer alias queried through `pdb.parse(...)`
- [More Like This](examples/more_like_this.rs) — related rows from `pdb.more_like_this(...)`
- [Hybrid Search (RRF)](examples/hybrid_rrf.rs) — BM25 + `pgvector` fused with Reciprocal Rank Fusion
- [RAG](examples/rag.rs) — retrieval + prompt assembly with deterministic fake vectors and no external LLM call

The quickstart, faceting, autocomplete, and more-like-this examples use typed Diesel expressions for ParadeDB searches. The hybrid/RAG examples intentionally use `diesel::sql_query` for the multi-CTE RRF query that combines ParadeDB and pgvector; that raw SQL is example orchestration, not new library API surface.

Run one example:

```bash
cargo run --example quickstart
```

Run or check all examples:

```bash
just examples        # starts containers and runs every example
just check-examples  # compile only; no containers
```

## What's modelled

| Item | Postgres form | Rust form |
|------|---------------|-----------|
| Match-type marker | `pdb.query` | `diesel_paradedb::sql_types::ParadeQuery` |
| Legacy MLT marker | `paradedb.searchqueryinput` | `diesel_paradedb::sql_types::SearchQueryInput` |
| Term match | `description \|\|\| 'query'` | `description.term_match("query")` |
| Phrase match | `description ### 'running shoes'` | `description.phrase_match("running shoes")` |
| Structured match | `id @@@ pdb.all()` | `id.parade_match(pdb_all())` |
| Match-all builder | `pdb.all()` | `pdb_all()` |
| Query-string parser | `pdb.parse('field:value')` | `pdb_parse("field:value")` |
| BM25 score | `pdb.score(id)` | `pdb_score(id)` |
| Snippet | `pdb.snippet(description)` | `pdb_snippet(description)` |
| Snippet with tags | `pdb.snippet(description, '<b>', '</b>')` | `pdb_snippet_with_tags(description, "<b>", "</b>")` |
| More like this | `pdb.more_like_this(id)` | `pdb_more_like_this(id)` |
| More like this in fields | `pdb.more_like_this(id, ARRAY['description'])` | `pdb_more_like_this_in_fields(id, vec!["description".to_string()])` |
| Terms facet | `pdb.agg('{"terms":...}')` | `pdb_agg_terms("field", 10)` |
| Avg facet | `pdb.agg('{"avg":...}')` | `pdb_agg_avg("field")` |
| Stats facet | `pdb.agg('{"stats":...}')` | `pdb_agg_stats("field")` |
| Value-count facet | `pdb.agg('{"value_count":...}')` | `pdb_agg_value_count("field")` |
| Window form | `... OVER ()` | `.over_all()` on any `pdb_agg_*` builder |

## What's not modelled

- **BM25 indexes themselves.** Index creation stays in SQL migrations. This crate type-checks queries that use an index; existence and configuration of the index is a runtime contract.
- **The entire ParadeDB query-builder surface.** The crate focuses on the primitives needed for common Diesel workflows and the examples above. New builders should be added with live ParadeDB coverage.
- **Vector search APIs.** Use [`pgvector`](https://crates.io/crates/pgvector) for vector columns/operators. The hybrid/RAG examples show one way to combine pgvector with ParadeDB BM25.

## Patterns

### BM25 search with score-ordered results

```rust
let hits: Vec<(i32, String, f32)> = products::table
    .select((products::id, products::name, pdb_score(products::id)))
    .filter(products::description.term_match("waterproof hiking"))
    .order(pdb_score(products::id).desc())
    .limit(20)
    .load(conn)?;
```

### Structured query strings

```rust
let outdoors: Vec<(i32, String, f32)> = products::table
    .select((products::id, products::name, pdb_score(products::id)))
    .filter(products::id.parade_match(pdb_parse(
        "description:waterproof AND category:outdoors",
    )))
    .order(pdb_score(products::id).desc())
    .load(conn)?;
```

### Faceting alongside a Top-K BM25 query

The `OVER ()` window form is required when you want row data and facets together: the planner uses ParadeDB's custom scan path to compute facets in one pass and attach the same facet payload to every row of the Top-K result.

```rust
let rows: Vec<(i32, String, f32, serde_json::Value)> = products::table
    .select((
        products::id,
        products::name,
        pdb_score(products::id),
        pdb_agg_terms("category", 10).over_all(),
    ))
    .filter(products::description.term_match("trail"))
    .order(pdb_score(products::id).desc())
    .limit(5)
    .load(conn)?;
```

### Aggregate form (one row, multiple facets)

When you don't need per-row data, drop `.over_all()` and the result reduces to one row of facets:

```rust
let (categories, ratings, total): (
    serde_json::Value,
    serde_json::Value,
    serde_json::Value,
) = products::table
    .select((
        pdb_agg_terms("category", 50),
        pdb_agg_stats("rating"),
        pdb_agg_value_count("id"),
    ))
    .filter(products::id.parade_match(pdb_all()))
    .get_result(conn)?;
```

### Trait-resolver overflow on wide selects

Diesel's typed AST composes via tuples; the trait resolver gets quadratically slow once a `select(...)` tuple grows past about 8 `PdbAgg` expressions. If you hit a `LimitDsl` overflow at compile time, split the SELECT into two queries inside the same transaction so the snapshot stays consistent — see [the upstream issue](https://github.com/diesel-rs/diesel/issues/2479) for context. Don't enable Diesel's `32-column-tables` feature without measuring; it bumps the bound but compounds compile times across the workspace.

## Testing

The repo ships a [`justfile`](https://github.com/casey/just). After cloning, the full suite is one command:

```bash
just test
```

That runs unit tests, compiles examples, starts a ParadeDB container through testcontainers, and runs live integration tests against it.

Individual recipes (`just --list` for the full set):

```bash
just test-unit       # JSON-shape checks; no database needed
just test-live       # starts ParadeDB with testcontainers and runs live tests
just check-examples  # compile examples without starting containers
just example quickstart
```

Set `TEST_DATABASE_URL` to point tests/examples at an existing server instead of starting a container:

```bash
TEST_DATABASE_URL=postgres://postgres:parade@localhost:5432/postgres \
  cargo run --example quickstart
```

## Version compatibility

`diesel-paradedb` is validated against specific Diesel minors:

| `diesel-paradedb` | `diesel` |
|-------------------|----------|
| `1.0.x`           | `>=2.2, <2.4` |

Every Diesel minor bump is tested before the upper bound is widened, since Diesel has broken extension crates on minor bumps before. Pin `diesel-paradedb` in your `Cargo.toml` accordingly.

## Contributing

PRs welcome. The crate's scope is narrow on purpose — only the ParadeDB primitives that don't yet have a Diesel equivalent. New primitives should ship with:

- A short doc comment explaining the Postgres semantics and any non-obvious wiring.
- A unit test for any compile-time-checkable shape.
- A live assertion in `tests/live_paradedb.rs` so drift in operator names or function schemas is caught against the real extension.

## Support

If you're missing a feature or have found a bug, please open a [GitHub Issue](https://github.com/cjrh/diesel-paradedb/issues/new).

For community support, join the [ParadeDB Slack](https://paradedb.com/slack/).

## Releasing

Releases are cut with [`cargo release`](https://github.com/crate-ci/cargo-release), which bumps the version, updates `CHANGELOG.md`, tags, and publishes to crates.io in one step:

```bash
cargo release patch   # or: minor / major
```

## License

Dual-licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual-licensed as above, without any additional terms or conditions.
