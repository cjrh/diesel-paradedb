mod common;

use diesel::prelude::*;
use diesel::sql_types::{Double, Float4, Integer, Text};
use diesel_async::RunQueryDsl;
use pgvector::{sql_types::Vector as VectorSql, Vector};

use common::{fake_embedding, AnyError, ExampleDatabase};

#[derive(Debug, QueryableByName)]
struct HybridHit {
    #[diesel(sql_type = Integer)]
    id: i32,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    category: String,
    #[diesel(sql_type = Float4)]
    bm25_score: f32,
    #[diesel(sql_type = Double)]
    rrf_score: f64,
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    common::print_header("Hybrid search: BM25 + pgvector via Reciprocal Rank Fusion");

    let database = ExampleDatabase::start().await?;
    let mut conn = database.connect().await?;
    common::setup_products(&mut conn).await?;

    let query = "waterproof hiking trail";
    let query_embedding = Vector::from(fake_embedding(42, query));

    let hits: Vec<HybridHit> = diesel::sql_query(
        r#"
        WITH bm25 AS (
            SELECT id,
                   pdb.score(id) AS bm25_score,
                   row_number() OVER (ORDER BY pdb.score(id) DESC) AS rank
            FROM diesel_paradedb_example_products
            WHERE description ||| $1
            LIMIT 8
        ),
        semantic AS (
            SELECT id,
                   row_number() OVER (ORDER BY embedding <=> $2) AS rank
            FROM diesel_paradedb_example_products
            ORDER BY embedding <=> $2
            LIMIT 8
        ),
        fused AS (
            SELECT id, sum(score) AS rrf_score
            FROM (
                SELECT id, 1.0 / (60 + rank) AS score FROM bm25
                UNION ALL
                SELECT id, 1.0 / (60 + rank) AS score FROM semantic
            ) ranked
            GROUP BY id
        )
        SELECT p.id,
               p.name,
               p.category,
               COALESCE(bm25.bm25_score, 0)::real AS bm25_score,
               fused.rrf_score::double precision AS rrf_score
        FROM fused
        JOIN diesel_paradedb_example_products p ON p.id = fused.id
        LEFT JOIN bm25 ON bm25.id = fused.id
        ORDER BY fused.rrf_score DESC
        LIMIT 5
        "#,
    )
    .bind::<Text, _>(query)
    .bind::<VectorSql, _>(query_embedding)
    .load(&mut conn)
    .await?;

    println!("Query: {query}");
    println!("Embedding: deterministic fake vector (no external LLM call)\n");
    for hit in hits {
        println!(
            "  #{:<2} {:<28} {:<10} bm25={:.3} rrf={:.5}",
            hit.id, hit.name, hit.category, hit.bm25_score, hit.rrf_score
        );
    }

    Ok(())
}
