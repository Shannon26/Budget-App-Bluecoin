mod app;
mod assistant;
mod csv;
mod date;
mod error;
mod models;
mod routes;
mod store;

#[cfg(test)]
mod tests;

#[cfg(test)]
use app::build_router;
use app::{build_router_with, IdentityConfig};
use assistant::AssistantClient;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;

/// Load a small allowlist from `.env` in the backend working directory.
/// Values already present in the process environment win, and no other file
/// content is interpreted. The file itself is Git-ignored.
fn load_dotenv() {
    let Ok(contents) = std::fs::read_to_string(".env") else {
        return;
    };
    let allowed = [
        "OPENROUTER_API_KEY",
        "OPENROUTER_MODEL",
        "OPENROUTER_URL",
        "PAISA_ASSISTANT_MODE",
        "PAISA_HOUSEHOLD_ENABLED",
        "PAISA_MEMBER_TOKEN",
        "PAISA_DEFAULT_MEMBER_ID",
    ];
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !allowed.contains(&key) || std::env::var_os(key).is_some() {
            continue;
        }
        let value = value.trim().trim_matches('"').trim_matches('\'');
        std::env::set_var(key, value);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,budget_backend=debug,tower_http=warn".into()),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://budget.db?mode=rwc".to_string());
    let options = SqliteConnectOptions::from_str(&database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let app = build_router_with(
        pool,
        IdentityConfig::from_env(),
        AssistantClient::from_env(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    tracing::info!("budget API listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await?;
    Ok(())
}
