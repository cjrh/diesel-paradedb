mod common;

use diesel::prelude::*;
use diesel::sql_types::{Double, Integer, Text};
use diesel_async::RunQueryDsl;
use pgvector::{sql_types::Vector as VectorSql, Vector};

use common::{fake_embedding, AnyError, ExampleDatabase};

#[derive(Debug, QueryableByName)]
struct RetrievedContext {
    #[diesel(sql_type = Integer)]
    id: i32,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    description: String,
    #[diesel(sql_type = Double)]
    rrf_score: f64,
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    common::print_header("RAG: retrieve context, then assemble a prompt");

    let database = ExampleDatabase::start().await?;
    let mut conn = database.connect().await?;
    common::setup_products(&mut conn).await?;

    let question = "What should I buy for a rainy mountain hike?";
    let lexical_query = "waterproof hiking mountain rain";
    let query_embedding = Vector::from(fake_embedding(99, question));

    let contexts: Vec<RetrievedContext> = diesel::sql_query(
        r#"
        WITH bm25 AS (
            SELECT id, row_number() OVER (ORDER BY pdb.score(id) DESC) AS rank
            FROM diesel_paradedb_example_products
            WHERE description ||| $1
            LIMIT 6
        ),
        semantic AS (
            SELECT id, row_number() OVER (ORDER BY embedding <=> $2) AS rank
            FROM diesel_paradedb_example_products
            ORDER BY embedding <=> $2
            LIMIT 6
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
               p.description,
               fused.rrf_score::double precision AS rrf_score
        FROM fused
        JOIN diesel_paradedb_example_products p ON p.id = fused.id
        ORDER BY fused.rrf_score DESC
        LIMIT 4
        "#,
    )
    .bind::<Text, _>(lexical_query)
    .bind::<VectorSql, _>(query_embedding)
    .load(&mut conn)
    .await?;

    println!("Question: {question}\n");
    println!("Retrieved context:");
    for row in &contexts {
        println!(
            "  [{:.5}] #{} {} — {}",
            row.rrf_score, row.id, row.name, row.description
        );
    }

    let context_block = contexts
        .iter()
        .map(|row| format!("- {}: {}", row.name, row.description))
        .collect::<Vec<_>>()
        .join("\n");

    println!("\nPrompt you would send to an LLM:");
    println!(
        "You are a product assistant. Answer using only this context:\n\n{context_block}\n\nQuestion: {question}"
    );

    println!("\nFake answer (no external LLM call):");
    println!(
        "For a rainy mountain hike, start with the waterproof hiking boots and pair them with the packable rain jacket."
    );

    Ok(())
}
