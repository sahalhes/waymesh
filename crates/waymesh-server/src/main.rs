use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use waymesh_server::{create_router, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "waymesh_server=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let state = if let Ok(database_url) = std::env::var("DATABASE_URL") {
        if !database_url.trim().is_empty() {
            match AppState::from_database(&database_url).await {
                Ok(s) => {
                    tracing::info!("Connected to PostgreSQL/PostGIS at {}", database_url);
                    s
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to connect to database ({}), falling back to in-memory: {}",
                        database_url,
                        e
                    );
                    AppState::new()
                }
            }
        } else {
            AppState::new()
        }
    } else {
        tracing::info!("DATABASE_URL not set; running in pure in-memory mode");
        AppState::new()
    };

    let app = create_router(state);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()
        .unwrap_or(8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Waymesh Node listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
