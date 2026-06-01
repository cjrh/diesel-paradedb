# Run `just` with no arguments to see the available recipes.

# Connection URL used only when you want to bypass testcontainers and point the
# live tests/examples at an existing ParadeDB server.
database_url := "postgres://postgres:parade@localhost:5432/postgres"

# Name of the optional debug ParadeDB container managed by `db-up` / `db-down`.
container := "diesel-paradedb-test"

# List available recipes.
default:
    @just --list

# Unit tests, live integration tests, and example compilation.
test: test-unit test-live check-examples

# Unit tests only — no database or Docker needed.
test-unit:
    cargo test --lib

# Live integration tests with testcontainers-managed ParadeDB.
test-live:
    cargo test --test live_paradedb -- --ignored --nocapture

# Live integration tests against an existing server.
test-live-external:
    TEST_DATABASE_URL={{database_url}} cargo test --test live_paradedb -- --ignored --nocapture

# Compile every example without starting containers.
check-examples:
    cargo check --examples

# Run every example. Each example starts its own ParadeDB container unless
# TEST_DATABASE_URL is set in the environment.
examples: check-examples
    cargo run --example quickstart
    cargo run --example faceted_search
    cargo run --example autocomplete
    cargo run --example more_like_this
    cargo run --example hybrid_rrf
    cargo run --example rag

# Run one example by name, e.g. `just example quickstart`.
example name:
    cargo run --example {{name}}

# Optional manual debug server. Normal tests/examples use testcontainers instead.
db-up:
    docker start {{container}} 2>/dev/null || \
        docker run -d --name {{container}} -p 5432:5432 \
            -e POSTGRES_PASSWORD=parade \
            paradedb/paradedb:latest
    @echo "waiting for ParadeDB to accept connections..."
    @until docker exec {{container}} pg_isready -U postgres >/dev/null 2>&1; do sleep 1; done
    @echo "ParadeDB ready at {{database_url}}."

# Stop and remove the optional debug container created by `db-up`.
db-down:
    -docker rm -f {{container}}
