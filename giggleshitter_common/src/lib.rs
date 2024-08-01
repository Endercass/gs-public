pub mod api;
pub mod error;
pub mod proxy;
pub mod rewriting;
pub mod state;

use std::{future::Future, sync::Arc};

use axum::{
    extract::{Host, Request, State},
    handler::Handler,
    routing::any,
};
use error::Result;
use reqwest::redirect::Policy;
use rewriting::html::html_rewriter;
use state::{APIState, Config, ProxyState, SharedState};
use tower::ServiceExt;

pub async fn serve<F>(config: Arc<Config>, graceful_shutdown: F) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let sharedstate = SharedState {
        config: config.clone(),
    };

    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .gzip(true)
        .brotli(true)
        .deflate(true)
        .zstd(true)
        .build()?;

    let proxystate = ProxyState {
        config: config.clone(),
        client,
        html_rewriter: Arc::new(html_rewriter::HtmlRewriter::new(Arc::new(
            sharedstate.clone(),
        ))),
    };

    let proxyrouter = proxy::service::proxy.with_state(Arc::new(proxystate).clone());

    let apistate = APIState {
        config: config.clone(),
    };

    let apirouter = api::service::service(Arc::new(apistate));

    let app = any(
        |State(state): State<SharedState>, Host(host): Host, req: Request| async move {
            if host == format!("api.{}", state.config.public_host) {
                return apirouter.oneshot(req).await;
            }
            proxyrouter.oneshot(req).await
        },
    )
    .with_state(sharedstate);

    let listener = tokio::net::TcpListener::bind(config.host).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(graceful_shutdown)
        .await
        .unwrap();

    Ok(())
}
