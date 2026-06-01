mod common;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use diesel_paradedb::{
    pdb_agg_stats, pdb_agg_terms, pdb_agg_value_count, pdb_all, pdb_score, ParadeMatchDsl,
    TermMatchDsl,
};
use serde_json::Value;

use common::{products::dsl::*, AnyError, ExampleDatabase};

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    common::print_header("Faceted search: rows plus ParadeDB aggregates");

    let database = ExampleDatabase::start().await?;
    let mut conn = database.connect().await?;
    common::setup_products(&mut conn).await?;

    let rows: Vec<(i32, String, f32, Value)> = diesel_paradedb_example_products
        .select((
            id,
            name,
            pdb_score(id),
            pdb_agg_terms("category", 10).over_all(),
        ))
        .filter(description.term_match("waterproof trail"))
        .order(pdb_score(id).desc())
        .limit(5)
        .load(&mut conn)
        .await?;

    println!("\nTop rows for 'waterproof trail':");
    for (hit_id, hit_name, score, _) in &rows {
        println!("  #{hit_id:<2} {hit_name:<28} score={score:.3}");
    }

    if let Some((_, _, _, facets)) = rows.as_slice().first() {
        println!("\nCategory facet attached to every row via OVER ():");
        println!("{}", serde_json::to_string_pretty(facets)?);
    }

    let (categories, rating_stats, total): (Value, Value, Value) = diesel_paradedb_example_products
        .select((
            pdb_agg_terms("category", 10),
            pdb_agg_stats("rating"),
            pdb_agg_value_count("id"),
        ))
        .filter(id.parade_match(pdb_all()))
        .get_result(&mut conn)
        .await?;

    println!("\nAggregate-only facets over all indexed products:");
    println!(
        "categories = {}",
        serde_json::to_string_pretty(&categories)?
    );
    println!(
        "rating    = {}",
        serde_json::to_string_pretty(&rating_stats)?
    );
    println!("total     = {}", serde_json::to_string_pretty(&total)?);

    Ok(())
}
