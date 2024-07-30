use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};

use crate::APIState;

use super::encode_url::post_encode;

pub fn service(state: Arc<APIState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/encode", post(post_encode))
        .with_state(state)
}

async fn index(State(state): State<Arc<APIState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        format!("Hello, world! Configured host: {}", state.config.host),
    )
}
