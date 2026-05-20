//! End-to-end verification of the typed DSL against a live ParadeDB
//! container.
//!
//! Every test composes a query using ONLY the typed DSL (no `sql_query`!),
//! runs it against a freshly-created scratch table inside a single
//! `tokio::test`, and asserts the result is sane. The point is to catch
//! drift in:
//!
//! - operator names (`|||`, `@@@`) or precedence
//! - function names / schemas (`pdb.all`, `pdb.score`)
//! - the bind-parameter shape for `pdb.agg(jsonb)`
//! - the `OVER ()` window-clause spelling
//!
//! These tests are tagged `#[ignore]` so a plain `cargo test` skips them.
//! Run against a ParadeDB instance with:
//!
//!     docker run -d --name paradedb -p 5432:5432 \
//!         -e POSTGRES_PASSWORD=parade paradedb/paradedb:latest
//!     cargo test --test live_paradedb -- --ignored
//!
//! Override the connection URL via the `TEST_DATABASE_URL` env var;
//! defaults to `postgres://postgres:parade@localhost:5432/postgres`.
//!
//! Tests assert *shape* (non-empty results, expected ordering, expected
//! JSON keys), not exact values — the synthetic fixture is small but
//! the test would otherwise be brittle without buying correctness.

use diesel::prelude::*;
use diesel_async::pooled_connection::bb8::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use diesel_paradedb::{
    pdb_agg_avg, pdb_agg_stats, pdb_agg_terms, pdb_agg_value_count, pdb_all, pdb_score,
    ParadeMatchDsl, TermMatchDsl,
};

const DEFAULT_URL: &str = "postgres://postgres:parade@localhost:5432/postgres";

diesel::table! {
    // SQL-side name is namespaced so the table can't collide with a
    // user table called `docs` if the test container is shared.
    // Rust-side identifier stays short — saves repeating the long
    // name at every call site.
    #[sql_name = "diesel_paradedb_test_docs"]
    docs (id) {
        id -> Int4,
        body -> Text,
        rating -> Float4,
        sentiment -> Text,
    }
}

async fn pool() -> Pool<AsyncPgConnection> {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(&url);
    Pool::builder()
        .max_size(2)
        .build(config)
        .await
        .expect("build pool — is ParadeDB running on $TEST_DATABASE_URL?")
}

/// Set up a clean fixture: drop+create the test table, insert four rows
/// with predictable text/sentiment/rating values, then build a BM25
/// index over it. All tests assume this state and run in the same
/// `#[tokio::test]` so they share the setup cost.
async fn setup(conn: &mut AsyncPgConnection) {
    let stmts = [
        "DROP TABLE IF EXISTS diesel_paradedb_test_docs",
        "CREATE TABLE diesel_paradedb_test_docs (
            id        SERIAL PRIMARY KEY,
            body      TEXT NOT NULL,
            rating    REAL NOT NULL,
            sentiment TEXT NOT NULL
        )",
        "INSERT INTO diesel_paradedb_test_docs (body, rating, sentiment) VALUES
            ('the service was excellent and fast',   4.5, 'positive'),
            ('terrible service slow and rude staff',  1.5, 'negative'),
            ('decent experience overall',             3.0, 'neutral'),
            ('great service great food great vibes', 5.0, 'positive')",
        "CREATE INDEX diesel_paradedb_test_idx ON diesel_paradedb_test_docs
         USING bm25 (
            id,
            body,
            rating,
            (sentiment::pdb.literal)
         ) WITH (key_field = 'id')",
    ];
    for stmt in stmts {
        diesel::sql_query(stmt)
            .execute(conn)
            .await
            .unwrap_or_else(|e| panic!("setup statement failed: {stmt}\n{e}"));
    }
}

#[tokio::test]
#[ignore = "requires running ParadeDB; see test file header for setup"]
async fn full_dsl_coverage_against_live_paradedb() {
    use self::docs::dsl::*;

    let pool = pool().await;
    let mut conn = pool.get().await.expect("checkout connection");
    setup(&mut conn).await;

    // -- term_match: BM25 hits for "service" --
    let hits: Vec<(i32, String)> = docs
        .select((id, body))
        .filter(body.term_match("service"))
        .load(&mut conn)
        .await
        .expect("term_match");
    assert!(
        hits.len() >= 3,
        "expected at least 3 rows matching 'service', got {}: {:?}",
        hits.len(),
        hits
    );

    // -- parade_match + pdb_all: matches every row --
    let (matched, total): (i64, i64) = {
        let m: i64 = docs
            .filter(id.parade_match(pdb_all()))
            .count()
            .get_result(&mut conn)
            .await
            .expect("parade_match count");
        let t: i64 = docs
            .count()
            .get_result(&mut conn)
            .await
            .expect("bare count");
        (m, t)
    };
    assert_eq!(matched, total, "id @@@ pdb.all() should match every row");
    assert_eq!(total, 4);

    // -- pdb_score: rows come back in monotonically non-increasing order --
    let scores: Vec<f32> = docs
        .select(pdb_score(id))
        .filter(body.term_match("service"))
        .order(pdb_score(id).desc())
        .load::<f32>(&mut conn)
        .await
        .expect("score");
    assert!(!scores.is_empty());
    for pair in scores.windows(2) {
        assert!(
            pair[0] >= pair[1],
            "scores not monotonically decreasing: {scores:?}"
        );
    }

    // -- pdb_agg in window form: facets attached to every Top-K row,
    //    identical across rows (the OVER () invariant). --
    let rows: Vec<(i32, f32, serde_json::Value, serde_json::Value)> = docs
        .select((
            id,
            pdb_score(id),
            pdb_agg_terms("sentiment", 10).over_all(),
            pdb_agg_avg("rating").over_all(),
        ))
        .filter(body.term_match("service"))
        .order(pdb_score(id).desc())
        .limit(3)
        .load::<(i32, f32, serde_json::Value, serde_json::Value)>(&mut conn)
        .await
        .expect("window-form pdb.agg");
    assert!(!rows.is_empty());
    let first_facet = &rows[0].2;
    assert!(
        rows.iter().all(|r| &r.2 == first_facet),
        "OVER () facet should be identical for every row"
    );
    assert!(
        first_facet.get("buckets").is_some(),
        "terms facet must have a buckets array, got: {first_facet}"
    );

    // -- pdb_agg in aggregate form: one row, multiple facets. --
    let (terms, stats, count): (serde_json::Value, serde_json::Value, serde_json::Value) =
        docs
            .select((
                pdb_agg_terms("sentiment", 50),
                pdb_agg_stats("rating"),
                pdb_agg_value_count("id"),
            ))
            .filter(id.parade_match(pdb_all()))
            .get_result(&mut conn)
            .await
            .expect("aggregate-form pdb.agg");
    assert!(terms.get("buckets").is_some());
    assert!(stats.get("avg").is_some());
    assert!(stats.get("count").is_some());
    assert!(count.get("value").is_some());

    // Cleanup so re-runs are idempotent.
    diesel::sql_query("DROP TABLE diesel_paradedb_test_docs")
        .execute(&mut conn)
        .await
        .expect("teardown");
}
