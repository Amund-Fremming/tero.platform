use std::collections::HashMap;

use axum::{Router, middleware::from_fn_with_state, routing::post};
use dotenvy::dotenv;
use models::app_state::AppState;
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
    config::config::CONFIG,
    models::{
        error::ServerError,
        integration::{INTEGRATION_NAMES, IntegrationName},
    },
};

mod api;
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

    // Initialize state
    let state = AppState::from_connection_string(&CONFIG.database_url)
        .await
        .unwrap_or_else(|e| panic!("{}", e));

    // Spawn cron jobs
    state.spawn_game_cleanup();

    // Initiate integrations
    if let Err(e) = load_integrations().await {
        error!("Failed to load integrations: {}", e);
        return;
    }

    // Run migrations
    if let Err(e) = sqlx::migrate!().run(state.get_pool()).await {
        error!("Failed to run migrations: {}", e);
        return;
    }

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

async fn load_integrations() -> Result<(), ServerError> {
    let integrations = &CONFIG.integrations;

    let integration_names: HashMap<String, IntegrationName> = integrations
        .iter()
        .map(|i| (i.subject.clone(), i.name.clone()))
        .collect();

    {
        let mut lock = INTEGRATION_NAMES.lock().await;
        *lock = integration_names;
    }

    Ok(())
}
