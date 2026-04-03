use axum::{Router, middleware::from_fn_with_state, routing::post};
use dotenvy::dotenv;
use sqlx::Pool;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    api::{
        auth_mw::auth_mw,
        game::game_routes,
        game_tip::{protected_game_tip_routes, public_game_tip_routes},
        health::health_routes,
        system_log::log_routes,
        user::{auth0_trigger_endpoint, protected_auth_routes, public_auth_routes},
        webhook_mw::webhook_mw,
    },
    app_state::AppState,
    config::app_config::CONFIG,
    models::integration::INTEGRATION_NAMES,
};

mod api;
mod app_state;
mod config;
mod db;
mod models;
mod service;
mod tests;

#[tokio::main]
async fn main() {
    // Initialize .env
    dotenv().ok();

    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    // Connect once, run migrations, then hand the pool to AppState.
    // KeyVault queries the DB on init so migrations must run first.
    let pool = Pool::<sqlx::Postgres>::connect(&CONFIG.database_url)
        .await
        .unwrap_or_else(|e| panic!("Failed to connect to database: {}", e));
    if let Err(e) = sqlx::migrate!().run(&pool).await {
        error!("Failed to run migrations: {}", e);
        return;
    }

    let state = AppState::from_pool(pool)
        .await
        .unwrap_or_else(|e| panic!("{}", e));

    // Spawn cron jobs
    state.spawn_game_cleanup();
    state.spawn_round_pool_job();

    // Force static initialization of INTEGRATION_NAMES from config
    let _ = &*INTEGRATION_NAMES;

    let event_routes = Router::new()
        .route("/{pseudo_id}", post(auth0_trigger_endpoint))
        .layer(from_fn_with_state(state.clone(), webhook_mw))
        .with_state(state.clone());

    let public_routes = Router::new()
        .nest("/health", health_routes(state.clone()))
        .nest("/pseudo-users", public_auth_routes(state.clone()))
        .nest("/tips", public_game_tip_routes(state.clone()));

    let protected_routes = Router::new()
        .nest("/games", game_routes(state.clone()))
        .nest("/users", protected_auth_routes(state.clone()))
        .nest("/logs", log_routes(state.clone()))
        .nest("/tips", protected_game_tip_routes(state.clone()))
        .layer(from_fn_with_state(state.clone(), auth_mw));

    let app = Router::new()
        .merge(protected_routes)
        .merge(public_routes)
        .nest("/webhooks/auth0", event_routes);

    // Initialize webserver
    let listener =
        tokio::net::TcpListener::bind(format!("{}:{}", CONFIG.server.address, CONFIG.server.port))
            .await
            .unwrap();

    info!(
        "Server listening on address: {}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app).await.unwrap();
}
