//! Method dispatcher.
//!
//! [`dispatch`] takes a [`BridgeRequest`] + optional input buffers, calls
//! the underlying `ct_*` / `cts_*` controller function, and returns a
//! (response_json, output_buffers) pair. The JNI layer wraps this in the
//! final envelope and constructs the `BridgeResult` POJO.

#![allow(dead_code)]

use std::sync::Arc;

use ease_client_schema::{
    MusicId, PlayMode, PlaylistId, PluginId, PluginStorageId, StorageEntryLoc, StorageHandle,
    StorageId,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    bridge::{
        handle_table::{get_backend, get_player, get_player_context, register, HandleEntry},
        request::BridgeRequest,
    },
    controllers::{
        asset::ct_get_asset,
        debug::{cts_list_log_files, cts_trigger_error, cts_trigger_panic},
        music::{ct_get_music, ct_update_music_lyric},
        playlist::{
            ct_add_musics_to_playlist, ct_create_playlist, ct_get_playlist, ct_list_playlist,
            ct_remove_music_from_playlist, ct_remove_playlist, ct_update_playlist,
            reorder_music_in_playlist_inner, reorder_playlist_inner, ArgReorderMusic,
            ArgReorderPlaylist,
        },
        storage::{ct_list_storage, ct_list_storage_entry_children, ct_remove_storage},
    },
    error::{BError, BResult},
    objects::music::{MetadataRecord, PlayerStateRecord},
    objects::player::{
        ct_player_context_new, ct_player_duration_ms, ct_player_load_music, ct_player_new,
        ct_player_pause, ct_player_play, ct_player_position_ms, ct_player_probe_duration_ms,
        ct_player_seek, ct_player_set_volume, ct_player_state, ct_player_stop,
    },
    services::{
        app::ArgInitializeApp,
        music::{
            get_music_abstract, update_music_cover, update_music_duration, ArgAddMusicsToPlaylist,
            ArgCreatePlaylist, ArgRemoveMusicFromPlaylist, ArgUpdateMusicCover,
            ArgUpdateMusicDuration, ArgUpdateMusicLyric, ArgUpdatePlaylist,
        },
        plugin_manager,
        preference::{get_preference_playmode, save_preference_playmode},
    },
    Backend, PlayerContextHandle, PlayerHandle,
};

/// Outcome of a successful dispatch: payload JSON + optional output buffers.
pub(crate) type DispatchResult = BResult<(Value, Vec<Vec<u8>>)>;

/// Top-level entry: builds the envelope around [`dispatch_inner`].
pub(crate) async fn dispatch(req: BridgeRequest, buffers: Vec<Vec<u8>>) -> (Value, Vec<Vec<u8>>) {
    match dispatch_inner(req, buffers).await {
        Ok((payload, bufs)) => (json!({ "success": true, "payload": payload }), bufs),
        Err(e) => {
            let err_value = serde_json::to_value(&e).unwrap_or_else(|_| {
                json!({
                    "errorCode": "SerializationError",
                    "errorDetail": format!("failed to serialize BError: {e:?}"),
                })
            });
            let mut resp = serde_json::Map::new();
            resp.insert("success".into(), Value::Bool(false));
            if let Value::Object(map) = err_value {
                for (k, v) in map {
                    resp.insert(k, v);
                }
            }
            (Value::Object(resp), vec![])
        }
    }
}

