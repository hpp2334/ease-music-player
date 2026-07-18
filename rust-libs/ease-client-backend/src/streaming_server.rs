//! Local HTTP server that streams music assets to the desktop JavaFX
//! MediaPlayer. Replaces the previous Kotlin `StreamingHttpServer` and
//! keeps every byte inside the Rust tokio runtime (no per-chunk JNA
//! crossings, no `runBlocking` on HTTP-handler threads).
//!
//! Only one route is exposed: `GET /music/:id`. Range requests are honored
//! so the player can seek.

use axum::{
    body::StreamBody,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use ease_client_schema::{DataSourceKey, MusicId};
use ease_client_tokio::tokio_runtime;
use futures_util::StreamExt;

use crate::ctx::WeakBackendContext;
use crate::services::get_asset_file;

/// A handle to the running streaming HTTP server. Dropping this initiates
/// graceful shutdown.
pub struct StreamingServer {
    base_url: String,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl StreamingServer {
    /// Bind a TCP listener on `127.0.0.1:0` (OS-assigned port) and spawn
    /// the axum server on the shared tokio runtime. Returns immediately
    /// after the listener is bound so callers can read `base_url()`.
    pub fn start(weak_cx: WeakBackendContext) -> Self {
        let rt = tokio_runtime();

        // Bind synchronously so we have the port before returning.
        let std_listener = rt
            .block_on(async {
                tokio::net::TcpListener::bind("127.0.0.1:0").await
            })
            .expect("streaming server: failed to bind TCP listener")
            .into_std()
            .expect("streaming server: failed to convert listener");
        std_listener
            .set_nonblocking(true)
            .expect("streaming server: set_nonblocking failed");
        let port = std_listener
            .local_addr()
            .expect("streaming server: no local_addr")
            .port();
        let base_url = format!("http://127.0.0.1:{}", port);

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let announced_url = base_url.clone();

        rt.spawn(async move {
            let app = Router::new()
                .route("/music/:id", get(handle_music))
                .with_state(weak_cx);

            let server = match axum::Server::from_tcp(std_listener) {
                Ok(s) => s.serve(app.into_make_service()),
                Err(e) => {
                    tracing::error!("streaming server: failed to start: {:?}", e);
                    return;
                }
            };

            tracing::info!("streaming server listening on {}", announced_url);

            let server = server.with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            });

            if let Err(e) = server.await {
                tracing::error!("streaming server: {:?}", e);
            }
            tracing::info!("streaming server stopped");
        });

        Self {
            base_url,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Drop for StreamingServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

async fn handle_music(
    State(weak_cx): State<WeakBackendContext>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let Some(cx) = weak_cx.upgrade() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "backend dropped").into_response();
    };

    let range_header = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let range_start = parse_range_start(range_header.as_deref());

    let file = match get_asset_file(&cx, DataSourceKey::Music { id: MusicId::wrap(id) }, range_start).await
    {
        Ok(Some(f)) => f,
        Ok(None) => return (StatusCode::NOT_FOUND, "asset not found").into_response(),
        Err(e) => {
            tracing::error!("streaming: get_asset_file failed: {:?}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    // `total_size` is the full resource size (used for Content-Range).
    // `size()` already accounts for `byte_offset` (used for Content-Length
    // when there is no Range header).
    let total_size = file.total_size();
    let content_type = file
        .content_type()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let adjusted_size = file.size();

    let body_stream = file.into_rx().map(|res| match res {
        Ok(bytes) => Ok::<_, std::io::Error>(bytes),
        Err(e) => Err(std::io::Error::other(format!("stream error: {:?}", e))),
    });
    let body = StreamBody::new(body_stream);

    let mut out_headers = HeaderMap::new();
    out_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    out_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type).unwrap_or_else(|_| {
            HeaderValue::from_static("application/octet-stream")
        }),
    );

    if range_header.is_some() {
        if let Some(total) = total_size {
            let start = range_start;
            let end = total.saturating_sub(1) as u64;
            let content_length = total.saturating_sub(range_start as usize) as u64;
            if let Ok(val) = HeaderValue::from_str(&format!("bytes {}-{}/{}", start, end, total)) {
                out_headers.insert(header::CONTENT_RANGE, val);
            }
            if let Ok(val) = HeaderValue::from_str(&content_length.to_string()) {
                out_headers.insert(header::CONTENT_LENGTH, val);
            }
            return (StatusCode::PARTIAL_CONTENT, out_headers, body).into_response();
        }
    }

    if let Some(adjusted) = adjusted_size {
        if let Ok(val) = HeaderValue::from_str(&adjusted.to_string()) {
            out_headers.insert(header::CONTENT_LENGTH, val);
        }
    }
    (StatusCode::OK, out_headers, body).into_response()
}

/// Parse the start byte from an HTTP `Range: bytes=N-` header.
fn parse_range_start(range: Option<&str>) -> u64 {
    let Some(range) = range else { return 0 };
    let Some(rest) = range.strip_prefix("bytes=") else { return 0 };
    let Some((start_str, _)) = rest.split_once('-') else { return 0 };
    start_str.trim().parse().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_range() {
        assert_eq!(parse_range_start(None), 0);
        assert_eq!(parse_range_start(Some("bytes=0-")), 0);
        assert_eq!(parse_range_start(Some("bytes=1024-")), 1024);
        assert_eq!(parse_range_start(Some("bytes=1024-2047")), 1024);
        assert_eq!(parse_range_start(Some("bytes=  50 -")), 50);
        assert_eq!(parse_range_start(Some("nonsense")), 0);
        assert_eq!(parse_range_start(Some("bytes=-500")), 0);
    }
}
