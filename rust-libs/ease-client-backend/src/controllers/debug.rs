use std::sync::Arc;

use crate::{
    error::{BError, BResult},
    infra::logs_dir,
    Backend,
};

#[derive(uniffi::Record)]
pub struct ListLogFile {
    name: String,
    path: String,
}

#[derive(uniffi::Record)]
pub struct ListLogFiles {
    files: Vec<ListLogFile>,
}

#[uniffi::export]
pub fn cts_list_log_files(cx: Arc<Backend>) -> BResult<ListLogFiles> {
    let dir = std::fs::read_dir(logs_dir(&cx.arg.app_document_dir))?;
    let mut files = dir
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            let path = entry.path().to_str()?.to_string();
            Some((name, path))
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    let files = files
        .into_iter()
        .map(|(name, path)| ListLogFile { name, path })
        .collect();
    Ok(ListLogFiles { files })
}

#[uniffi::export]
pub fn cts_trigger_error(_cx: Arc<Backend>) -> BResult<()> {
    Err(BError::CustomError {
        message: "cts_trigger_error".to_string(),
    })
}

#[uniffi::export]
pub async fn ct_trigger_error(_cx: Arc<Backend>) -> BResult<()> {
    Err(BError::CustomError {
        message: "ct_trigger_error".to_string(),
    })
}

#[uniffi::export]
pub fn cts_trigger_panic(_cx: Arc<Backend>) -> BResult<()> {
    panic!("cts_trigger_panic");
}
