//! End-to-end verification of the typed DSL against a live ParadeDB
//! container.
//!
//! The test starts ParadeDB with `testcontainers` by default, so contributors do
//! not need a database already running on `localhost:5432`. Set
//! `TEST_DATABASE_URL` to point at an existing database while debugging.
//!
//! These tests are tagged `#[ignore]` so a plain `cargo test` still runs without
//! Docker. Run them with:
//!
//!     cargo test --test live_paradedb -- --ignored --nocapture
//!
//! The assertions check query shape and sane behavior (non-empty results,
//! expected ordering, expected JSON keys), not exact BM25 scores.

#[path = "../examples/common/mod.rs"]
mod example_common;

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use diesel_paradedb::{
    pdb_agg_avg, pdb_agg_stats, pdb_agg_terms, pdb_agg_value_count, pdb_all, pdb_more_like_this,
    pdb_more_like_this_in_fields, pdb_parse, pdb_score, pdb_snippet_with_tags, ParadeMatchDsl,
    PhraseMatchDsl, TermMatchDsl,
};
use example_common::{AnyError, ExampleDatabase};

diesel::table! {
    // SQL-side name is namespaced so the table can't collide with a user table
    // if TEST_DATABASE_URL points at a shared database.
    #[sql_name = "diesel_paradedb_test_docs"]
    docs (id) {
        id -> Int4,
        body -> Text,
        rating -> Float4,
        sentiment -> Text,
    }
}

/// Set up a clean fixture: drop+create the test table, insert four rows with
/// predictable text/sentiment/rating values, then build a BM25 index over it.
async fn setup(conn: &mut AsyncPgConnection) -> Result<(), AnyError> {
    let stmts = [
        "CREATE EXTENSION IF NOT EXISTS pg_search",
        "DROP TABLE IF EXISTS diesel_paradedb_test_docs",
        "CREATE TABLE diesel_paradedb_test_docs (
            id        SERIAL PRIMARY KEY,
            body      TEXT NOT NULL,
            rating    REAL NOT NULL,
            sentiment TEXT NOT NULL
        )",
        "INSERT INTO diesel_paradedb_test_docs (body, rating, sentiment) VALUES
            ('the service was excellent and fast',   4.5, 'positive'),
            ('terrible service slow and rude staff', 1.5, 'negative'),
            ('decent experience overall',            3.0, 'neutral'),
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
        diesel::sql_query(stmt).execute(conn).await?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires Docker/testcontainers or TEST_DATABASE_URL"]
async fn full_dsl_coverage_against_live_paradedb() -> Result<(), AnyError> {
    use self::docs::dsl::*;

    let database = ExampleDatabase::start().await?;
    let mut conn = database.connect().await?;
    setup(&mut conn).await?;

    // -- term_match: BM25 hits for "service" --
    let hits: Vec<(i32, String)> = docs
        .select((id, body))
        .filter(body.term_match("service"))
        .load(&mut conn)
        .await?;
    assert!(
        hits.len() >= 3,
        "expected at least 3 rows matching 'service', got {}: {:?}",
        hits.len(),
        hits
    );

    // -- phrase_match: `###` phrase operator against a text field --
    let phrase_hits: Vec<i32> = docs
        .select(id)
        .filter(body.phrase_match("great service"))
        .order(pdb_score(id).desc())
        .load(&mut conn)
        .await?;
    assert_eq!(phrase_hits.as_slice().first().copied(), Some(4));

    // -- parade_match + pdb_all: matches every row --
    let matched: i64 = docs
        .filter(id.parade_match(pdb_all()))
        .count()
        .get_result(&mut conn)
        .await?;
    let total: i64 = docs.count().get_result(&mut conn).await?;
    assert_eq!(matched, total, "id @@@ pdb.all() should match every row");
    assert_eq!(total, 4);

    // -- pdb_parse: structured query string targeting indexed fields --
    let parsed: Vec<i32> = docs
        .select(id)
        .filter(id.parade_match(pdb_parse("body:service AND sentiment:positive")))
        .order(id.asc())
        .load(&mut conn)
        .await?;
    assert_eq!(parsed, vec![1, 4]);

    // -- pdb_score: rows come back in monotonically non-increasing order --
    let scores: Vec<f32> = docs
        .select(pdb_score(id))
        .filter(body.term_match("service"))
        .order(pdb_score(id).desc())
        .load::<f32>(&mut conn)
        .await?;
    assert!(!scores.is_empty());
    for pair in scores.windows(2) {
        assert!(
            pair[0] >= pair[1],
            "scores not monotonically decreasing: {scores:?}"
        );
    }

    // -- pdb_snippet: highlighting uses the active BM25 scan context --
    let snippet: String = docs
        .select(pdb_snippet_with_tags(body, "[[", "]]"))
        .filter(body.term_match("service"))
        .order(pdb_score(id).desc())
        .first(&mut conn)
        .await?;
    assert!(
        snippet.contains("[[service]]"),
        "snippet should highlight service with custom tags, got: {snippet}"
    );

    // -- pdb_more_like_this: ParadeDB still returns legacy searchqueryinput;
    //    the typed `@@@` DSL accepts it. --
    let related: Vec<i32> = docs
        .select(id)
        .filter(id.parade_match(pdb_more_like_this(1)))
        .filter(id.ne(1))
        .order(pdb_score(id).desc())
        .load(&mut conn)
        .await?;
    assert!(
        related.contains(&4),
        "row 4 should be similar to row 1, got: {related:?}"
    );

    let related_in_fields: Vec<i32> = docs
        .select(id)
        .filter(id.parade_match(pdb_more_like_this_in_fields(1, vec!["body".to_string()])))
        .filter(id.ne(1))
        .order(pdb_score(id).desc())
        .load(&mut conn)
        .await?;
    assert!(
        related_in_fields.contains(&4),
        "field-restricted MLT should find row 4, got: {related_in_fields:?}"
    );

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
        .await?;
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
    let (terms, stats, count): (serde_json::Value, serde_json::Value, serde_json::Value) = docs
        .select((
            pdb_agg_terms("sentiment", 50),
            pdb_agg_stats("rating"),
            pdb_agg_value_count("id"),
        ))
        .filter(id.parade_match(pdb_all()))
        .get_result(&mut conn)
        .await?;
    assert!(terms.get("buckets").is_some());
    assert!(stats.get("avg").is_some());
    assert!(stats.get("count").is_some());
    assert!(count.get("value").is_some());

    diesel::sql_query("DROP TABLE diesel_paradedb_test_docs")
        .execute(&mut conn)
        .await?;

    Ok(())
}
