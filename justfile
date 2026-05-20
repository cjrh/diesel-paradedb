# Run `just` with no arguments to see the available recipes.

# Connection URL the live integration tests use. Override on the command
# line (`just test-live database_url=...`) to point at an existing server.
database_url := "postgres://postgres:parade@localhost:5432/postgres"

# Name of the throwaway ParadeDB container managed by `db-up` / `db-down`.
container := "diesel-paradedb-test"

# List available recipes.
default:
    @just --list

# Unit tests plus live integration tests against a managed ParadeDB container.
test: test-unit db-up test-live

# Unit tests only — the JSON-shape checks that run without a database.
test-unit:
    cargo test --lib

# Live integration tests against a running server (run `just db-up` first).
test-live:
    TEST_DATABASE_URL={{database_url}} cargo test --test live_paradedb -- --ignored

# Start a ParadeDB container and block until it accepts connections.
db-up:
    docker start {{container}} 2>/dev/null || \
        docker run -d --name {{container}} -p 5432:5432 \
            -e POSTGRES_PASSWORD=parade \
            paradedb/paradedb:latest
    @echo "waiting for ParadeDB to accept connections..."
    @until docker exec {{container}} pg_isready -U postgres >/dev/null 2>&1; do sleep 1; done
    @echo "ParadeDB ready."

# Stop and remove the ParadeDB container created by `db-up`.
db-down:
    -docker rm -f {{container}}
