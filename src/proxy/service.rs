use std::{sync::Arc, usize};

use crate::{error::Result, proxy::util::encode_url};
use axum::{
    body::{to_bytes, Body},
    debug_handler,
    extract::{
        ws::{CloseFrame, WebSocket},
        Host, Request, State, WebSocketUpgrade,
    },
    http::{HeaderName, HeaderValue},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use hyper::{
    header::{HOST, LOCATION},
    HeaderMap,
};
use reqwest_websocket::RequestBuilderExt;

use crate::ProxyState;

use super::util::{proxied_origin, Scheme};

#[debug_handler]
pub async fn proxy(
    ws: Option<WebSocketUpgrade>,
    State(state): State<Arc<ProxyState>>,
    Host(host): Host,
    req: Request,
) -> Result<impl IntoResponse> {
    let origin = proxied_origin(&state.config, &host)?;

    if let Some(ws) = ws {
        return Ok(ws.on_upgrade(move |socket| {
            proxy_ws(
                state.client.clone(),
                socket,
                format!(
                    "{}://{}{}{}",
                    match origin.scheme() {
                        Scheme::Http => "ws",
                        Scheme::Https => "wss",
                    },
                    origin.host(),
                    if origin.port() == 0 {
                        "".to_string()
                    } else {
                        format!(":{}", origin.port())
                    },
                    req.uri(),
                ),
            )
        }));
    }

    let (mut parts, body) = req.into_parts();

    let body_bytes: Vec<u8> = to_bytes(body, usize::MAX).await?.to_vec();

    parts
        .headers
        .insert(HOST, HeaderValue::from_str(origin.host())?);

    parts
        .headers
        .clone()
        .iter()
        .filter(|(name, _)| {
            name.as_str().starts_with("cf-")
                || matches!(name.as_str(), "referer" | "x-forwarded-for" | "cdn-loop")
        })
        .for_each(|(name, _)| {
            parts.headers.remove(name);
        });

    let origin_url: String = origin.into();

    let res = state
        .client
        .request(parts.method, format!("{}{}", origin_url, parts.uri))
        .headers(parts.headers)
        .body(body_bytes)
        .send()
        .await?;

    let mut response_builder = Response::builder().status(res.status().as_u16());

    let mut headers = HeaderMap::with_capacity(res.headers().len());
    headers.extend(
        res.headers()
            .into_iter()
            .filter(|(name, _)| {
                !matches!(
                    name.as_str(),
                    "cross-origin-embedder-policy"
                        | "cross-origin-opener-policy"
                        | "cross-origin-resource-policy"
                        | "content-security-policy"
                        | "content-security-policy-report-only"
                        | "expect-ct"
                        | "feature-policy"
                        | "origin-isolation"
                        | "strict-transport-security"
                        | "upgrade-insecure-requests"
                        | "x-content-type-options"
                        | "x-download-options"
                        | "x-frame-options"
                        | "x-permitted-cross-domain-policies"
                        | "x-powered-by"
                        | "x-xss-protection"
                )
            })
            .map(|(name, value)| {
                let name = HeaderName::from_bytes(name.as_ref()).unwrap();
                let mut value = HeaderValue::from_bytes(value.as_ref()).unwrap();

                if name == LOCATION {
                    let unproxied_location = value.to_str().unwrap();
                    let proxied_location = encode_url(&state.config, unproxied_location);
                    value = HeaderValue::from_str(&proxied_location).unwrap();
                }

                (name, value)
            }),
    );

    *response_builder.headers_mut().unwrap() = headers;

    Ok(response_builder
        .body(Body::from_stream(res.bytes_stream()))
        // This unwrap is fine because the body is empty here
        .unwrap()
        .into_response())
}

async fn proxy_ws(client: reqwest::Client, socket: WebSocket, dest: String) {
    if let Ok(res) = client.get(&dest).upgrade().send().await {
        if let Ok(dest_socket) = res.into_websocket().await {
            let (mut dest_tx, mut dest_rx) = dest_socket.split();

            let (mut tx, mut rx) = socket.split();

            let rx_to_dest = async {
                while let Some(msg) = rx.next().await {
                    if let Ok(msg) = msg {
                        match msg {
                            axum::extract::ws::Message::Text(text) => {
                                let dest_msg = reqwest_websocket::Message::Text(text);
                                let _ = dest_tx.send(dest_msg).await;
                            }
                            axum::extract::ws::Message::Binary(bin) => {
                                let dest_msg = reqwest_websocket::Message::Binary(bin);
                                let _ = dest_tx.send(dest_msg).await;
                            }
                            axum::extract::ws::Message::Close(close) => {
                                let close = close.unwrap_or(CloseFrame {
                                    code: 1000,
                                    reason: "Unknown Error".into(),
                                });
                                let dest_msg = reqwest_websocket::Message::Close {
                                    code: close.code.into(),
                                    reason: close.reason.to_string(),
                                };
                                let _ = dest_tx.send(dest_msg).await;
                                break;
                            }
                            axum::extract::ws::Message::Ping(data) => {
                                let dest_msg = reqwest_websocket::Message::Ping(data);
                                let _ = dest_tx.send(dest_msg).await;
                            }
                            axum::extract::ws::Message::Pong(data) => {
                                let dest_msg = reqwest_websocket::Message::Pong(data);
                                let _ = dest_tx.send(dest_msg).await;
                            }
                        }
                    }
                }
            };

            let tx_to_src = async {
                while let Some(msg) = dest_rx.next().await {
                    if let Ok(msg) = msg {
                        match msg {
                            reqwest_websocket::Message::Text(text) => {
                                let src_msg = axum::extract::ws::Message::from(text);
                                let _ = tx.send(src_msg).await;
                            }
                            reqwest_websocket::Message::Binary(bin) => {
                                let src_msg = axum::extract::ws::Message::from(bin);
                                let _ = tx.send(src_msg).await;
                            }
                            reqwest_websocket::Message::Close { code, reason } => {
                                let src_msg = axum::extract::ws::Message::Close(Some(CloseFrame {
                                    code: code.into(),
                                    reason: reason.into(),
                                }));
                                let _ = tx.send(src_msg).await;
                                break;
                            }
                            reqwest_websocket::Message::Ping(data) => {
                                let src_msg = axum::extract::ws::Message::Ping(data);
                                let _ = tx.send(src_msg).await;
                            }
                            reqwest_websocket::Message::Pong(data) => {
                                let src_msg = axum::extract::ws::Message::Pong(data);
                                let _ = tx.send(src_msg).await;
                            }
                        }
                    }
                }
            };

            tokio::select! {
                _ = rx_to_dest => {}
                _ = tx_to_src => {}
            }
        }
    }
}
