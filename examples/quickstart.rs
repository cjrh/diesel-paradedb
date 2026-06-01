mod common;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use diesel_paradedb::{
    pdb_parse, pdb_score, pdb_snippet_with_tags, ParadeMatchDsl, PhraseMatchDsl, TermMatchDsl,
};

use common::{products::dsl::*, AnyError, ExampleDatabase};

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    common::print_header("Quickstart: BM25 search from Diesel");

    let database = ExampleDatabase::start().await?;
    let mut conn = database.connect().await?;
    common::setup_products(&mut conn).await?;

    println!("Database: {}", database.url());

    let hits: Vec<(i32, String, f32)> = diesel_paradedb_example_products
        .select((id, name, pdb_score(id)))
        .filter(description.term_match("waterproof hiking"))
        .order(pdb_score(id).desc())
        .limit(5)
        .load(&mut conn)
        .await?;

    println!("\nTop BM25 hits for 'waterproof hiking':");
    for (hit_id, hit_name, score) in hits {
        println!("  #{hit_id:<2} {hit_name:<28} score={score:.3}");
    }

    let phrase_hits: Vec<(String, f32)> = diesel_paradedb_example_products
        .select((name, pdb_score(id)))
        .filter(description.phrase_match("trail shoes"))
        .order(pdb_score(id).desc())
        .load(&mut conn)
        .await?;

    println!("\nPhrase search for 'trail shoes':");
    for (hit_name, score) in phrase_hits {
        println!("  {hit_name:<28} score={score:.3}");
    }

    let parsed_hits: Vec<(String, f32)> = diesel_paradedb_example_products
        .select((name, pdb_score(id)))
        .filter(id.parade_match(pdb_parse("description:waterproof AND category:outdoors")))
        .order(pdb_score(id).desc())
        .load(&mut conn)
        .await?;

    println!("\nStructured query: description:waterproof AND category:outdoors");
    for (hit_name, score) in parsed_hits {
        println!("  {hit_name:<28} score={score:.3}");
    }

    let snippets: Vec<(String, String)> = diesel_paradedb_example_products
        .select((name, pdb_snippet_with_tags(description, "**", "**")))
        .filter(description.term_match("waterproof"))
        .order(pdb_score(id).desc())
        .limit(3)
        .load(&mut conn)
        .await?;

    println!("\nHighlighted snippets:");
    for (hit_name, snippet) in snippets {
        println!("  {hit_name}: {snippet}");
    }

    Ok(())
}
