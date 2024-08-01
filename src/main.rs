pub mod api;
pub mod error;
pub mod proxy;
pub mod rewriting;

use std::{net::SocketAddr, sync::Arc, usize};

use axum::{
    extract::{Host, Request, State},
    handler::Handler,
    routing::any,
};
use base32::Alphabet;
use error::Result;
use reqwest::redirect::Policy;
use rewriting::html::html_rewriter;
use scorched::{logf, LogData, LogImportance};
use serde::{Deserialize, Serialize};
use tower::ServiceExt;

const fn default_padding() -> bool {
    false
}

#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(remote = "Alphabet")]
pub enum AlphabetDef {
    Crockford,
    Rfc4648 {
        #[serde(default = "default_padding")]
        padding: bool,
    },
    Rfc4648Lower {
        #[serde(default = "default_padding")]
        padding: bool,
    },
    Rfc4648Hex {
        #[serde(default = "default_padding")]
        padding: bool,
    },
    Rfc4648HexLower {
        #[serde(default = "default_padding")]
        padding: bool,
    },
    Z,
}

#[derive(Clone, Serialize, Deserialize)]
/// The algorithm to encode the origin of the proxied host. The output must be a valid subdomain,
/// so it must only contain alphanumeric characters and hyphens.
pub enum UrlEncodingAlgorithm {
    /// Encode the origin as a base32 string.
    Base32(
        #[serde(rename = "alphabet")]
        #[serde(with = "AlphabetDef")]
        Alphabet,
    ),
    /// XOR the origin with the given key then encode it as a base32 string.
    Base32Xor(
        #[serde(rename = "alphabet")]
        #[serde(with = "AlphabetDef")]
        Alphabet,
        #[serde(rename = "key")] Vec<u8>,
    ),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    /// The algorithm to encode the origin of the proxied host
    url_encoding_algorithm: UrlEncodingAlgorithm,
    /// The listen address for the proxy server, where all proxied hosts will point to
    host: SocketAddr,
    /// The public root domain to host the proxied hosts on, e.g. `example.com`
    public_host: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            url_encoding_algorithm: UrlEncodingAlgorithm::Base32(Alphabet::Z),
            host: SocketAddr::from(([0, 0, 0, 0], 3069)),
            public_host: "changeme.local".to_string(),
        }
    }
}

#[derive(Clone)]
/// The state that is passed to frontend routes
pub struct APIState {
    config: Arc<Config>,
}

#[derive(Clone)]
/// The state that is passed to the proxy handler
pub struct ProxyState {
    config: Arc<Config>,
    client: reqwest::Client,
    html_rewriter: Arc<html_rewriter::HtmlRewriter>,
}

#[derive(Clone)]
/// The shared state that is passed to the hostname router
pub struct SharedState {
    config: Arc<Config>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    logf!(
        Info,
        "Loading config from file: {}",
        confy::get_configuration_file_path("weirdproxy", None)?.display()
    );

    let config: Arc<Config> = Arc::new(confy::load("weirdproxy", None)?);

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

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(config.host).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