/// The big match. Each branch:
///   1. Looks up handles via the handle table.
///   2. Deserializes `req.args` into the concrete arg types.
///   3. Calls the underlying controller function.
///   4. Serializes the return value back to a `Value`.
async fn dispatch_inner(req: BridgeRequest, buffers: Vec<Vec<u8>>) -> DispatchResult {
    let method = req.method.as_str();
    let handle = req.handle.unwrap_or(0);

    match method {
        // ====================================================================
        // backend.*
        // ====================================================================
        "backend.create" => {
            let arg: ArgInitializeApp = serde_json::from_value(req.args)?;
            let backend = crate::create_backend(arg);
            let id = register(HandleEntry::Backend(backend));
            Ok((json!({ "handle": id }), vec![]))
        }
        "backend.init" => {
            let backend = must_backend(handle)?;
            backend.init_async().await?;
            Ok((Value::Null, vec![]))
        }
        "backend.deinit" => {
            let backend = must_backend(handle)?;
            backend.deinit_async().await?;
            Ok((Value::Null, vec![]))
        }
        "backend.log" => {
            #[derive(Deserialize)]
            struct Args {
                level: String,
                message: String,
            }
            let args: Args = serde_json::from_value(req.args)?;
            match args.level.as_str() {
                "error" => tracing::error!("{}", args.message),
                _ => tracing::info!("{}", args.message),
            }
            Ok((Value::Null, vec![]))
        }

        // ====================================================================
        // music.*
        // ====================================================================
        "music.get" => {
            let id: MusicId = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let result = ct_get_music(cx, id).await?;
            Ok((serde_json::to_value(result)?, vec![]))
        }
        "music.getAbstract" => {
            let id: MusicId = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let cx_cx = cx.get_context().clone();
            let result = get_music_abstract(&cx_cx, id).await?;
            Ok((serde_json::to_value(result)?, vec![]))
        }
        "music.updateLyric" => {
            let arg: ArgUpdateMusicLyric = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            ct_update_music_lyric(cx, arg).await?;
            Ok((Value::Null, vec![]))
        }
        "music.updateDuration" => {
            let arg: ArgUpdateMusicDuration = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let cx_cx = cx.get_context().clone();
            // Inside our dispatcher we're already on the tokio runtime
            // (block_on at the JNI layer); call the async service fn
            // directly instead of going through the cts_ block_on wrapper.
            update_music_duration(&cx_cx, arg).await?;
            Ok((Value::Null, vec![]))
        }
        "music.updateCover" => {
            #[derive(Deserialize)]
            struct Args {
                id: MusicId,
                bytesIndex: usize,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cover =
                buffers
                    .into_iter()
                    .nth(args.bytesIndex)
                    .ok_or_else(|| BError::CustomError {
                        message: format!(
                            "music.updateCover: missing buffer at index {}",
                            args.bytesIndex
                        ),
                    })?;
            let cx = must_backend(handle)?;
            let cx_cx = cx.get_context().clone();
            update_music_cover(&cx_cx, ArgUpdateMusicCover { id: args.id, cover }).await?;
            Ok((Value::Null, vec![]))
        }

        // ====================================================================
        // playlist.*
        // ====================================================================
        "playlist.get" => {
            let id: PlaylistId = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let result = ct_get_playlist(cx, id).await?;
            Ok((serde_json::to_value(result)?, vec![]))
        }
        "playlist.list" => {
            let cx = must_backend(handle)?;
            let result = ct_list_playlist(cx).await?;
            Ok((serde_json::to_value(result)?, vec![]))
        }
        "playlist.create" => {
            let arg: ArgCreatePlaylist = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let result = ct_create_playlist(cx, arg).await?;
            Ok((serde_json::to_value(result)?, vec![]))
        }
        "playlist.update" => {
            let arg: ArgUpdatePlaylist = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            ct_update_playlist(cx, arg).await?;
            Ok((Value::Null, vec![]))
        }
        "playlist.remove" => {
            let id: PlaylistId = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            ct_remove_playlist(cx, id).await?;
            Ok((Value::Null, vec![]))
        }
        "playlist.addMusics" => {
            let arg: ArgAddMusicsToPlaylist = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let result = ct_add_musics_to_playlist(cx, arg).await?;
            Ok((serde_json::to_value(result)?, vec![]))
        }
        "playlist.removeMusic" => {
            let arg: ArgRemoveMusicFromPlaylist = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            ct_remove_music_from_playlist(cx, arg).await?;
            Ok((Value::Null, vec![]))
        }
        "playlist.reorder" => {
            let arg: ArgReorderPlaylist = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let cx_cx = cx.get_context().clone();
            reorder_playlist_inner(&cx_cx, arg).await?;
            Ok((Value::Null, vec![]))
        }
        "playlist.reorderMusic" => {
            let arg: ArgReorderMusic = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let cx_cx = cx.get_context().clone();
            reorder_music_in_playlist_inner(&cx_cx, arg).await?;
            Ok((Value::Null, vec![]))
        }

        // ====================================================================
        // storage.*
        // ====================================================================
        "storage.list" => {
            let cx = must_backend(handle)?;
            let result = ct_list_storage(cx).await?;
            Ok((serde_json::to_value(result)?, vec![]))
        }
        "storage.remove" => {
            let id: StorageId = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            ct_remove_storage(cx, id).await?;
            Ok((Value::Null, vec![]))
        }
        "storage.listEntryChildren" => {
            let loc: StorageEntryLoc = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let result = ct_list_storage_entry_children(cx, loc).await?;
            Ok((serde_json::to_value(result)?, vec![]))
        }

        // ====================================================================
        // storage_plugin.* — OAuth add / instance removal for JS plugin
        // storage providers (e.g. OneDrive). The provider prefixes the op
        // namespace (`<provider>:oauth.url` etc.); the plugin id follows the
        // `com.ease.<provider>` convention.
        // ====================================================================
        "storage_plugin.oauth_url" => {
            #[derive(Deserialize)]
            struct Args {
                provider: String,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let rpc = cx
                .get_context()
                .service_rpc_for(&format!("com.ease.{}", args.provider))
                .ok_or_else(|| BError::CustomError {
                    message: "service RPC not wired (headless instance not up)".into(),
                })?;
            let result = rpc
                .call(
                    &format!("{}:oauth.url", args.provider),
                    serde_json::json!({}),
                )
                .await
                .map_err(|e| BError::CustomError {
                    message: format!("oauth.url rpc: {e}"),
                })?;
            Ok((serde_json::to_value(result)?, vec![]))
        }
        "storage_plugin.oauth_exchange" => {
            #[derive(Deserialize)]
            struct Args {
                provider: String,
                code: String,
                #[serde(default)]
                alias: Option<String>,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let cx_cx = cx.get_context().clone();
            let rpc = cx_cx
                .service_rpc_for(&format!("com.ease.{}", args.provider))
                .ok_or_else(|| BError::CustomError {
                    message: "service RPC not wired (headless instance not up)".into(),
                })?;
            let mut call_args = serde_json::json!({ "code": args.code });
            if let Some(a) = &args.alias {
                call_args["alias"] = serde_json::Value::String(a.clone());
            }
            let result = rpc
                .call(&format!("{}:oauth.exchange", args.provider), call_args)
                .await
                .map_err(|e| BError::CustomError {
                    message: format!("oauth.exchange rpc: {e}"),
                })?;
            let instance = result
                .get("instance")
                .and_then(|v| v.as_str())
                .ok_or_else(|| BError::CustomError {
                    message: "oauth.exchange: plugin did not return an instance id".into(),
                })?;
            let handle = StorageHandle::Plugin {
                plugin_id: PluginId::new(format!("com.ease.{}", args.provider)),
                plugin_storage_id: PluginStorageId::new(instance.to_string()),
            };
            let id = cx_cx.database_server().obtain_storage(&handle).await?;
            crate::services::evict_storage_backend_cache(&cx_cx, id);
            Ok((serde_json::json!({ "storageId": *id.as_ref() }), vec![]))
        }
        "storage_plugin.remove_instance" => {
            let id: StorageId = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let cx_cx = cx.get_context().clone();
            // Load the row to find the provider + instance, then ask the plugin
            // to drop its config (kv) + secret before removing the registry row.
            if let Some(row) = cx_cx.database_server().load_storage_row(id).await? {
                if let (Some(plugin_storage_id), Some(plugin_id)) =
                    (row.plugin_storage_id, row.plugin_id)
                {
                    let provider = plugin_storage_id
                        .split(':')
                        .next()
                        .unwrap_or(&plugin_storage_id)
                        .to_string();
                    if let Some(rpc) = cx_cx.service_rpc_for(&plugin_id) {
                        let _ = rpc
                            .call(
                                &format!("{}:removeInstance", provider),
                                serde_json::json!({ "instance": plugin_storage_id }),
                            )
                            .await;
                    }
                }
            }
            ct_remove_storage(cx, id).await?;
            Ok((Value::Null, vec![]))
        }

        // ====================================================================
        // asset.*
        // ====================================================================
        "asset.get" => {
            use ease_client_schema::DataSourceKey;
            let key: DataSourceKey = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let bytes_opt = ct_get_asset(cx, key).await?;
            match bytes_opt {
                Some(bytes) => {
                    let payload = json!({ "bytesIndex": 0 });
                    Ok((payload, vec![bytes]))
                }
                None => Ok((Value::Null, vec![])),
            }
        }

        // ====================================================================
        // player.* — handle refers to the PlayerHandle (or PlayerContext
        // for contextNew / Player for new + transport ops).
        // ====================================================================
        "player.contextNew" => {
            let ctx = ct_player_context_new()?;
            let id = register(HandleEntry::PlayerContext(ctx));
            Ok((json!({ "handle": id }), vec![]))
        }
        "player.new" => {
            let cx = must_player_context(handle)?;
            let player = ct_player_new(cx)?;
            let id = register(HandleEntry::Player(player));
            Ok((json!({ "handle": id }), vec![]))
        }
        "player.loadMusic" => {
            #[derive(Deserialize)]
            struct Args {
                backendHandle: u64,
                musicId: MusicId,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let player = must_player(handle)?;
            let backend = must_backend(args.backendHandle)?;
            let metadata = ct_player_load_music(backend, player, args.musicId).await?;
            Ok((serde_json::to_value(metadata)?, vec![]))
        }
        "player.play" => {
            let player = must_player(handle)?;
            ct_player_play(player).await?;
            Ok((Value::Null, vec![]))
        }
        "player.pause" => {
            let player = must_player(handle)?;
            ct_player_pause(player).await?;
            Ok((Value::Null, vec![]))
        }
        "player.stop" => {
            let player = must_player(handle)?;
            ct_player_stop(player).await?;
            Ok((Value::Null, vec![]))
        }
        "player.seek" => {
            let pos_ms: u64 = serde_json::from_value(req.args)?;
            let player = must_player(handle)?;
            let actual = ct_player_seek(player, pos_ms).await?;
            Ok((serde_json::to_value(actual)?, vec![]))
        }
        "player.setVolume" => {
            let volume: f32 = serde_json::from_value(req.args)?;
            let player = must_player(handle)?;
            ct_player_set_volume(player, volume).await?;
            Ok((Value::Null, vec![]))
        }
        "player.probeDurationMs" => {
            #[derive(Deserialize)]
            struct Args {
                contextHandle: u64,
                backendHandle: u64,
                musicId: MusicId,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_player_context(args.contextHandle)?;
            let backend = must_backend(args.backendHandle)?;
            let dur = ct_player_probe_duration_ms(cx, backend, args.musicId).await?;
            Ok((serde_json::to_value(dur)?, vec![]))
        }
        "player.pollState" => {
            // Batched: state + positionMs + durationMs in one shot.
            // Hot path (10 Hz from CantodeEngine); combining three FFI
            // calls into one cuts JSON overhead 3x.
            let player = must_player(handle)?;
            let state = ct_player_state(player.clone());
            let position_ms = ct_player_position_ms(player.clone());
            let duration_ms = ct_player_duration_ms(player);
            Ok((
                json!({
                    "state": state,
                    "positionMs": position_ms,
                    "durationMs": duration_ms,
                }),
                vec![],
            ))
        }

        // ====================================================================
        // preference.*
        // ====================================================================
        "preference.savePlayMode" => {
            let mode: PlayMode = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let cx_cx = cx.get_context().clone();
            save_preference_playmode(&cx_cx, mode).await?;
            Ok((Value::Null, vec![]))
        }
        "preference.getPlayMode" => {
            let cx = must_backend(handle)?;
            let cx_cx = cx.get_context().clone();
            let mode = get_preference_playmode(&cx_cx).await?;
            Ok((serde_json::to_value(mode)?, vec![]))
        }

        // ====================================================================
        // plugin.* — only the methods actually called from Kotlin.
        // The other 14 plugin KV functions are routed in-process via
        // BACKEND_CONTEXT (tur engine db_bridge), not through this
        // bridge.
        // ====================================================================
        "plugin.event" => {
            #[derive(Deserialize)]
            struct Args {
                pluginId: String,
                #[serde(rename = "type")]
                event_type: String,
                payload: Value,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            cx.get_context().dispatch_plugin_event(
                &args.pluginId,
                &args.event_type,
                args.payload,
            )?;
            Ok((Value::Null, vec![]))
        }

        // plugin.* — the Rust-side plugin install layer (see
        // `services/plugin_manager.rs`). All file/registry/state IO happens
        // here; Kotlin keeps only the SAF picker, VMs, and the tur instance
        // lifecycle. `plugin.list` returns opaque module-source handles
        // (tur #198) so plugin JS never crosses the boundary as a string.
        "plugin.list" => {
            let cx = must_backend(handle)?;
            let out = plugin_manager::scan(cx.get_context(), &cx.arg.app_document_dir).await?;
            Ok((serde_json::to_value(out)?, vec![]))
        }
        "plugin.installZipPath" => {
            #[derive(Deserialize)]
            struct Args {
                path: String,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let (id, generation) = plugin_manager::install_from_zip_path(
                cx.get_context(),
                &cx.arg.app_document_dir,
                &args.path,
            )
            .await?;
            Ok((json!({ "id": id, "generation": generation }), vec![]))
        }
        "plugin.installFromRegistry" => {
            #[derive(Deserialize)]
            struct Args {
                entry: plugin_manager::RegistryEntry,
                baseUrl: String,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let (id, generation) = plugin_manager::install_from_registry(
                cx.get_context(),
                &cx.arg.app_document_dir,
                &args.entry,
                &args.baseUrl,
            )
            .await?;
            Ok((json!({ "id": id, "generation": generation }), vec![]))
        }
        "plugin.setEnable" => {
            #[derive(Deserialize)]
            struct Args {
                pluginId: String,
                enabled: bool,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let generation = plugin_manager::set_enabled(
                cx.get_context(),
                &cx.arg.app_document_dir,
                &args.pluginId,
                args.enabled,
            )
            .await?;
            Ok((json!({ "generation": generation }), vec![]))
        }
        "plugin.uninstall" => {
            #[derive(Deserialize)]
            struct Args {
                pluginId: String,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let generation = plugin_manager::uninstall(
                cx.get_context(),
                &cx.arg.app_document_dir,
                &args.pluginId,
            )
            .await?;
            Ok((json!({ "generation": generation }), vec![]))
        }
        "plugin.bootstrap" => {
            let cx = must_backend(handle)?;
            let generation =
                plugin_manager::bootstrap(cx.get_context(), &cx.arg.app_document_dir).await?;
            Ok((json!({ "generation": generation }), vec![]))
        }
        "plugin.registryFetch" => {
            #[derive(Deserialize)]
            struct Args {
                baseUrl: String,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let dir = cx.arg.app_document_dir.clone();
            let base = args.baseUrl;
            let entries = plugin_manager::fetch_registry(&dir, &base).await?;
            let root = plugin_manager::plugins_root(&dir);
            let entries =
                tokio::task::spawn_blocking(move || plugin_manager::stamp_entries(entries, &root))
                    .await
                    .map_err(|e| BError::CustomError {
                        message: format!("stamp task: {e}"),
                    })?;
            Ok((json!({ "entries": entries }), vec![]))
        }
        "plugin.registryCached" => {
            #[derive(Deserialize)]
            struct Args {
                baseUrl: String,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let dir = cx.arg.app_document_dir.clone();
            let root = plugin_manager::plugins_root(&dir);
            let entries = tokio::task::spawn_blocking(move || {
                plugin_manager::cached_registry(&dir, &args.baseUrl)
                    .map(|e| plugin_manager::stamp_entries(e, &root))
                    .unwrap_or_default()
            })
            .await
            .map_err(|e| BError::CustomError {
                message: format!("cache task: {e}"),
            })?;
            Ok((json!({ "entries": entries }), vec![]))
        }
        "plugin.sourcesList" => {
            let cx = must_backend(handle)?;
            let dir = cx.arg.app_document_dir.clone();
            let out = tokio::task::spawn_blocking(move || {
                let state = plugin_manager::read_state(&dir);
                json!({
                    "presets": plugin_manager::preset_sources()
                        .into_iter()
                        .map(|(url, label)| json!({ "url": url, "label": label, "preset": true }))
                        .collect::<Vec<_>>(),
                    "customSources": state
                        .custom_sources
                        .iter()
                        .map(|c| json!({ "url": c.url, "label": c.label, "preset": false }))
                        .collect::<Vec<_>>(),
                    "lastSourceUrl": plugin_manager::effective_last_source(&state),
                })
            })
            .await
            .map_err(|e| BError::CustomError {
                message: format!("sources task: {e}"),
            })?;
            Ok((out, vec![]))
        }
        "plugin.sourceRemember" => {
            #[derive(Deserialize)]
            struct Args {
                url: String,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            plugin_manager::remember_source(&cx.arg.app_document_dir, &args.url)?;
            Ok((Value::Null, vec![]))
        }
        "plugin.sourceAddCustom" => {
            #[derive(Deserialize)]
            struct Args {
                url: String,
                #[serde(default)]
                label: Option<String>,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            let dir = cx.arg.app_document_dir.clone();
            let entries =
                plugin_manager::add_custom_source(&dir, &args.url, args.label.as_deref()).await?;
            let root = plugin_manager::plugins_root(&dir);
            let entries =
                tokio::task::spawn_blocking(move || plugin_manager::stamp_entries(entries, &root))
                    .await
                    .map_err(|e| BError::CustomError {
                        message: format!("stamp task: {e}"),
                    })?;
            Ok((json!({ "entries": entries }), vec![]))
        }
        "plugin.sourceRemoveCustom" => {
            #[derive(Deserialize)]
            struct Args {
                url: String,
            }
            let args: Args = serde_json::from_value(req.args)?;
            let cx = must_backend(handle)?;
            plugin_manager::remove_custom_source(&cx.arg.app_document_dir, &args.url)?;
            Ok((Value::Null, vec![]))
        }

        // ====================================================================
        // debug.*
        // ====================================================================
        "debug.listLogFiles" => {
            let cx = must_backend(handle)?;
            let result = cts_list_log_files(cx)?;
            Ok((serde_json::to_value(result)?, vec![]))
        }
        "debug.triggerError" => {
            let cx = must_backend(handle)?;
            cts_trigger_error(cx)?;
            Ok((Value::Null, vec![]))
        }
        "debug.triggerPanic" => {
            let cx = must_backend(handle)?;
            cts_trigger_panic(cx)?;
            Ok((Value::Null, vec![]))
        }

        unknown => Err(BError::CustomError {
            message: format!("unknown bridge method: {unknown}"),
        }),
    }
}

// ====================================================================
// Internal helpers
// ====================================================================

fn must_backend(handle: u64) -> BResult<Arc<Backend>> {
    get_backend(handle).ok_or_else(|| BError::CustomError {
        message: format!("no backend registered for handle {handle}"),
    })
}

fn must_player_context(handle: u64) -> BResult<Arc<PlayerContextHandle>> {
    get_player_context(handle).ok_or_else(|| BError::CustomError {
        message: format!("no player context registered for handle {handle}"),
    })
}

fn must_player(handle: u64) -> BResult<Arc<PlayerHandle>> {
    get_player(handle).ok_or_else(|| BError::CustomError {
        message: format!("no player registered for handle {handle}"),
    })
}
