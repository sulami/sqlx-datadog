# SQLx-Datadog

This crate provides a drop-in replacement for `tracing::instrument` meant for
instrumenting [SQLx](https://docs.rs/sqlx/latest/sqlx/) queries with 
[tracing](https://docs.rs/tracing/latest/tracing/) for use with 
[tracing-datadog](https://docs.rs/tracing-datadog).

It automatically injects span tags for Datadog to correctly identify the 
span as a SQL query and set relevant attributes.

This is what it looks like in action:

```rust
use sqlx_datadog::instrument_query;

#[derive(Debug, sqlx::FromRow)]
struct User { name: String, email: String }

#[instrument_query(skip(db))]
async fn fetch_user(db: &sqlx::MySqlPool, user_id: i64) -> Result<User, sqlx::Error> {
    let query = "SELECT name, email FROM users WHERE id = ? LIMIT 1";
    sqlx::query_as(query).bind(user_id).fetch_one(db).await
}
```

### Current Limitations

It probably does not work with SQLite, as SQLite's `ConnectOptions` are 
quite different from both MySQL's and Postgres'.
