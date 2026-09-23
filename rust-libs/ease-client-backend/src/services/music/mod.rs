use std::collections::HashMap;
use std::time::Duration;

use ease_client_schema::{
    DataSourceKey, MusicId, MusicModel, PlaylistGroupId, PlaylistId, StorageEntryLoc, StorageId,
};
use serde::{Deserialize, Serialize};

use crate::{
    ctx::BackendContext,
    error::BResult,
    objects::{LyricLine, LyricLoadState, Lyrics, Music, MusicAbstract, MusicLyric, MusicMeta},
    StorageEntry,
};

use super::{
    lyrics::{parse_lyric_content, MAX_LINES, MAX_LYRIC_BYTES},
    storage::{get_storage_backend, load_storage_entry_data},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgUpdatePlaylist {
    pub id: PlaylistId,
    pub title: String,
    pub cover: Option<StorageEntryLoc>,
    /// Import-source restriction (`None` = all storages). Defaults for
    /// older callers that predate the field.
    #[serde(default)]
    pub storage_allowlist: Option<Vec<StorageId>>,
    /// Move the playlist into another group when `Some` and different
    /// from the current one. `None` (default) = keep the group.
    #[serde(default)]
    pub group_id: Option<PlaylistGroupId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToAddMusicEntry {
    pub entry: StorageEntry,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgCreatePlaylist {
    pub title: String,
    pub cover: Option<StorageEntryLoc>,
    pub entries: Vec<ToAddMusicEntry>,
    /// Import-source restriction (`None` = all storages). Defaults for
    /// older callers that predate the field.
    #[serde(default)]
    pub storage_allowlist: Option<Vec<StorageId>>,
    /// Group the playlist is created in. Required — playlists always
    /// live in a group.
    pub group_id: PlaylistGroupId,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgAddMusicsToPlaylist {
    pub id: PlaylistId,
    pub entries: Vec<ToAddMusicEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgRemoveMusicFromPlaylist {
    pub playlist_id: PlaylistId,
    pub music_id: MusicId,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgUpdateMusicLyric {
    pub id: MusicId,
    pub lyric_loc: Option<StorageEntryLoc>,
}

/// Try each candidate location in order: a fetch miss (missing file /
/// mid-stream error — e.g. a missing sibling) moves on to the next
/// candidate. The first successfully fetched file is parsed exactly once
/// through the plugin chain ([`parse_lyric_content`]) — a parse failure
/// is terminal (`Failed`), it does not retry further locations (the file
/// exists but nothing can parse it). All candidates missing ⇒ `Missing`
/// when every candidate was a fallback, `Failed` for the explicit pick.
async fn load_lyric(
    cx: &BackendContext,
    candidates: Vec<(StorageEntryLoc, bool)>,
) -> Option<MusicLyric> {
    if candidates.is_empty() {
        return None;
    }
    for (loc, _) in candidates.iter() {
        let data = load_storage_entry_data(cx, loc).await;
        let bytes = match data {
            Err(e) => {
                tracing::error!("fail to load entry {loc:?}: {e}");
                continue;
            }
            Ok(None) => continue,
            Ok(Some(bytes)) => bytes,
        };
        let file_name = loc
            .path
            .rsplit('/')
            .next()
            .unwrap_or(loc.path.as_str())
            .to_string();
        return Some(match parse_lyric_content(cx, &file_name, &bytes).await {
            Ok(data) => MusicLyric {
                loc: loc.clone(),
                data,
                loaded_state: LyricLoadState::Loaded,
            },
            Err(e) => {
                tracing::error!("fail to parse lyric '{file_name}': {e}");
                MusicLyric {
                    loc: loc.clone(),
                    data: Default::default(),
                    loaded_state: LyricLoadState::Failed,
                }
            }
        });
    }
    // Every candidate missed.
    let (loc, is_fallback) = candidates.into_iter().next().unwrap();
    Some(MusicLyric {
        loc,
        data: Default::default(),
        loaded_state: if is_fallback {
            LyricLoadState::Missing
        } else {
            LyricLoadState::Failed
        },
    })
}

/// Embedded-tag fallback for [`load_music_lyric`]: the lyric text
/// captured from the audio's own container tags (`music.embedded_lyric`,
/// filled by the `player.loadMusic` writeback). LRC-shaped text is
/// dispatched through the plugin chain under a synthesized `.lrc` name —
/// parsers stay plugin-owned and the user's parser selection applies;
/// anything no parser claims renders as unsynchronized plain text (all
/// lines at t=0, `Lyrics.synced = false`). Returns `None` when no tag
/// text was captured (or it is blank).
async fn embedded_music_lyric(cx: &BackendContext, model: &MusicModel) -> Option<MusicLyric> {
    let text = model.embedded_lyric.as_deref()?.trim();
    if text.is_empty() {
        return None;
    }
    let loc = model.loc.clone();
    let audio_name = loc.path.rsplit('/').next().unwrap_or(loc.path.as_str());
    // `"song.mp3" -> "song.mp3.lrc"`: `parse_lyric_content` dispatches by
    // file extension only, and the full-name form mirrors how sibling
    // candidates are built.
    let lrc_name = format!("{audio_name}.lrc");
    let data = match parse_lyric_content(cx, &lrc_name, text.as_bytes()).await {
        Ok(data) => data,
        Err(e) => {
            // Expected for plain USLT text (no timed lines) — the info
            // level keeps the log readable; this is the designed path.
            tracing::info!(
                "embedded lyric of '{audio_name}' has no synced parse ({e}); using plain text"
            );
            unsynced_lyrics_from_text(text)
        }
    };
    Some(MusicLyric {
        loc,
        data,
        loaded_state: LyricLoadState::Loaded,
    })
}

/// Build unsynchronized [`Lyrics`] from plain tag text: split on line
/// terminators (`\n`, `\r\n` or a lone `\r` — same set the plugin
/// parsers accept), trim, drop blanks, every line at t=0
/// (`synced = false` — the client renders it without highlight /
/// auto-scroll). Splitting plain text is not lyric-format knowledge;
/// every *synced* format stays plugin-owned. Lines are capped at
/// [`MAX_LINES`].
fn unsynced_lyrics_from_text(text: &str) -> Lyrics {
    // `str::lines` misses the lone-\r terminator some taggers emit.
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<LyricLine> = normalized
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(MAX_LINES)
        .map(|l| LyricLine {
            duration: Duration::ZERO,
            text: l.to_string(),
        })
        .collect();
    Lyrics {
        metdata: Default::default(),
        lines,
        synced: false,
    }
}

pub(crate) fn build_music_meta(model: MusicModel) -> MusicMeta {
    MusicMeta {
        id: model.id,
        title: model.title,
        artist: model.artist,
        duration: model.duration,
        order: model.order,
    }
}

pub(crate) fn build_music_abstract(_cx: &BackendContext, model: MusicModel) -> MusicAbstract {
    let cover = if model.cover.is_some() {
        Some(DataSourceKey::Cover { id: model.id })
    } else {
        Default::default()
    };

    MusicAbstract {
        cover,
        meta: build_music_meta(model),
    }
}

pub async fn get_music_storage_entry_loc(
    cx: &BackendContext,
    id: MusicId,
) -> BResult<Option<StorageEntryLoc>> {
    let m = cx.database_server().load_music(id).await?;
    if m.is_none() {
        return Ok(None);
    }
    let m = m.unwrap();
    let m = m.loc;
    Ok(Some(m))
}

pub async fn get_music_cover_bytes(cx: &BackendContext, id: MusicId) -> BResult<Vec<u8>> {
    let m = cx.database_server().load_music(id).await?.unwrap();
    if let Some(id) = m.cover {
        cx.database_server().blob().read(id)
    } else {
        Ok(Default::default())
    }
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgUpdateMusicDuration {
    pub id: MusicId,
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub duration: Duration,
}
pub(crate) async fn update_music_duration(
    cx: &BackendContext,
    arg: ArgUpdateMusicDuration,
) -> BResult<()> {
    cx.database_server()
        .update_music_total_duration(arg.id, arg.duration)
        .await?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgUpdateMusicArtist {
    pub id: MusicId,
    pub artist: String,
}

/// Persist a probed track artist. The **only-if-empty guard lives here**
/// — the single choke point shared by the `player.loadMusic` writeback
/// and the `music.updateArtist` bridge arm — so a probed artist never
/// clobbers a value already in the DB (mirrors the duration/cover
/// writeback policy). Blank probes are dropped silently.
pub(crate) async fn update_music_artist(
    cx: &BackendContext,
    arg: ArgUpdateMusicArtist,
) -> BResult<()> {
    let artist = arg.artist.trim();
    if artist.is_empty() {
        return Ok(());
    }
    let Some(m) = cx.database_server().load_music(arg.id).await? else {
        return Ok(());
    };
    if !m.artist.is_empty() {
        return Ok(());
    }
    cx.database_server()
        .update_music_artist(arg.id, artist.to_string())
        .await?;
    Ok(())
}

/// Persist an embedded tag lyric captured by the `player.loadMusic`
/// writeback. Plain setter — the only-if-NULL guard lives in the single
/// caller (the metadata writeback), mirroring the duration/cover policy.
/// Not bridge-exposed: embedded lyrics are data capture, never user
/// input.
pub(crate) async fn update_music_embedded_lyric(
    cx: &BackendContext,
    id: MusicId,
    lyric: String,
) -> BResult<()> {
    cx.database_server()
        .update_music_embedded_lyric(id, lyric)
        .await?;
    Ok(())
}

/// Extract the artist from probed container tags. cantode normalizes tag
/// keys to symphonia-standard names via `std_key` (ID3 `TPE1` arrives as
/// `Artist`, MP4 `aART` as `Artist`, ...), so a case-insensitive match on
/// those names covers the common containers. `Artist` wins; `AlbumArtist`
/// is the fallback for compilations / files with an empty per-track tag.
/// Multi-value tags (FLAC Vorbis comments) are joined with `", "`.
/// `None` = nothing usable — the DB keeps its empty artist.
pub(crate) fn extract_artist_tag(tags: &[cantode::Tag]) -> Option<String> {
    for key in ["Artist", "AlbumArtist"] {
        let values: Vec<&str> = tags
            .iter()
            .filter(|t| t.key.eq_ignore_ascii_case(key))
            .map(|t| t.value.trim())
            .filter(|v| !v.is_empty())
            .collect();
        if !values.is_empty() {
            return Some(values.join(", "));
        }
    }
    None
}

/// Extract embedded lyrics from probed container tags. cantode normalizes
/// tag keys to symphonia-standard names, and symphonia maps ID3v2 `USLT`
/// (MP3), the Vorbis comments `lyrics` / `unsyncedlyrics` (FLAC / OGG)
/// and the MP4 `©lyr` atom all to `Lyrics`. The first non-empty value
/// wins (multiple USLT frames = multiple languages; there is no language
/// preference). Text over [`MAX_LYRIC_BYTES`] is dropped — the
/// sidecar/missing paths remain. `None` = nothing usable. (ID3v2 `SYLT`
/// and WMA never surface here: symphonia skips SYLT frames and has no
/// ASF demuxer at all.)
pub(crate) fn extract_lyric_tag(tags: &[cantode::Tag]) -> Option<String> {
    for t in tags.iter().filter(|t| t.key.eq_ignore_ascii_case("Lyrics")) {
        let v = t.value.trim();
        if v.is_empty() {
            continue;
        }
        if v.len() > MAX_LYRIC_BYTES {
            tracing::warn!(
                "embedded lyric dropped: {len} bytes > {MAX_LYRIC_BYTES}",
                len = v.len()
            );
            return None;
        }
        return Some(v.to_string());
    }
    None
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgUpdateMusicCover {
    pub id: MusicId,
    pub cover: Vec<u8>,
}

/// Largest cover blob accepted at write time. Covers ride in full over
/// the bridge buffer channel and are decoded on the UI thread; multi-MB
/// artwork is pathological (and the usual cause of decode OOMs).
const MAX_COVER_BYTES: usize = 4 * 1024 * 1024;

/// Pure gate used by [`update_music_cover`]: `Err(reason)` for bytes that
/// cannot be a raster image. Keep this the single choke point — import
/// probes, `music.updateCover` and the player loadMusic writeback all
/// land here, so a rejected cover simply never enters the DB and
/// `Music.cover` / `has_cover` keep meaning "a cover that will render".
fn validate_cover_bytes(cover: &[u8]) -> Result<(), &'static str> {
    if cover.len() > MAX_COVER_BYTES {
        return Err("too large");
    }
    if !sniffs_as_image(cover) {
        return Err("unrecognized image data");
    }
    Ok(())
}

/// Magic-number sniff — deliberately cheap and dependency-free. WebP /
/// HEIC / AVIF acceptance is a container check only; whether the device
/// actually decodes it stays BitmapFactory's call, and the Kotlin side
/// treats a failed decode as terminal (default art + warn log).
fn sniffs_as_image(buf: &[u8]) -> bool {
    const PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if buf.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return true; // JPEG
    }
    if buf.starts_with(&PNG) {
        return true;
    }
    if buf.starts_with(b"GIF87a") || buf.starts_with(b"GIF89a") {
        return true;
    }
    if buf.starts_with(b"BM") && buf.len() >= 14 {
        return true; // BMP (BITMAPFILEHEADER alone is 14 bytes)
    }
    if buf.len() >= 12 && &buf[0..4] == b"RIFF" && &buf[8..12] == b"WEBP" {
        return true;
    }
    // ISO-BMFF containers: "ftyp" box at 0, major brand at 8.
    if buf.len() >= 12 && &buf[4..8] == b"ftyp" {
        return matches!(
            &buf[8..12],
            b"heic" | b"heix" | b"hevc" | b"hevx" | b"avif" | b"mif1" | b"msf1"
        );
    }
    false
}

pub(crate) async fn update_music_cover(
    cx: &BackendContext,
    arg: ArgUpdateMusicCover,
) -> BResult<()> {
    // Best-effort by contract: a non-image (e.g. an ID3v2 APIC link entry
    // carrying a URL string, or an HTML error page mis-saved as art) is
    // dropped with a warn instead of poisoning the DB with bytes no
    // client can decode.
    if let Err(reason) = validate_cover_bytes(&arg.cover) {
        tracing::warn!(
            "rejected cover for music {:?}: {} bytes ({reason})",
            arg.id,
            arg.cover.len()
        );
        return Ok(());
    }
    cx.database_server()
        .update_music_cover(arg.id, arg.cover.clone())
        .await?;
    Ok(())
}

/// Resolve a music's candidate lyric locations: the explicit `model.lyric`
/// if set, else — when `lyric_default` is enabled — siblings per
/// registered parser extension (`<audio>.<ext>` and `<audio-base>.<ext>`,
/// registry order —
/// `plugin_manager::PluginManagerShared::lyric_sibling_extensions`).
/// Empty when nothing can parse (no parser plugin enabled and no explicit
/// pick). Returns `(loc, is_fallback)` pairs; only the explicit pick is
/// ever non-fallback.
///
/// A user's explicit pick (`lyric_default` cleared by `update_music_lyric`)
/// is exclusive. An import-detected loc (see
/// [`detect_import_sibling_lyrics`]) keeps `lyric_default` set — it is a
/// resolution snapshot, not a commitment, so the sibling candidates are
/// still appended and probing falls through to them if the attached file
/// can no longer be fetched.
fn resolve_lyric_locs(
    model: &MusicModel,
    sibling_extensions: &[String],
) -> Vec<(StorageEntryLoc, bool)> {
    let mut out: Vec<(StorageEntryLoc, bool)> = Vec::new();
    if let Some(loc) = model.lyric.clone() {
        out.push((loc, model.lyric_default));
        if !model.lyric_default {
            return out;
        }
    }
    if !model.lyric_default || sibling_extensions.is_empty() {
        return out;
    }
    let audio = &model.loc;
    // Same two name forms as import-time detection, same precedence per
    // extension: the full audio file name + lyric extension
    // (`a.wav.vtt`) before the extension-swapped base (`a.vtt`).
    let full_form = audio.path.clone();
    let base_form = match audio.path.rfind('.') {
        Some(pos) => audio.path[..pos].to_string(),
        None => String::new(),
    };
    out.extend(
        sibling_extensions
            .iter()
            .flat_map(|ext| {
                let full = format!("{full_form}.{ext}");
                let base = format!("{base_form}.{ext}");
                if base != full {
                    vec![full, base]
                } else {
                    // Extensionless audio — the two forms coincide.
                    vec![full]
                }
            })
            .map(|path| (StorageEntryLoc {
                path,
                storage_id: audio.storage_id,
            }, true))
            // The attached loc is already the first candidate — don't
            // probe the same path twice.
            .filter(|(loc, _)| Some(loc) != model.lyric.as_ref()),
    );
    out
}

/// Parent directory of a storage path (`"/"` for root-level files).
fn parent_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(pos) => path[..pos].to_string(),
    }
}

/// Match a music file's sibling lyric among a directory listing.
/// Extensions are tried in the given order (registry scan order — the
/// same precedence play-time probing uses). Within an extension, two
/// name forms are tried, most specific first: the full audio file name
/// with the lyric extension appended (`a.wav.vtt`) and the
/// extension-swapped base (`a.vtt`); for each, an exact hit wins over a
/// case-insensitive one (a real listing makes that free;
/// constructed-path probing cannot do it on case-sensitive storages).
/// Directory entries never match. Returns the matched entry's full path.
pub(crate) fn match_sibling_lyric(
    music_path: &str,
    listed: &[StorageEntry],
    extensions: &[String],
) -> Option<String> {
    let file_name = music_path.rsplit('/').next().unwrap_or(music_path);
    // Base form needs an extension to swap (and a non-empty base); the
    // full-name form is always available, so extensionless musics can
    // still match `<name>.<ext>` through it.
    let base = match file_name.rfind('.') {
        Some(pos) if pos > 0 => Some(&file_name[..pos]),
        _ => None,
    };
    for ext in extensions {
        let full_form = format!("{file_name}.{ext}");
        let mut forms = vec![full_form.clone()];
        if let Some(b) = base {
            let base_form = format!("{b}.{ext}");
            if base_form != full_form {
                forms.push(base_form);
            }
        }
        for wanted in forms {
            if let Some(hit) = listed.iter().find(|e| !e.is_dir && e.name == wanted) {
                return Some(hit.path.clone());
            }
            let wanted_lower = wanted.to_ascii_lowercase();
            if let Some(hit) = listed
                .iter()
                .find(|e| !e.is_dir && e.name.to_ascii_lowercase() == wanted_lower)
            {
                return Some(hit.path.clone());
            }
        }
    }
    None
}

/// List a folder over the storage seam; `None` on any failure (missing
/// backend, network error) — lyric detection is best-effort and must
/// never fail the import around it.
async fn list_storage_children(
    cx: &BackendContext,
    loc: &StorageEntryLoc,
) -> Option<Vec<StorageEntry>> {
    let backend = get_storage_backend(cx, loc.storage_id).await.ok()??;
    let entries = backend.list(loc.path.clone()).await.ok()?;
    Some(
        entries
            .into_iter()
            .map(|entry| StorageEntry {
                storage_id: loc.storage_id,
                name: entry.name,
                path: entry.path,
                size: entry.size.map(|s| s as u64),
                is_dir: entry.is_dir,
                created_at: entry.created_at,
                modified_at: entry.modified_at,
            })
            .collect(),
    )
}

/// Detect sibling lyric files for the musics of an import: list each
/// distinct `(storage, parent folder)` once and match `<base>.<ext>`
/// against every extension registered by enabled lyric-parser plugins —
/// the generalized form of the old fixed `.lrc`-sibling swap. Best
/// effort: a failed listing skips that group (lazy play-time probing
/// stays the safety net for anything undetected).
pub(crate) async fn detect_import_sibling_lyrics(
    cx: &BackendContext,
    music_locs: &[StorageEntryLoc],
) -> HashMap<StorageEntryLoc, StorageEntryLoc> {
    let mut out: HashMap<StorageEntryLoc, StorageEntryLoc> = HashMap::new();
    let extensions = cx.plugin_manager().lyric_sibling_extensions();
    if extensions.is_empty() || music_locs.is_empty() {
        return out;
    }

    // Distinct (storage, parent) groups in first-seen order — one
    // listing per group (an import's selection shares one folder).
    let mut groups: Vec<(StorageEntryLoc, Vec<usize>)> = Vec::new();
    for (idx, loc) in music_locs.iter().enumerate() {
        let parent = StorageEntryLoc {
            storage_id: loc.storage_id,
            path: parent_dir(&loc.path),
        };
        match groups.iter_mut().find(|(p, _)| *p == parent) {
            Some((_, members)) => members.push(idx),
            None => groups.push((parent, vec![idx])),
        }
    }

    for (parent, members) in groups {
        let Some(entries) = list_storage_children(cx, &parent).await else {
            tracing::warn!(
                "import lyric detection: failed to list {}/{} — skipping",
                parent.storage_id.as_ref(),
                parent.path
            );
            continue;
        };
        for idx in members {
            let loc = &music_locs[idx];
            if let Some(lyric_path) = match_sibling_lyric(&loc.path, &entries, &extensions) {
                out.insert(
                    loc.clone(),
                    StorageEntryLoc {
                        storage_id: loc.storage_id,
                        path: lyric_path,
                    },
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ease_client_schema::{BlobId, MusicId, StorageId};

    fn listed_entry(name: &str, is_dir: bool) -> StorageEntry {
        StorageEntry {
            storage_id: StorageId::wrap(1),
            name: name.to_string(),
            path: format!("/Music/{name}"),
            size: None,
            is_dir,
            created_at: None,
            modified_at: None,
        }
    }

    fn music_model(lyric: Option<StorageEntryLoc>, lyric_default: bool) -> MusicModel {
        MusicModel {
            id: MusicId::wrap(7),
            loc: StorageEntryLoc {
                storage_id: StorageId::wrap(1),
                path: "/Music/song.mp3".to_string(),
            },
            title: "song".to_string(),
            artist: String::new(),
            duration: None,
            cover: None::<BlobId>,
            lyric,
            lyric_default,
            embedded_lyric: None,
            order: vec![],
        }
    }

    fn exts(exts: &[&str]) -> Vec<String> {
        exts.iter().map(|e| e.to_string()).collect()
    }

    #[test]
    fn parent_dir_of_storage_paths() {
        assert_eq!(parent_dir("/Music/a/song.mp3"), "/Music/a");
        assert_eq!(parent_dir("/song.mp3"), "/");
        assert_eq!(parent_dir("/"), "/");
    }

    #[test]
    fn sibling_match_exact_then_case_insensitive_per_extension_order() {
        let exts = exts(&["lrc", "srt", "vtt"]);
        let listed = vec![
            listed_entry("cover.jpg", false),
            listed_entry("sub", true),
            listed_entry("song.srt", false),
            listed_entry("song.LRC", false),
        ];
        // Case-insensitive hit within the first extension beats an exact
        // hit in a later one (registry order wins).
        assert_eq!(
            match_sibling_lyric("/Music/song.mp3", &listed, &exts),
            Some("/Music/song.LRC".to_string())
        );

        // Exact hit beats a case-insensitive one of the same extension
        // regardless of listing order.
        let listed = vec![
            listed_entry("song.lrc", false),
            listed_entry("other.txt", false),
        ];
        assert_eq!(
            match_sibling_lyric("/Music/song.mp3", &listed, &exts),
            Some("/Music/song.lrc".to_string())
        );

        // Fall through to the next extension when the first misses.
        let listed = vec![listed_entry("song.SRT", false)];
        assert_eq!(
            match_sibling_lyric("/Music/song.mp3", &listed, &exts),
            Some("/Music/song.SRT".to_string())
        );

        // Directories never match, even named exactly.
        let listed = vec![listed_entry("song.lrc", true)];
        assert_eq!(match_sibling_lyric("/Music/song.mp3", &listed, &exts), None);

        // Nothing matches → None.
        let listed = vec![listed_entry("other.lrc", false)];
        assert_eq!(match_sibling_lyric("/Music/song.mp3", &listed, &exts), None);
    }

    #[test]
    fn sibling_match_full_name_form_and_precedence() {
        // Declared first: `let exts = …` below shadows the helper fn.
        let exts_late = exts(&["srt", "vtt"]);
        let exts = exts(&["lrc", "vtt"]);
        // `a.wav` also matches `a.wav.vtt` (full file name + lyric ext).
        let listed = vec![listed_entry("a.wav.vtt", false)];
        assert_eq!(
            match_sibling_lyric("/Music/a.wav", &listed, &exts),
            Some("/Music/a.wav.vtt".to_string())
        );

        // The full-name form is the more specific attachment: when both
        // forms exist for the same extension, it wins — this is what
        // disambiguates `a.wav` vs `a.flac` sharing a base.
        let listed = vec![
            listed_entry("a.lrc", false),
            listed_entry("a.wav.lrc", false),
        ];
        assert_eq!(
            match_sibling_lyric("/Music/a.wav", &listed, &exts),
            Some("/Music/a.wav.lrc".to_string())
        );

        // ... but extension order is the primary key: an earlier
        // extension's base form beats a later extension's full form.
        let listed = vec![
            listed_entry("a.srt", false),
            listed_entry("a.wav.vtt", false),
        ];
        assert_eq!(
            match_sibling_lyric("/Music/a.wav", &listed, &exts_late),
            Some("/Music/a.srt".to_string())
        );

        // Case-insensitive matching applies to the full-name form too.
        let listed = vec![listed_entry("a.WAV.VTT", false)];
        assert_eq!(
            match_sibling_lyric("/Music/a.wav", &listed, &exts),
            Some("/Music/a.WAV.VTT".to_string())
        );
    }

    #[test]
    fn sibling_match_without_extension_uses_full_name_form() {
        // Extensionless musics can still match `<name>.<ext>` — the two
        // forms coincide, so nothing is tried twice.
        let listed = vec![listed_entry("song.lrc", false)];
        assert_eq!(
            match_sibling_lyric("/Music/song", &listed, &exts(&["lrc"])),
            Some("/Music/song.lrc".to_string())
        );
        // Nothing listed → no match.
        assert_eq!(match_sibling_lyric("/Music/song", &[], &exts(&["lrc"])), None);
        // Empty extension registry.
        assert_eq!(
            match_sibling_lyric("/Music/song.mp3", &[listed_entry("song.lrc", false)], &[]),
            None
        );
    }

    #[test]
    fn resolve_user_pick_is_exclusive() {
        let pick = StorageEntryLoc {
            storage_id: StorageId::wrap(1),
            path: "/Music/custom.lrc".to_string(),
        };
        let out = resolve_lyric_locs(&music_model(Some(pick.clone()), false), &exts(&["lrc"]));
        assert_eq!(out, vec![(pick, false)]);
    }

    #[test]
    fn resolve_detected_loc_falls_through_to_siblings() {
        let attached = StorageEntryLoc {
            storage_id: StorageId::wrap(1),
            path: "/Music/song.lrc".to_string(),
        };
        let out = resolve_lyric_locs(
            &music_model(Some(attached.clone()), true),
            &exts(&["lrc", "srt"]),
        );
        // Attached loc first (flagged as a fallback candidate — an
        // all-miss shows Missing, not Failed), then the remaining
        // sibling probes in probe order: per extension the full-name
        // form (`song.mp3.<ext>`) before the base form (`song.<ext>`);
        // the sibling identical to the attached loc is deduplicated
        // away.
        assert_eq!(
            out,
            vec![
                (attached, true),
                (
                    StorageEntryLoc {
                        storage_id: StorageId::wrap(1),
                        path: "/Music/song.mp3.lrc".to_string()
                    },
                    true
                ),
                (
                    StorageEntryLoc {
                        storage_id: StorageId::wrap(1),
                        path: "/Music/song.mp3.srt".to_string()
                    },
                    true
                ),
                (
                    StorageEntryLoc {
                        storage_id: StorageId::wrap(1),
                        path: "/Music/song.srt".to_string()
                    },
                    true
                ),
            ]
        );
    }

    #[test]
    fn resolve_plain_default_probes_siblings_only() {
        let out = resolve_lyric_locs(&music_model(None, true), &exts(&["lrc"]));
        assert_eq!(
            out,
            vec![
                (
                    StorageEntryLoc {
                        storage_id: StorageId::wrap(1),
                        path: "/Music/song.mp3.lrc".to_string()
                    },
                    true
                ),
                (
                    StorageEntryLoc {
                        storage_id: StorageId::wrap(1),
                        path: "/Music/song.lrc".to_string()
                    },
                    true
                ),
            ]
        );
        // Removed lyric (default cleared, no loc) resolves to nothing.
        assert!(resolve_lyric_locs(&music_model(None, false), &exts(&["lrc"])).is_empty());
    }

    // -- cover validation (see validate_cover_bytes / sniffs_as_image) ----

    fn png_magic() -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        v.extend_from_slice(&[0, 0, 0, 13, b'I', b'H', b'D', b'R']);
        v
    }

    #[test]
    fn cover_sniff_accepts_common_rasters() {
        assert!(sniffs_as_image(&[0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3])); // JPEG
        assert!(sniffs_as_image(&png_magic())); // PNG
        assert!(sniffs_as_image(b"GIF89a......")); // GIF
        assert!(sniffs_as_image(b"GIF87a......"));
        let mut webp = b"RIFF\x00\x00\x00\x00WEBPVP8 ".to_vec();
        webp.extend_from_slice(&[0; 8]);
        assert!(sniffs_as_image(&webp));
        let mut bmp = b"BM".to_vec();
        bmp.extend_from_slice(&[0; 12]);
        assert!(sniffs_as_image(&bmp));
        let mut heic = b"\x00\x00\x00\x18ftypheic".to_vec();
        heic.extend_from_slice(&[0; 8]);
        assert!(sniffs_as_image(&heic));
        let mut avif = b"\x00\x00\x00\x18ftypavif".to_vec();
        avif.extend_from_slice(&[0; 8]);
        assert!(sniffs_as_image(&avif));
    }

    #[test]
    fn cover_sniff_rejects_real_world_garbage() {
        // An ID3v2 APIC of picture type "linked" carries a URL string,
        // not pixels — symphonia hands it over verbatim.
        assert!(!sniffs_as_image(b"--> https://example.com/art.jpg"));
        // An HTML error page mis-served as art by a WebDAV server.
        assert!(!sniffs_as_image(
            b"<!DOCTYPE html><html><head><title>404</title></head></html>"
        ));
        assert!(!sniffs_as_image(b""));
        // Truncated magics.
        assert!(!sniffs_as_image(&[0xFF, 0xD8])); // JPEG preamble only
        assert!(!sniffs_as_image(&png_magic()[..4]));
        assert!(!sniffs_as_image(b"RIFF????WAVE")); // RIFF, not WEBP
        assert!(!sniffs_as_image(b"\x00\x00\x00\x18ftypisomxxxx")); // plain mp4
    }

    #[test]
    fn cover_validation_enforces_size_cap() {
        assert_eq!(validate_cover_bytes(&png_magic()), Ok(()));
        let big = vec![0u8; MAX_COVER_BYTES + 1];
        assert_eq!(validate_cover_bytes(&big), Err("too large"));
        assert_eq!(
            validate_cover_bytes(b"--> https://example.com/art.jpg"),
            Err("unrecognized image data")
        );
    }

    #[tokio::test]
    async fn update_music_cover_rejects_non_image_at_the_choke_point() {
        use crate::repositories::music::ArgDBAddMusic;
        use ease_order_key::OrderKey;

        let dir = tempfile::tempdir().unwrap();
        let cx = BackendContext::new();
        cx.database_server()
            .init(dir.path().to_str().unwrap().to_string())
            .await
            .unwrap();
        let (id, _) = cx
            .database_server()
            .add_music_impl(
                ArgDBAddMusic {
                    loc: StorageEntryLoc {
                        storage_id: StorageId::wrap(1),
                        path: "/Music/song.mp3".to_string(),
                    },
                    title: "song".to_string(),
                    lyric: None,
                },
                OrderKey::default(),
            )
            .await
            .unwrap();

        // Garbage: best-effort Ok, but nothing written.
        update_music_cover(
            &cx,
            ArgUpdateMusicCover {
                id,
                cover: b"--> https://example.com/art.jpg".to_vec(),
            },
        )
        .await
        .unwrap();
        let m = cx.database_server().load_music(id).await.unwrap().unwrap();
        assert!(m.cover.is_none(), "a non-image cover must not be stored");

        // A real raster passes and lands in the DB.
        update_music_cover(&cx, ArgUpdateMusicCover { id, cover: png_magic() })
            .await
            .unwrap();
        let m = cx.database_server().load_music(id).await.unwrap().unwrap();
        assert!(m.cover.is_some(), "a sniffable cover must be stored");
    }

    // -- artist extraction (see extract_artist_tag / update_music_artist) --

    fn tag(key: &str, value: &str) -> cantode::Tag {
        cantode::Tag {
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn artist_tag_prefers_artist_over_album_artist() {
        let tags = vec![
            tag("Album", "Greatest Hits"),
            tag("AlbumArtist", "Various Artists"),
            tag("Artist", "Alice"),
        ];
        assert_eq!(extract_artist_tag(&tags).as_deref(), Some("Alice"));
    }

    #[test]
    fn artist_tag_falls_back_to_album_artist() {
        // Compilations often carry only the album-level artist.
        let tags = vec![tag("AlbumArtist", "Various Artists")];
        assert_eq!(
            extract_artist_tag(&tags).as_deref(),
            Some("Various Artists")
        );
    }

    #[test]
    fn artist_tag_matches_keys_case_insensitively() {
        let tags = vec![tag("ARTIST", "Alice")];
        assert_eq!(extract_artist_tag(&tags).as_deref(), Some("Alice"));
    }

    #[test]
    fn artist_tag_joins_multiple_values() {
        // FLAC Vorbis comments repeat the field per value.
        let tags = vec![tag("Artist", "Alice"), tag("Artist", "Bob")];
        assert_eq!(extract_artist_tag(&tags).as_deref(), Some("Alice, Bob"));
    }

    #[test]
    fn artist_tag_ignores_blank_values_and_unknown_keys() {
        assert_eq!(extract_artist_tag(&[]), None);
        assert_eq!(extract_artist_tag(&[tag("Title", "Song")]), None);
        // Whitespace-only and empty values never count as found.
        assert_eq!(extract_artist_tag(&[tag("Artist", "   ")]), None);
        // A blank Artist falls through to a real AlbumArtist.
        let tags = vec![tag("Artist", ""), tag("AlbumArtist", " Various ")];
        assert_eq!(extract_artist_tag(&tags).as_deref(), Some("Various"));
    }

    #[tokio::test]
    async fn update_music_artist_never_clobbers_and_drops_blanks() {
        use crate::repositories::music::ArgDBAddMusic;
        use ease_order_key::OrderKey;

        let dir = tempfile::tempdir().unwrap();
        let cx = BackendContext::new();
        cx.database_server()
            .init(dir.path().to_str().unwrap().to_string())
            .await
            .unwrap();
        let (id, _) = cx
            .database_server()
            .add_music_impl(
                ArgDBAddMusic {
                    loc: StorageEntryLoc {
                        storage_id: StorageId::wrap(1),
                        path: "/Music/song.mp3".to_string(),
                    },
                    title: "song".to_string(),
                    lyric: None,
                },
                OrderKey::default(),
            )
            .await
            .unwrap();

        // Blank probes are dropped silently.
        update_music_artist(&cx, ArgUpdateMusicArtist { id, artist: "  ".to_string() })
            .await
            .unwrap();
        let m = cx.database_server().load_music(id).await.unwrap().unwrap();
        assert_eq!(m.artist, "");

        // The first real value lands.
        update_music_artist(&cx, ArgUpdateMusicArtist { id, artist: " Alice ".to_string() })
            .await
            .unwrap();
        let m = cx.database_server().load_music(id).await.unwrap().unwrap();
        assert_eq!(m.artist, "Alice");

        // ... and a later probe never clobbers it.
        update_music_artist(&cx, ArgUpdateMusicArtist { id, artist: "Bob".to_string() })
            .await
            .unwrap();
        let m = cx.database_server().load_music(id).await.unwrap().unwrap();
        assert_eq!(m.artist, "Alice");
    }

    // -- embedded lyrics (see extract_lyric_tag / embedded_music_lyric) --

    #[test]
    fn lyric_tag_extracts_first_non_empty_trimmed_value() {
        let tags = vec![
            tag("Title", "Song"),
            tag("Lyrics", ""),
            tag("Lyrics", "  \n "),
            tag("Lyrics", " la la "),
        ];
        assert_eq!(extract_lyric_tag(&tags).as_deref(), Some("la la"));
    }

    #[test]
    fn lyric_tag_matches_keys_case_insensitively_and_ignores_unknown() {
        assert_eq!(extract_lyric_tag(&[]), None);
        assert_eq!(extract_lyric_tag(&[tag("Title", "Song")]), None);
        // symphonia maps USLT / Vorbis `lyrics` / MP4 `©lyr` here.
        assert_eq!(extract_lyric_tag(&[tag("Lyrics", "x")]).as_deref(), Some("x"));
    }

    #[test]
    fn lyric_tag_drops_oversized_text() {
        let big = "a".repeat(MAX_LYRIC_BYTES + 1);
        assert_eq!(extract_lyric_tag(&[tag("Lyrics", &big)]), None);
    }

    #[test]
    fn unsynced_text_splits_trims_and_flags() {
        let lyrics = unsynced_lyrics_from_text("\r\n First  \n\nsecond\rthird\n");
        assert!(!lyrics.synced);
        assert_eq!(lyrics.metdata, Default::default());
        assert!(
            lyrics.lines.iter().all(|l| l.duration.is_zero()),
            "unsynced lines all sit at t=0"
        );
        let texts: Vec<&str> = lyrics.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, vec!["First", "second", "third"]);
    }

    #[tokio::test]
    async fn load_music_lyric_embedded_fallback_and_explicit_pick_precedence() {
        use crate::repositories::music::ArgDBAddMusic;
        use ease_order_key::OrderKey;

        let dir = tempfile::tempdir().unwrap();
        let cx = BackendContext::new();
        cx.database_server()
            .init(dir.path().to_str().unwrap().to_string())
            .await
            .unwrap();
        let add = |cx: &BackendContext, path: &str| {
            let cx = cx.clone();
            let path = path.to_string();
            async move {
                let (id, _) = cx
                    .database_server()
                    .add_music_impl(
                        ArgDBAddMusic {
                            loc: StorageEntryLoc {
                                storage_id: StorageId::wrap(1),
                                path,
                            },
                            title: "song".to_string(),
                            lyric: None,
                        },
                        OrderKey::default(),
                    )
                    .await
                    .unwrap();
                id
            }
        };

        // No sidecar candidate + no embedded tag -> None (MISSING pane).
        let id_none = add(&cx, "/Music/none.mp3").await;
        assert!(load_music_lyric(&cx, id_none).await.unwrap().is_none());

        // Embedded tag text, no sidecar candidate -> LOADED, unsynced,
        // located on the audio file itself. (No parser plugins live in
        // this test context, so LRC-shaped text also lands here — see
        // the plugins' own selftests for the synced parse.)
        let id_emb = add(&cx, "/Music/embedded.flac").await;
        cx.database_server()
            .update_music_embedded_lyric(
                id_emb,
                "First line\n\n  Second line  \n".to_string(),
            )
            .await
            .unwrap();
        let lyric = load_music_lyric(&cx, id_emb).await.unwrap().unwrap();
        assert_eq!(lyric.loaded_state, LyricLoadState::Loaded);
        assert_eq!(lyric.loc.path, "/Music/embedded.flac");
        assert!(!lyric.data.synced);
        let texts: Vec<&str> = lyric.data.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, vec!["First line", "Second line"]);

        // An explicit pick that fails to FETCH stays terminal (Failed) —
        // embedded text must not silently paper over a broken user pick.
        let id_pick = add(&cx, "/Music/picked.mp3").await;
        let pick_loc = StorageEntryLoc {
            storage_id: StorageId::wrap(1),
            path: "/Music/picked.lrc".to_string(),
        };
        cx.database_server()
            .update_music_lyric(id_pick, Some(pick_loc))
            .await
            .unwrap();
        cx.database_server()
            .update_music_embedded_lyric(id_pick, "embedded".to_string())
            .await
            .unwrap();
        let lyric = load_music_lyric(&cx, id_pick).await.unwrap().unwrap();
        assert_eq!(lyric.loaded_state, LyricLoadState::Failed);
    }
}

/// DB-only music fetch. The lyric arrives as a [`LyricLoadState::Loading`]
/// placeholder (loc resolved, `data` empty) — the bytes are fetched
/// separately via [`load_music_lyric`]. `music.get` used to await the
/// lyric's network round trip inline, which gated track switches (and
/// the old track's stop) on storage-plugin latency.
pub(crate) async fn get_music(cx: &BackendContext, id: MusicId) -> BResult<Option<Music>> {
    let model = cx.database_server().load_music(id).await?;
    let Some(model) = model else {
        return Ok(None);
    };

    let meta = build_music_meta(model.clone());
    let cover = if model.cover.is_some() {
        Some(DataSourceKey::Cover { id: model.id })
    } else {
        Default::default()
    };
    let lyric = resolve_lyric_locs(&model, &cx.plugin_manager().lyric_sibling_extensions())
        .into_iter()
        .next()
        .map(|(loc, _)| MusicLyric {
            loc,
            data: Default::default(),
            loaded_state: LyricLoadState::Loading,
        })
        .or_else(|| {
            // No sidecar candidate at all — but the play-time writeback
            // may already have captured embedded tag lyrics. The pane
            // must show its LOADING spinner (not flash MISSING) until
            // `music.loadLyric` resolves the tag text.
            model
                .embedded_lyric
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|_| MusicLyric {
                    loc: model.loc.clone(),
                    data: Default::default(),
                    loaded_state: LyricLoadState::Loading,
                })
        });
    let loc = model.loc;

    Ok(Some(Music {
        meta,
        loc,
        cover,
        lyric,
    }))
}

/// Fetch + parse the lyric for a music over the storage seam — the
/// network-bound half that [`get_music`] no longer performs inline. The
/// player calls it right after the (instant) track switch and patches the
/// result into the current music. Returns `Ok(None)` when the music (or
/// its resolved lyric location) doesn't exist.
pub(crate) async fn load_music_lyric(
    cx: &BackendContext,
    id: MusicId,
) -> BResult<Option<MusicLyric>> {
    let model = cx.database_server().load_music(id).await?;
    let Some(model) = model else {
        return Ok(None);
    };
    let candidates = resolve_lyric_locs(&model, &cx.plugin_manager().lyric_sibling_extensions());
    if let Some(lyric) = load_lyric(cx, candidates).await {
        // `Missing` = every fallback candidate missed — embedded tag
        // lyrics may still win below. `Failed` is terminal: the file the
        // user explicitly picked exists but nothing can parse it, and
        // silently swapping in other text would hide that.
        if lyric.loaded_state != LyricLoadState::Missing {
            return Ok(Some(lyric));
        }
    }
    Ok(embedded_music_lyric(cx, &model).await)
}

pub(crate) async fn get_music_abstract(
    cx: &BackendContext,
    id: MusicId,
) -> BResult<Option<MusicAbstract>> {
    let model = cx.database_server().load_music(id).await?;
    if model.is_none() {
        return Ok(None);
    }

    let model = model.unwrap();
    let meta = build_music_meta(model.clone());
    let cover = if model.cover.is_none() {
        Default::default()
    } else {
        Some(DataSourceKey::Cover { id: model.id })
    };

    let abstract_music = MusicAbstract { cover, meta };
    Ok(Some(abstract_music))
}
