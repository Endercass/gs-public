use std::{net::SocketAddr, sync::Arc, usize};

use base32::Alphabet;
use serde::{Deserialize, Serialize};

use super::rewriting::html::html_rewriter;

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
    pub url_encoding_algorithm: UrlEncodingAlgorithm,
    /// The listen address for the proxy server, where all proxied hosts will point to
    pub host: SocketAddr,
    /// The public root domain to host the proxied hosts on, e.g. `example.com`
    pub public_host: String,
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
    pub config: Arc<Config>,
}

#[derive(Clone)]
/// The state that is passed to the proxy handler
pub struct ProxyState {
    pub config: Arc<Config>,
    pub client: reqwest::Client,
    pub html_rewriter: Arc<html_rewriter::HtmlRewriter>,
}

#[derive(Clone)]
/// The shared state that is passed to the hostname router
pub struct SharedState {
    pub config: Arc<Config>,
}
