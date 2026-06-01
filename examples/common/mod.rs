#![allow(dead_code)]

use std::{env, error::Error, time::Duration};

use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use pgvector::Vector;
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    ContainerAsync, GenericImage, ImageExt,
};
use tokio::time::sleep;

pub type AnyError = Box<dyn Error + Send + Sync>;

const POSTGRES_PORT: u16 = 5432;
const DEFAULT_IMAGE: &str = "paradedb/paradedb";
const DEFAULT_TAG: &str = "latest";
const DEFAULT_PASSWORD: &str = "parade";

pub mod schema {
    diesel::table! {
        use diesel::sql_types::*;
        use pgvector::sql_types::*;

        diesel_paradedb_example_products (id) {
            id -> Int4,
            name -> Text,
            description -> Text,
            category -> Text,
            rating -> Float4,
            price -> Float4,
            embedding -> Vector,
        }
    }
}

pub use schema::diesel_paradedb_example_products as products;

/// A ParadeDB database owned by the example process.
///
/// By default this starts a `paradedb/paradedb` container with a random host
/// port. Set `TEST_DATABASE_URL` to reuse an existing database while debugging;
/// the examples still create/drop only their own fixture table.
pub struct ExampleDatabase {
    url: String,
    _container: Option<ContainerAsync<GenericImage>>,
}

impl ExampleDatabase {
    pub async fn start() -> Result<Self, AnyError> {
        if let Ok(url) = env::var("TEST_DATABASE_URL") {
            return Ok(Self {
                url,
                _container: None,
            });
        }

        let image = env::var("PARADEDB_IMAGE").unwrap_or_else(|_| DEFAULT_IMAGE.to_string());
        let tag = env::var("PARADEDB_IMAGE_TAG").unwrap_or_else(|_| DEFAULT_TAG.to_string());
        let container = GenericImage::new(image, tag)
            .with_exposed_port(POSTGRES_PORT.tcp())
            .with_wait_for(WaitFor::message_on_stderr(
                "database system is ready to accept connections",
            ))
            .with_startup_timeout(Duration::from_secs(180))
            .with_env_var("POSTGRES_PASSWORD", DEFAULT_PASSWORD)
            .start()
            .await?;

        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(POSTGRES_PORT.tcp()).await?;
        let url = format!("postgres://postgres:{DEFAULT_PASSWORD}@{host}:{port}/postgres");

        Ok(Self {
            url,
            _container: Some(container),
        })
    }

    #[allow(dead_code)]
    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn connect(&self) -> Result<AsyncPgConnection, AnyError> {
        connect_with_retry(&self.url).await
    }
}

async fn connect_with_retry(url: &str) -> Result<AsyncPgConnection, AnyError> {
    let mut last_error = None;
    for _ in 0..60 {
        match AsyncPgConnection::establish(url).await {
            Ok(conn) => return Ok(conn),
            Err(err) => {
                last_error = Some(err);
                sleep(Duration::from_millis(500)).await;
            }
        }
    }

    Err(format!(
        "timed out connecting to ParadeDB at {url}: {}",
        last_error
            .map(|err| err.to_string())
            .unwrap_or_else(|| "no attempts made".to_string())
    )
    .into())
}

#[derive(Insertable)]
#[diesel(table_name = schema::diesel_paradedb_example_products)]
struct NewProduct {
    name: String,
    description: String,
    category: String,
    rating: f32,
    price: f32,
    embedding: Vector,
}

pub async fn setup_products(conn: &mut AsyncPgConnection) -> Result<(), AnyError> {
    let statements = [
        "CREATE EXTENSION IF NOT EXISTS pg_search",
        "CREATE EXTENSION IF NOT EXISTS vector",
        "DROP TABLE IF EXISTS diesel_paradedb_example_products",
        "CREATE TABLE diesel_paradedb_example_products (
            id          SERIAL PRIMARY KEY,
            name        TEXT NOT NULL,
            description TEXT NOT NULL,
            category    TEXT NOT NULL,
            rating      REAL NOT NULL,
            price       REAL NOT NULL,
            embedding   VECTOR(3) NOT NULL
        )",
    ];

    for statement in statements {
        diesel::sql_query(statement).execute(conn).await?;
    }

    let rows = sample_products();
    diesel::insert_into(products::table)
        .values(&rows)
        .execute(conn)
        .await?;

    diesel::sql_query(
        "CREATE INDEX diesel_paradedb_example_products_idx
         ON diesel_paradedb_example_products
         USING bm25 (
            id,
            name,
            (description::pdb.unicode_words),
            (description::pdb.ngram(3, 8, 'alias=description_ngram')),
            (category::pdb.literal),
            rating,
            price
         ) WITH (key_field = 'id')",
    )
    .execute(conn)
    .await?;

    Ok(())
}

fn sample_products() -> Vec<NewProduct> {
    let rows = [
        (
            "Waterproof hiking boots",
            "rugged waterproof boots for mountain trails and rainy weekends",
            "outdoors",
            4.8,
            129.0,
        ),
        (
            "Trail running shoes",
            "lightweight trail shoes with grippy soles for fast rocky routes",
            "outdoors",
            4.5,
            89.0,
        ),
        (
            "City walking shoes",
            "comfortable leather shoes for commuting and city travel",
            "footwear",
            4.1,
            74.0,
        ),
        (
            "Rain jacket",
            "packable waterproof jacket for hiking camping and travel",
            "outdoors",
            4.7,
            149.0,
        ),
        (
            "Espresso grinder",
            "quiet burr grinder for espresso and pour over coffee",
            "kitchen",
            4.6,
            199.0,
        ),
        (
            "Coffee beans",
            "single origin medium roast coffee beans with chocolate notes",
            "grocery",
            4.4,
            18.0,
        ),
        (
            "Insulated water bottle",
            "stainless steel bottle keeps water cold on long trail days",
            "outdoors",
            4.3,
            32.0,
        ),
        (
            "Camping lantern",
            "rechargeable lantern with warm light for tents and patios",
            "outdoors",
            4.2,
            45.0,
        ),
        (
            "Chef knife",
            "balanced stainless chef knife for precise kitchen prep",
            "kitchen",
            4.9,
            110.0,
        ),
        (
            "Travel backpack",
            "weather resistant backpack with laptop storage and hiking comfort",
            "travel",
            4.4,
            95.0,
        ),
    ];

    rows.into_iter()
        .enumerate()
        .map(
            |(idx, (name, description, category, rating, price))| NewProduct {
                name: name.to_string(),
                description: description.to_string(),
                category: category.to_string(),
                rating,
                price,
                embedding: Vector::from(fake_embedding(idx as u32 + 1, description)),
            },
        )
        .collect()
}

/// Deterministic fake embeddings for examples. They have no semantic model
/// behind them; they just make pgvector examples reproducible without an LLM
/// API key.
pub fn fake_embedding(seed: u32, text: &str) -> Vec<f32> {
    let mut hash = seed.wrapping_mul(0x9E37_79B9);
    for byte in text.bytes() {
        hash ^= byte as u32;
        hash = hash.rotate_left(5).wrapping_mul(0x85EB_CA6B);
    }

    let a = ((hash & 0xff) as f32) / 255.0;
    let b = (((hash >> 8) & 0xff) as f32) / 255.0;
    let c = (((hash >> 16) & 0xff) as f32) / 255.0;
    vec![a, b, c]
}

pub fn print_header(title: &str) {
    println!("\n{:=<72}", "");
    println!("{title}");
    println!("{:=<72}", "");
}
