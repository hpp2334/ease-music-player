use std::net::SocketAddr;

use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue},
    response::IntoResponse,
    Router,
};

#[axum::debug_handler]
async fn handle_music_download() -> impl IntoResponse {
    let handle_pod = accessor.get();
    if handle_pod.is_none() {
        return Err(crate::modules::error::EaseError::ClientDestroyed);
    }
    let handle = handle_pod.unwrap();
    let handle = handle.handle();

    let state = CurrentServerState::map(handle, |v| v.clone());
    let current_music = state.current_music.unwrap();
    let music_id = current_music.id();
    let stream_file = load_music_data(handle, current_music).await;
    if let Err(e) = stream_file {
        tracing::error!(
            "[handle_music_download] load music {:?} error, {}",
            music_id,
            e
        );
        return Err(e);
    }
    let stream_file = stream_file.unwrap();

    let file_name = stream_file.url().to_string();
    let file_name = file_name.split('/').last().unwrap();

    let mut headers = HeaderMap::new();
    headers.append(
        header::CONTENT_TYPE,
        HeaderValue::from_str(
            stream_file
                .content_type()
                .clone()
                .unwrap_or("application/octet-stream"),
        )
        .unwrap(),
    );
    headers.append(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", file_name)).unwrap(),
    );

    let body = axum::body::StreamBody::new(stream_file.into_stream());
    return Ok((headers, body));
}

pub fn start_server() -> u16 {
    let router_svc = Router::new()
        .route("/music/:id", axum::routing::get(handle_music_download))
        .into_make_service();

    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let incomming = axum::Server::bind(&addr)
        .http1_max_buf_size(20_000_000) // ~20MB
        .serve(router_svc);

    let port = incomming.local_addr().port();

    tokio::spawn(async move {
        let _ = incomming.await.unwrap();
    });
    tracing::info!("setup a local server on {}", port);

    port
}
