use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use hyper::Method;
use tower_http::cors::{Any, CorsLayer};

use crate::APIState;

use super::encode_url::post_encode;

pub fn service(state: Arc<APIState>) -> Router {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any)
        .allow_origin(Any);

    Router::new()
        .route("/", get(index))
        .route("/encode", post(post_encode))
        .layer(cors)
        .with_state(state)
}

async fn index(State(state): State<Arc<APIState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        format!("Hello, world! Configured host: {}", state.config.host),
    )
}
