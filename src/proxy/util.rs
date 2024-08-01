use std::str::FromStr;

use crate::{Config, UrlEncodingAlgorithm};
use anyhow::Result;
use hyper::Uri;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
struct DecodeError;

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to decode the proxied origin from base32")
    }
}

#[derive(Error, Debug, Clone)]
struct InvalidHostError;

impl std::fmt::Display for InvalidHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "The public host field from the configuration is not a suffix of the origin"
        )
    }
}

#[derive(Error, Debug, Clone)]
struct InvalidOriginError;

impl std::fmt::Display for InvalidOriginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "The origin must contain a scheme, host and port")
    }
}

pub fn proxied_origin(config: &Config, origin: &str) -> Result<Origin> {
    let origin = match origin.rfind(':') {
        Some(index) => &origin[..index],
        None => origin,
    };

    let origin = origin
        .strip_suffix(&config.public_host)
        .ok_or(InvalidHostError)?
        .trim_end_matches('.');

    // Decode the proxied origin
    match &config.url_encoding_algorithm {
        UrlEncodingAlgorithm::Base32(alphabet) => parse_origin(&String::from_utf8(
            base32::decode(*alphabet, origin).ok_or(DecodeError)?,
        )?),
        UrlEncodingAlgorithm::Base32Xor(alphabet, key) => parse_origin(&String::from_utf8(
            base32::decode(*alphabet, origin)
                .ok_or(DecodeError)?
                .iter()
                .zip(key.iter().cycle())
                .map(|(byte, key_byte)| byte ^ key_byte)
                .collect(),
        )?),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// The scheme of the origin
    scheme: Scheme,
    /// The host of the origin
    host: String,
    /// The port of the origin
    port: u16,
}

impl Origin {
    pub fn scheme(&self) -> Scheme {
        self.scheme
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl From<Origin> for String {
    fn from(origin: Origin) -> String {
        match origin.scheme {
            Scheme::Http => format!("http://{}:{}", origin.host, origin.port),
            Scheme::Https => format!("https://{}:{}", origin.host, origin.port),
        }
    }
}

fn parse_origin(origin: &str) -> Result<Origin> {
    // Determine if string is empty
    if origin.is_empty() {
        return Err(InvalidOriginError.into());
    }

    let mut parts = origin.splitn(2, "://");
    let scheme = match parts.next() {
        Some("http") => Scheme::Http,
        Some("https") => Scheme::Https,
        _ => return Err(InvalidOriginError.into()),
    };

    let mut parts = parts.next().ok_or(InvalidOriginError)?.splitn(2, ':');
    let host = parts.next().ok_or(InvalidOriginError)?.to_string();
    if host.contains('/') || host.contains(':') {
        return Err(InvalidOriginError.into());
    }

    let port = parts
        .next()
        .map(|port| port.parse().map_err(|_| InvalidOriginError))
        .unwrap_or_else(|| match scheme {
            Scheme::Http => Ok(80),
            Scheme::Https => Ok(443),
        })?;

    Ok(Origin { scheme, host, port })
}

pub fn encode_url(config: &Config, url: &str) -> String {
    if !url.contains("://") {
        return url.to_string();
    }

    let uri = match Uri::from_str(url) {
        Ok(uri) => uri,
        Err(_) => return url.to_string(),
    };

    let scheme = match uri.scheme_str() {
        Some(str) => str,
        None => return url.to_string(),
    };

    let auth = match uri.authority() {
        Some(auth) => auth,
        None => return url.to_string(),
    };

    let port = match auth.port_u16() {
        Some(p) => format!(":{}", p),
        None => "".to_string(),
    };

    let origin = format!("{}://{}{}", scheme, auth.host(), port);

    let path = uri.path();

    return match config.url_encoding_algorithm.clone() {
        UrlEncodingAlgorithm::Base32(alphabet) => {
            let encoded_origin = base32::encode(alphabet, origin.as_bytes());
            format!("https://{}.{}{}", encoded_origin, config.public_host, path)
        }
        UrlEncodingAlgorithm::Base32Xor(alphabet, key) => {
            let encoded_origin = base32::encode(
                alphabet,
                &origin
                    .as_bytes()
                    .iter()
                    .zip(key.iter().cycle())
                    .map(|(byte, key_byte)| byte ^ key_byte)
                    .collect::<Vec<u8>>(),
            );
            format!("https://{}.{}{}", encoded_origin, config.public_host, path)
        }
    };
}
