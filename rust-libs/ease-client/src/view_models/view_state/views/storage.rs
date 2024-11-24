use ease_client_shared::backends::storage::{
    ArgUpsertStorage, CurrentStorageImportType, CurrentStorageStateType, Storage,
    StorageConnectionTestResult, StorageEntryType, StorageId, StorageType,
};
use serde::Serialize;

use crate::{
    utils::{cmp_name_smartly::cmp_name_smartly, common::decode_component_or_origin},
    view_models::storage::{
        import::{entry_can_check, get_entry_type},
        state::{AllStorageState, CurrentStorageState, EditStorageFormValidated, EditStorageState},
    },
};

use super::models::RootViewModelState;

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VStorageListItem {
    pub storage_id: StorageId,
    pub name: String,
    pub sub_title: String,
    pub typ: StorageType,
}

#[derive(Debug, Clone, Default, Serialize, uniffi::Record)]
pub struct VStorageListState {
    pub items: Vec<VStorageListItem>,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VCurrentStorageEntry {
    pub path: String,
    pub name: String,
    pub is_folder: bool,
    pub can_check: bool,
    pub checked: bool,
    pub entry_typ: StorageEntryType,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VSplitPathItem {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VCurrentStorageEntriesStateStorageItem {
    pub id: StorageId,
    pub name: String,
    pub subtitle: String,
    pub selected: bool,
    pub is_local: bool,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VCurrentStorageEntriesState {
    pub import_type: CurrentStorageImportType,
    pub state_type: CurrentStorageStateType,
    pub current_storage_id: Option<StorageId>,
    pub storage_items: Vec<VCurrentStorageEntriesStateStorageItem>,
    pub entries: Vec<VCurrentStorageEntry>,
    pub selected_count: i32,
    pub split_paths: Vec<VSplitPathItem>,
    pub current_path: String,
    pub disabled_toggle_all: bool,
    pub can_undo: bool,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VEditStorageState {
    pub is_created: bool,
    pub title: String,
    pub info: ArgUpsertStorage,
    pub validated: EditStorageFormValidated,
    pub test: StorageConnectionTestResult,
    pub music_count: u32,
    pub playlist_count: u32,
}

fn resolve_storage_name(storage: &Storage) -> String {
    if !storage.alias.is_empty() {
        storage.alias.clone()
    } else {
        storage.addr.clone()
    }
}

fn resolve_upsert_storage_name(storage: &ArgUpsertStorage) -> String {
    if !storage.alias.is_empty() {
        storage.alias.clone()
    } else {
        storage.addr.clone()
    }
}

pub(crate) fn storage_list_vs(state: &AllStorageState, root: &mut RootViewModelState) {
    let items: Vec<VStorageListItem> = {
        state
            .storage_ids
            .iter()
            .map(|id| {
                let info = state.storages.get(id).unwrap();

                VStorageListItem {
                    storage_id: *id,
                    name: resolve_storage_name(info),
                    sub_title: info.addr.clone(),
                    typ: info.typ.clone(),
                }
            })
            .collect()
    };

    let list = VStorageListState { items };
    root.storage_list = Some(list);
}

pub(crate) fn current_storage_entries_vs(
    (state, storages_state): (&CurrentStorageState, &AllStorageState),
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
        .storage_ids
        .iter()
        .map(|id| {
            let v = storages_state.storages.get(id).unwrap();
            VCurrentStorageEntriesStateStorageItem {
                id: v.id.clone(),
                name: resolve_storage_name(v),
                subtitle: v.addr.clone(),
                selected: Some(v.id.clone()) == state.current_storage_id,
                is_local: v.typ == StorageType::Local,
            }
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
        can_undo: !state.undo_stack.is_empty(),
    };
    root.current_storage_entries = Some(current_storage_entries);
}

pub(crate) fn edit_storage_vs(state: &EditStorageState, root: &mut RootViewModelState) {
    root.edit_storage = Some(VEditStorageState {
        is_created: state.is_create,
        title: resolve_upsert_storage_name(&state.info),
        info: state.info.clone(),
        validated: state.validated.clone(),
        test: state.test,
        music_count: state.music_count,
        playlist_count: state.playlist_count,
    });
}
