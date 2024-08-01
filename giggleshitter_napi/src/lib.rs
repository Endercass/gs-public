use std::sync::Arc;

use base32::Alphabet;
use giggleshitter_common::state::{Config, UrlEncodingAlgorithm};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use scorched::{LogExpect, LogImportance};
use tokio::sync::oneshot::{Receiver, Sender};

#[napi]
#[derive(Debug)]
pub enum AlphabetNapi {
    Crockford,
    Rfc4648,
    Rfc4648Lower,
    Rfc4648Hex,
    Rfc4648HexLower,
    Z,
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct EncoderOptions {
    pub alphabet: Option<AlphabetNapi>,
    pub key: Option<Vec<u8>>,
}

impl Default for EncoderOptions {
    fn default() -> Self {
        Self {
            alphabet: Some(AlphabetNapi::Z),
            key: None,
        }
    }
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct ServeConfig {
    pub host: Option<String>,
    pub public_host: Option<String>,
    pub encoder: Option<EncoderOptions>,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            host: Some("0.0.0.0:3069".to_string()),
            public_host: Some("changeme.local".to_string()),
            encoder: Some(EncoderOptions::default()),
        }
    }
}

impl ServeConfig {
    // Set all none values to default values
    fn set_defaults(&mut self) {
        let default = ServeConfig::default();

        if self.host.is_none() {
            self.host = default.host;
        }

        if self.public_host.is_none() {
            self.public_host = default.public_host;
        }

        if self.encoder.is_none() {
            self.encoder = default.encoder;
        } else if self.encoder.as_ref().unwrap().alphabet.is_none() {
            self.encoder.as_mut().unwrap().alphabet = default.encoder.unwrap().alphabet;
        }
    }
}

impl From<ServeConfig> for Config {
    fn from(config: ServeConfig) -> Self {
        let encoder = config.encoder.unwrap();
        // KMS
        let url_encoding_algorithm = match (encoder.alphabet.unwrap(), encoder.key) {
            (AlphabetNapi::Crockford, None) => UrlEncodingAlgorithm::Base32(Alphabet::Crockford),
            (AlphabetNapi::Rfc4648, None) => {
                UrlEncodingAlgorithm::Base32(Alphabet::Rfc4648 { padding: false })
            }
            (AlphabetNapi::Rfc4648Lower, None) => {
                UrlEncodingAlgorithm::Base32(Alphabet::Rfc4648Lower { padding: false })
            }
            (AlphabetNapi::Rfc4648Hex, None) => {
                UrlEncodingAlgorithm::Base32(Alphabet::Rfc4648Hex { padding: false })
            }
            (AlphabetNapi::Rfc4648HexLower, None) => {
                UrlEncodingAlgorithm::Base32(Alphabet::Rfc4648HexLower { padding: false })
            }
            (AlphabetNapi::Z, None) => UrlEncodingAlgorithm::Base32(Alphabet::Z),

            (AlphabetNapi::Crockford, Some(key)) => {
                UrlEncodingAlgorithm::Base32Xor(Alphabet::Crockford, key)
            }
            (AlphabetNapi::Rfc4648, Some(key)) => {
                UrlEncodingAlgorithm::Base32Xor(Alphabet::Rfc4648 { padding: false }, key)
            }
            (AlphabetNapi::Rfc4648Lower, Some(key)) => {
                UrlEncodingAlgorithm::Base32Xor(Alphabet::Rfc4648Lower { padding: false }, key)
            }
            (AlphabetNapi::Rfc4648Hex, Some(key)) => {
                UrlEncodingAlgorithm::Base32Xor(Alphabet::Rfc4648Hex { padding: false }, key)
            }
            (AlphabetNapi::Rfc4648HexLower, Some(key)) => {
                UrlEncodingAlgorithm::Base32Xor(Alphabet::Rfc4648HexLower { padding: false }, key)
            }
            (AlphabetNapi::Z, Some(key)) => UrlEncodingAlgorithm::Base32Xor(Alphabet::Z, key),
        };

        Config {
            url_encoding_algorithm,
            host: config.host.unwrap().parse().unwrap(),
            public_host: config.public_host.unwrap(),
        }
    }
}

#[napi]
pub struct App {
    pub config: ServeConfig,
    channel: (Option<Sender<()>>, Option<Receiver<()>>),
}

#[napi]
impl App {
    #[napi(constructor)]
    pub fn new(config: Option<ServeConfig>) -> Result<Self> {
        let channel = {
            let channel = tokio::sync::oneshot::channel::<()>();
            (Some(channel.0), Some(channel.1))
        };

        let mut config = config.unwrap_or_default();

        config.set_defaults();

        Ok(Self { config, channel })
    }

    #[napi]
    /// Close the server
    /// # Safety
    /// This function is marked as unsafe because of a limitation in the napi crate.
    pub async unsafe fn close(&mut self) -> Result<()> {
        match self.channel.0.take() {
            Some(rx) => {
                let _ = rx.send(());
            }
            None => {
                return Err(Error::new(
                    Status::GenericFailure,
                    "Server is already closed",
                ));
            }
        }

        Ok(())
    }

    #[napi]
    /// Start the server
    /// # Safety
    /// This function is marked as unsafe because of a limitation in the napi crate.
    pub async unsafe fn serve(&mut self) -> Result<()> {
        let _ = tracing_subscriber::fmt::try_init();

        let config = self.config.clone();

        let rx = match self.channel.1.take() {
            Some(rx) => rx,
            None => {
                return Err(Error::new(
                    Status::GenericFailure,
                    "Server is already running",
                ));
            }
        };

        let server_handle = tokio::spawn(async move {
            giggleshitter_common::serve(Arc::new(config.into()), async {
                rx.await.ok();
            })
            .await
            .log_expect(LogImportance::Error, "Failed to start server")
        });

        let _ = server_handle.await;

        Ok(())
    }
}
