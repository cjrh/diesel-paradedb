mod common;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use diesel_paradedb::{pdb_more_like_this_in_fields, pdb_score, ParadeMatchDsl};

use common::{products::dsl::*, AnyError, ExampleDatabase};

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    common::print_header("More like this: related products without embeddings");

    let database = ExampleDatabase::start().await?;
    let mut conn = database.connect().await?;
    common::setup_products(&mut conn).await?;

    let source_id = 1;
    let source_name: String = diesel_paradedb_example_products
        .select(name)
        .filter(id.eq(source_id))
        .get_result(&mut conn)
        .await?;

    let related: Vec<(i32, String, f32)> = diesel_paradedb_example_products
        .select((id, name, pdb_score(id)))
        .filter(id.parade_match(pdb_more_like_this_in_fields(
            source_id,
            vec!["description".to_string()],
        )))
        .filter(id.ne(source_id))
        .order(pdb_score(id).desc())
        .limit(5)
        .load(&mut conn)
        .await?;

    println!("Source: #{source_id} {source_name}\n");
    println!("Related rows from pdb.more_like_this({source_id}, ARRAY['description']):");
    for (hit_id, hit_name, score) in related {
        println!("  #{hit_id:<2} {hit_name:<28} score={score:.3}");
    }

    Ok(())
}
