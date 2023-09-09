use std::collections::HashSet;

use misty_vm::views::MistyViewModelManagerBuilder;

use crate::{
    core_views::RootViewModelState,
    modules::{MusicId, PlaylistId},
    utils::{cmp_name_smartly, decode_component_or_origin},
};

use super::{
    service::{
        entry_can_check, get_entry_type, CurrentStorageState, EditStorageState, StoragesState,
    },
    typ::*,
    StorageType,
};

fn storage_list_view_model(state: &StoragesState, root: &mut RootViewModelState) {
    let items: Vec<VStorageListItem> = {
        state
            .storage_infos
            .iter()
            .map(|info| VStorageListItem {
                storage_id: info.id.clone(),
                name: info.name.clone(),
                sub_title: info.addr.clone(),
                typ: info.typ.clone(),
            })
            .collect()
    };
    let list = VStorageListState { items };
    root.storage_list = Some(list);
}

fn current_storage_entries_view_model(
    (state, storages_state): (&CurrentStorageState, &StoragesState),
    root: &mut RootViewModelState,
) {
    let current_storage = state.clone();
    if current_storage.current_storage_id.is_none() {
        return;
    }
    let mut splited_path_items: Vec<VSplitPathItem> = Default::default();

    let current_path = current_storage.current_path;
    let paths: Vec<String> = current_path
        .split('/')
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect();

    for i in 0..paths.len() {
        let path = "/".to_string() + &paths[0..=i].join("/");
        splited_path_items.push(VSplitPathItem {
            path,
            name: decode_component_or_origin(paths[i].to_string()),
        });
    }

    let mut entries: Vec<VCurrentStorageEntry> = Default::default();
    let mut selected_count = 0;
    for entry in current_storage.entries.iter() {
        let name = entry.name.clone();
        let entry_typ = get_entry_type(&entry);
        let checked = current_storage.checked_entries_path.contains(&entry.path);

        if checked {
            selected_count += 1;
        }

        entries.push(VCurrentStorageEntry {
            path: entry.path.clone(),
            name,
            is_folder: entry.is_dir,
            can_check: entry_can_check(entry, current_storage.import_type),
            checked: current_storage.checked_entries_path.contains(&entry.path),
            entry_typ,
        });
    }

    entries.sort_by(|lhs, rhs| {
        if lhs.is_folder ^ rhs.is_folder {
            return if lhs.is_folder {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        cmp_name_smartly(&lhs.name, &rhs.name)
    });

    let storage_items: Vec<VCurrentStorageEntriesStateStorageItem> = storages_state
        .storage_infos
        .iter()
        .map(|v| VCurrentStorageEntriesStateStorageItem {
            id: v.id.clone(),
            name: v.name.clone(),
            subtitle: v.addr.clone(),
            selected: Some(v.id.clone()) == state.current_storage_id,
            is_local: v.typ == StorageType::Local,
        })
        .collect();

    let any_can_check = entries
        .iter()
        .fold(false, |prev, curr| curr.can_check || prev);

    let disabled_toggle_all = !any_can_check;

    let current_storage_entries = VCurrentStorageEntriesState {
        import_type: current_storage.import_type,
        current_storage_id: current_storage.current_storage_id.clone(),
        entries,
        selected_count,
        split_paths: splited_path_items,
        current_path,
        state_type: state.state_type.clone(),
        storage_items,
        disabled_toggle_all,
    };
    root.current_storage_entries = Some(current_storage_entries);
}

fn edit_storage_view_model(state: &EditStorageState, root: &mut RootViewModelState) {
    let mut music_id_visited: HashSet<MusicId> = Default::default();
    let mut playlist_id_visited: HashSet<PlaylistId> = Default::default();

    for (playlist_id, music_ids) in state.tuple_maps.iter() {
        playlist_id_visited.insert(*playlist_id);
        for music_id in music_ids.iter() {
            music_id_visited.insert(*music_id);
        }
    }

    root.edit_storage = Some(VEditStorageState {
        is_created: state.is_create,
        title: state.title.clone(),
        info: state.info.clone(),
        test: state.test,
        update_signal: state.update_signal,
        music_count: music_id_visited.len(),
        playlist_count: playlist_id_visited.len(),
    });
}

pub fn register_storage_viewmodels(
    builder: MistyViewModelManagerBuilder<RootViewModelState>,
) -> MistyViewModelManagerBuilder<RootViewModelState> {
    builder
        .register(storage_list_view_model)
        .register(current_storage_entries_view_model)
        .register(edit_storage_view_model)
}
