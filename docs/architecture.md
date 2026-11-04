# Service structure

Auth (`services/auth`) and runtime (`services/runtime`) use these layers:

- `route.rs` defines HTTP methods, paths, and nested routers.
- `handler.rs` extracts requests, calls services, and builds HTTP responses. Runtime uses `handlers/mod.rs` with handlers split by feature under `handlers/`.
- `services.rs` owns business logic and calls repositories or other services.
- `repository.rs` checks out database connections and owns SQL and row mapping for the domain.

The builder package is an internal service with `services.rs` and `repository.rs`; its HTTP endpoints are part of the runtime service. The web service owns page routes, handlers, state, templates, and presentation data. It has no persistence repository.

`apps/api` initializes dependencies and composes service routers. Its `state.rs` supplies service-specific Axum state through `FromRef`; the web service handles the dashboard, login and setup pages, create-project page, and favicon. The proxy remains in `proxy.rs`.

`packages/db` owns connection pooling, schema creation (`schema.rs`), and upgrades (`migration.rs`). It exposes `Database::connection()` for repositories. Add domain queries to the relevant repository rather than to `Database` or a handler.

Request flow: route → handler → services → repository → pooled connection.

The refactor preserves HTTP paths and SQLite table layouts. Database operations remain synchronous.

## Tests

Keep focused unit tests beside their implementation under `#[cfg(test)]`. Public repository behavior is tested in each owning crate's `tests/repository.rs` with isolated SQLite databases. HTTP behavior is tested in `apps/api/tests/routes.rs` through the composed router, including response bodies and persistence effects.

API fixtures live in `apps/api/tests/common/mod.rs` and build an explicit configuration without reading environment variables. Test applications disable automatic process startup. Database failure tests use constraints or triggers on their isolated database rather than mocking repository internals.

Run the complete suite with `cargo test --workspace`, or just the HTTP integration tests with `cargo test -p api --test routes`.

## Administrator access

`services/auth/src/admin/` follows the same route, handler, services, and repository split. Its tables are created by the database migration. The API access middleware routes deployed application domains to the proxy, then requires an administrator session for control pages and APIs. Setup and login endpoints, their pages, the favicon, and the health check are public.

Initial setup requires a random token stored in the server data directory with owner-only permissions on Unix. A singleton database key makes administrator creation one-time. Passwords use Argon2id, while session tokens are random and stored as SHA-256 digests with a 12-hour expiry. Password hashing runs outside the async worker threads. Login attempts are limited, cookies are HttpOnly and SameSite=Lax, and browser mutations check their origin. Enable Secure cookies when using HTTPS.

GitHub credentials use a separate cookie and are never inferred from the most recently stored session.
