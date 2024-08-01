use std::sync::Arc;

use axum::debug_handler;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::proxy::util::encode_url;
use crate::APIState;

#[derive(Deserialize)]
pub struct EncodeUrlRequest {
    pub url: String,
}

#[derive(Serialize)]
pub struct EncodeUrlResponse {
    pub encoded_url: String,
}

#[debug_handler]
pub async fn post_encode(
    State(state): State<Arc<APIState>>,
    Json(EncodeUrlRequest { url }): Json<EncodeUrlRequest>,
) -> Result<Json<EncodeUrlResponse>> {
    Ok(Json(EncodeUrlResponse {
        encoded_url: encode_url(&state.config, &url),
    }))
}
