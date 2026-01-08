use axum::{extract::State, routing::get, Json, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

use crate::discovery::{DiscoveredService, DiscoveryState};

pub async fn run_server(state: DiscoveryState, port: u16) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(list_services))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("HTTP Server listening on http://{}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn list_services(State(state): State<DiscoveryState>) -> Json<Vec<DiscoveredService>> {
    let services = state.get_services();
    Json(services)
}
