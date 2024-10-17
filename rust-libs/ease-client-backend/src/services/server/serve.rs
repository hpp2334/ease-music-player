use std::{collections::HashMap, net::SocketAddr};

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use ease_client_shared::backends::{
    music::MusicId,
    storage::{StorageEntryLoc, StorageId},
};
use ease_remote_storage::StreamFile;

use crate::{
    ctx::BackendContext,
    services::{music::get_music_storage_entry_loc, storage::get_storage_backend},
};

async fn get_stream_file_by_loc(
    cx: &BackendContext,
    loc: StorageEntryLoc,
) -> anyhow::Result<Option<StreamFile>> {
    let backend = get_storage_backend(&cx, loc.storage_id)?;
    if backend.is_none() {
        return Ok(None);
    }
    let backend = backend.unwrap();
    let stream_file = backend.get(&loc.path).await?;
    Ok(Some(stream_file))
}

async fn get_stream_file_by_music_id(
    cx: &BackendContext,
    id: MusicId,
) -> anyhow::Result<Option<StreamFile>> {
    let loc = get_music_storage_entry_loc(&cx, id)?;
    if loc.is_none() {
        return Ok(None);
    }
    let loc = loc.unwrap();
    get_stream_file_by_loc(cx, loc).await
}

async fn handle_got_stream_file(res: anyhow::Result<Option<StreamFile>>) -> Response {
    if let Err(e) = res {
        tracing::error!("{}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let res = res.unwrap();
    if res.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let stream_file = res.unwrap();
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
    if let Some(size) = stream_file.size() {
        headers.append(header::CONTENT_LENGTH, HeaderValue::from(size));
    }
    headers.append(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", file_name)).unwrap(),
    );

    let body = axum::body::StreamBody::new(stream_file.into_stream());
    return (headers, body).into_response();
}

#[axum::debug_handler]
async fn handle_music_download(State(cx): State<BackendContext>, Path(id): Path<i64>) -> Response {
    let id = MusicId::wrap(id);
    let res = get_stream_file_by_music_id(&cx, id).await;
    handle_got_stream_file(res).await
}

async fn handle_asset_download(
    State(cx): State<BackendContext>,
    Path(id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let p = params.get("sp").unwrap();
    let p = URL_SAFE.encode(p);
    let id = StorageId::wrap(id);
    let res = get_stream_file_by_loc(
        &cx,
        StorageEntryLoc {
            path: p,
            storage_id: id,
        },
    )
    .await;
    handle_got_stream_file(res).await
}

pub fn start_server(cx: &BackendContext) -> u16 {
    let router_svc = Router::new()
        .route("/music/:id", axum::routing::get(handle_music_download))
        .route("/asset/:id", axum::routing::get(handle_asset_download))
        .with_state(cx.clone())
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
