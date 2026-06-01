mod common;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use diesel_paradedb::{pdb_parse, pdb_score, ParadeMatchDsl};

use common::{products::dsl::*, AnyError, ExampleDatabase};

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    common::print_header("Autocomplete: ngram tokenizer alias queried from Diesel");

    let database = ExampleDatabase::start().await?;
    let mut conn = database.connect().await?;
    common::setup_products(&mut conn).await?;

    for prefix in ["wat", "trai", "grip", "esp"] {
        let query = format!("description_ngram:{prefix}");
        let suggestions: Vec<(String, f32)> = diesel_paradedb_example_products
            .select((name, pdb_score(id)))
            .filter(id.parade_match(pdb_parse(query)))
            .order(pdb_score(id).desc())
            .limit(5)
            .load(&mut conn)
            .await?;

        println!("\nSuggestions for '{prefix}':");
        for (hit_name, score) in suggestions {
            println!("  {hit_name:<28} score={score:.3}");
        }
    }

    Ok(())
}
