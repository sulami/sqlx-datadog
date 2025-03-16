# SQLx-Datadog

This crate provides a drop-in replacement for `tracing::instrument` meant for
instrumenting [SQLx](https://docs.rs/sqlx/latest/sqlx/) queries with 
[tracing](https://docs.rs/tracing/latest/tracing/) for use with 
[tracing-opentelemetry](https://docs.rs/tracing-opentelemetry/latest/tracing_opentelemetry).

It automatically injects span tags for Datadog to correctly identify the 
span as a SQL query and set relevant attributes.

This is what it looks like in action:

```rust
use sqlx::Execute;
use sqlx_datadog::instrument_query;

#[derive(Debug, sqlx::FromRow)]
struct User { name: String, email: String }

#[instrument_query(skip(db))]
async fn fetch_user(db: &sqlx::MySqlPool, user_id: i64) -> Result<User, sqlx::Error> {
    let query = sqlx::query_as("SELECT name, email FROM users WHERE id = ? LIMIT 1");
    tracing::Span::current().record("db.statement", query.sql().trim());
    query.bind(user_id).fetch_one(db).await
}
```

### Current Limitations

For the time being, it still requires manually setting the query text as 
shown in the example, and it does not inject the distributed tracing comment 
into the query yet.

It probably does not work with SQLite, as SQLite's `ConnectOptions` are 
quite different from both MySQL's and Postgres'.
