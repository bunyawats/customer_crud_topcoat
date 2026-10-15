use customer_crud_topcoat::router;
use sqlx::postgres::PgPoolOptions;
use topcoat::asset::AssetBundle;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "customer_crud_topcoat=debug".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    // Written next to the executable by `topcoat dev` / `topcoat asset bundle`.
    let assets = AssetBundle::load()
        .expect("asset bundle not found - run via `topcoat dev`, or `topcoat asset bundle` first");

    // Binds to $HOST:$PORT (default 127.0.0.1:3000).
    tracing::info!("starting server");
    topcoat::start(router(pool, assets))
        .await
        .expect("server error");
}
