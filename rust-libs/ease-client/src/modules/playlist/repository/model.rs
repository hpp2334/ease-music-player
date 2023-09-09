use std::{collections::HashMap, time::Duration};

use crate::{
    modules::{
        music::{repository::MusicDuration, Music},
        MusicId,
    },
    utils::cmp_name_smartly,
};

use super::super::typ::*;
use getset::Getters;
use misty_vm::resources::MistyResourceHandle;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PlaylistModel {
    pub id: PlaylistId,
    pub title: String,
    pub created_time: i64,
    #[serde(with = "serde_bytes")]
    pub picture: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PlaylistMusicModel {
    pub playlist_id: PlaylistId,
    pub music_id: MusicId,
}

#[derive(Debug, Clone, Getters)]
pub struct PlaylistMusic {
    pub(super) model: PlaylistMusicModel,
    #[getset(get = "pub")]
    pub(super) title: String,
    #[getset(get = "pub")]
    pub(super) duration: Option<MusicDuration>,
}

#[derive(Debug, Clone, Getters)]
pub struct Playlist {
    pub(super) model: PlaylistModel,
    #[getset(get = "pub")]
    pub(super) musics: HashMap<MusicId, PlaylistMusic>,
    pub(super) ordered_music_ids: Vec<MusicId>,
    #[getset(get = "pub")]
    pub(super) self_picture: Option<MistyResourceHandle>,
    #[getset(get = "pub")]
    pub(super) picture_owning_music: Option<MusicId>,
    #[getset(get = "pub")]
    pub(super) first_picture_in_musics: Option<MistyResourceHandle>,
}

pub(super) fn recalc_playlist_ordered_music_ids(playlist: &mut Playlist) {
    let mut playlist_musics: Vec<PlaylistMusic> =
        playlist.musics().iter().map(|(_, m)| m.clone()).collect();
    playlist_musics.sort_by(|lhs, rhs| cmp_name_smartly(&lhs.title(), &rhs.title()));
    playlist.ordered_music_ids = playlist_musics.iter().map(|m| m.music_id()).collect();
}

impl PlaylistMusic {
    pub fn music_id(&self) -> MusicId {
        self.model.music_id
    }

    pub fn playlist_id(&self) -> PlaylistId {
        self.model.playlist_id
    }
}

impl Playlist {
    pub fn id(&self) -> PlaylistId {
        self.model.id
    }

    pub fn created_time(&self) -> Duration {
        Duration::from_millis(self.model.created_time.try_into().unwrap())
    }

    pub fn title(&self) -> &str {
        self.model.title.as_str()
    }

    pub fn duration(&self) -> Option<MusicDuration> {
        let duration = self
            .musics
            .iter()
            .fold(Some(Duration::ZERO), |prev, (_, music)| {
                if prev.is_none() || music.duration.is_none() {
                    None
                } else {
                    Some(prev.unwrap() + **&music.duration.unwrap())
                }
            })
            .map(|duration| MusicDuration::new(duration));
        duration
    }

    pub fn get_ordered_musics(&self) -> Vec<PlaylistMusic> {
        self.ordered_music_ids
            .iter()
            .map(|id| self.musics.get(id).unwrap().clone())
            .collect()
    }

    pub fn set_title(&mut self, title: String) {
        self.model.title = title;
    }

    pub fn set_self_picture(&mut self, self_picture: Option<MistyResourceHandle>) {
        self.self_picture = self_picture;
    }

    pub fn add_musics(&mut self, musics: Vec<Music>) {
        for music in musics.into_iter() {
            self.musics.insert(
                music.id(),
                PlaylistMusic {
                    model: PlaylistMusicModel {
                        playlist_id: self.model.id,
                        music_id: music.id(),
                    },
                    title: music.title().to_string(),
                    duration: music.duration(),
                },
            );
        }
        recalc_playlist_ordered_music_ids(self);
    }

    pub fn remove_music(&mut self, music_id: MusicId) {
        self.musics.remove(&music_id);
        recalc_playlist_ordered_music_ids(self);
    }

    pub fn set_music_duration(&mut self, music_id: MusicId, duration: Option<MusicDuration>) {
        let m = self.musics.get_mut(&music_id).unwrap();
        m.duration = duration;
    }

    pub fn set_preferred_music_cover(
        &mut self,
        music_id: Option<MusicId>,
        cover: Option<MistyResourceHandle>,
    ) {
        self.first_picture_in_musics = cover;
        self.picture_owning_music = music_id;
    }
}
