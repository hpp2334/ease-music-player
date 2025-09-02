use redb::{ReadTransaction, ReadableMultimapTable};

use crate::v2::*;
use std::sync::Arc;

fn load_music_impl(db: &ReadTransaction, id: MusicId) -> anyhow::Result<Option<MusicModel>> {
    let table_music = db.open_table(TABLE_MUSIC)?;
    let model = table_music.get(id)?.map(|v| v.value()).clone();

    Ok(model)
}

pub fn upgrade_v1_to_v2(database: &Arc<redb::Database>) -> anyhow::Result<()> {
    let db = database.begin_write()?;
    let rdb = database.begin_read()?;

    {
        let mut table_storage_musics = db.open_multimap_table(TABLE_STORAGE_MUSIC)?;

        let mut to_remove: Vec<(StorageId, MusicId)> = Default::default();

        for music_iter in table_storage_musics.iter()? {
            let (storage_id, mut music_iter) = music_iter?;
            let storage_id = storage_id.value();
            while let Some(v) = music_iter.next() {
                let id = v?.value();
                {
                    let m = load_music_impl(&rdb, id)?;
                    if m.is_none() {
                        to_remove.push((storage_id, id));
                    }
                }
            }
            drop(music_iter);
        }

        for (storage_id, music_id) in to_remove {
            table_storage_musics.remove(storage_id, music_id)?;
        }
        tracing::info!("v1 -> v2: finish to remove invalid music");
    }

    {
        let mut t = db.open_table(TABLE_SCHEMA_VERSION)?;
        t.insert((), 2)?;
    }
    db.commit()?;
    tracing::info!("v1 -> v2: finish all");

    Ok(())
}
