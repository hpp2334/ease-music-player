use std::{cmp::Ordering, sync::RwLock, time::Duration};


use futures_util::future::BoxFuture;
use reqwest::header::HeaderValue;
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    env::EASEM_ONEDRIVE_ID, Entry, StorageBackend, StorageBackendError, StorageBackendResult, StreamFile,
};

pub struct BuildOneDriveArg {
    pub code: String,
}

struct Auth {
    access_token: String,
    refresh_token: String,
}

pub struct OneDriveBackend {
    refresh_token: String,
    auth: tokio::sync::RwLock<Option<Auth>>
}


mod onedrive_types {
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    pub struct RedeemCodeResp {
        pub access_token: String,
        pub refresh_token: String,
    }
    #[derive(Debug, Deserialize)]
    pub struct ListItemResponse {
        pub value: Vec<ListItem>,
    }
    
    #[derive(Debug, Deserialize)]
    pub struct ListItem {
        pub name: String,
        #[serde(flatten)]
        pub kind: ListItemKind,
    }
    
    #[derive(Debug, Deserialize)]
    #[serde(untagged)]
    pub enum ListItemKind {
        File {
            size: u64,
            #[serde(rename = "file")]
            _file: ListFileMetadata
        },
        Folder {
            #[serde(rename = "folder")]
            _folder: ListFolderMetadata,
        },
    }
    
    #[derive(Debug, Deserialize)]
    pub struct ListFolderMetadata {
        #[serde(rename = "childCount")]
        pub _child_count: u64,
    }

    #[derive(Debug, Deserialize)]
    pub struct ListFileMetadata {
        #[serde(rename = "mimeType")]
        pub _mime_type: String
    }
}


const ONEDRIVE_ROOT_API: &str = "https://graph.microsoft.com/v1.0/me/drive";
const ONEDRIVE_API_BASE: &str = "https://login.microsoftonline.com/common/oauth2/v2.0";
const ONEDRIVE_REDIRECT_URI: &str = "easem://oauth2redirect/";  


fn is_auth_error<T>(r: &StorageBackendResult<T>) -> bool {
    if let Err(e) = r {
        if let StorageBackendError::RequestFail(e) = e {
            if let Some(StatusCode::UNAUTHORIZED) = e.status() {
                return true;
            }
        }
    }
    return false;
}

fn build_client() -> StorageBackendResult<reqwest::Client> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .no_proxy()
        .build()?;
    Ok(client)
}

async fn refresh_token_by_code_impl(code: String) -> StorageBackendResult<Auth> {      
    let client_id = EASEM_ONEDRIVE_ID;
    let body = 
        format!("client_id={client_id}&redirect_uri={ONEDRIVE_REDIRECT_URI}&code={code}&grant_type=authorization_code");

    let resp = build_client()?
        .request(reqwest::Method::POST, format!("{ONEDRIVE_API_BASE}/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?;
    let resp_text = resp.text().await?;
    let value = serde_json::from_str::<onedrive_types::RedeemCodeResp>(&resp_text)?;
    Ok(Auth {
        access_token: value.access_token,
        refresh_token: value.refresh_token,
    })
}

impl OneDriveBackend {
    pub fn new(arg: BuildOneDriveArg) -> Self {
        Self { refresh_token: arg.code, auth: Default::default() }
    }

    async fn build_base_header_map(&self) -> reqwest::header::HeaderMap {
        let mut header_map = reqwest::header::HeaderMap::new();
        {
            let r = self.auth.read().await;
            if let Some(auth) = r.as_ref() {
                header_map.append(
                    reqwest::header::AUTHORIZATION,
                    HeaderValue::from_str(format!("bearer {}", auth.access_token).as_str()).unwrap(),
                );
            }
        }
        return header_map;
    }

    async fn try_ensure_refresh_token_by_refresh_token(&self) -> StorageBackendResult<()> {
        let mut w= self.auth.write().await;
        if w.is_none() {
            self.refresh_token_by_refresh_token_impl(&mut w).await?;
        }
        Ok(())
    }

    async fn refresh_token_by_refresh_token_impl(&self, w: &mut Option<Auth>) -> StorageBackendResult<()> {
        let client_id = EASEM_ONEDRIVE_ID;
        let refresh_token = self.refresh_token.clone();
        let body = 
            format!("client_id={client_id}&redirect_uri={ONEDRIVE_REDIRECT_URI}&refresh_token={refresh_token}&grant_type=refresh_token");

        let resp = self
            .build_client()?
            .request(reqwest::Method::POST, format!("{ONEDRIVE_API_BASE}/token"))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await?;
        let resp_text = resp.text().await?;
        let value = serde_json::from_str::<onedrive_types::RedeemCodeResp>(&resp_text)?;

        *w = Some(Auth {
            access_token: value.access_token,
            refresh_token: value.refresh_token,
        });
        Ok(())
    }

    async fn refresh_token_by_refresh_token(&self) -> StorageBackendResult<()> {
        let mut w= self.auth.write().await;
        self.refresh_token_by_refresh_token_impl(&mut w).await?;
        Ok(())
    }

    async fn list_core(&self, dir: &str) -> StorageBackendResult<reqwest::Response> {
        let subdir = if dir == "/" {
            "/root/children".to_string()
        } else {
            ("/root:".to_string() + dir + ":/children").to_string()
        };
        let _url = ONEDRIVE_ROOT_API.to_string() + subdir.as_str();
        let url = reqwest::Url::parse(_url.as_str())
            .map_err(|e| StorageBackendError::UrlParseError(e.to_string()))?;
        let base_headers = self.build_base_header_map().await;
        
        let resp = self
            .build_client()?
            .request(reqwest::Method::GET, url.clone())
            .headers(base_headers)
            .send()
            .await?;

        Ok(resp)
    }

    async fn list_impl(&self, dir: &str) -> StorageBackendResult<Vec<Entry>> {
        let resp = self.list_core(dir).await?.error_for_status()?;
        let text: String = resp.text().await?;
        let obj: onedrive_types::ListItemResponse = serde_json::from_str(&text)?;

        let mut ret: Vec<Entry> = Default::default();
        for item in obj.value {
            let name = item.name;
            let path = dir.to_string() + "/" + name.as_str();
            match item.kind {
                onedrive_types::ListItemKind::File { size, .. } => {
                    ret.push(Entry {
                        name,
                        path,
                        size: Some(size as usize),
                        is_dir: false,
                    });
                }
                onedrive_types::ListItemKind::Folder { .. } => {
                    ret.push(Entry {
                        name,
                        path,
                        size: None,
                        is_dir: true,
                    });
                }
            }
        }

        ret.sort_by(|lhs, rhs| {
            if lhs.is_dir ^ rhs.is_dir {
                if lhs.is_dir {
                    return Ordering::Less;
                } else {
                    return Ordering::Greater;
                }
            }
            if lhs.path < rhs.path {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        });

        Ok(ret)
    }

    async fn list_with_retry_impl(&self, dir: String) -> StorageBackendResult<Vec<Entry>> {
        self.try_ensure_refresh_token_by_refresh_token().await?;
        let r = self.list_impl(dir.as_str()).await;
        if !is_auth_error(&r) {
            return r;
        }
        self.refresh_token_by_refresh_token().await?;
        return self.list_impl(dir.as_str()).await;
    }

    async fn get_impl(&self, p: &str) -> StorageBackendResult<StreamFile> {
        let _url = ONEDRIVE_ROOT_API.to_string() + "/root:" + p + ":/content";
        let url = reqwest::Url::parse(_url.as_str())
            .map_err(|e| StorageBackendError::UrlParseError(e.to_string()))?;
        let base_headers = self.build_base_header_map().await;

        let resp = self
            .build_client()?
            .get(url.clone())
            .headers(base_headers)
            .send()
            .await?;
        let res = resp.error_for_status().map(|resp| StreamFile::new(resp))?;
        Ok(res)
    }

    async fn get_with_retry_impl(&self, p: String) -> StorageBackendResult<StreamFile> {
        self.try_ensure_refresh_token_by_refresh_token().await?;
        let r = self.get_impl(p.as_str()).await;
        if !is_auth_error(&r) {
            return r;
        }
        self.refresh_token_by_refresh_token().await?;
        return self.get_impl(p.as_str()).await;
    }

    fn build_client(&self) -> StorageBackendResult<reqwest::Client> {
        build_client()
    }
}

impl StorageBackend for OneDriveBackend {
    fn list(&self, dir: String) -> BoxFuture<StorageBackendResult<Vec<Entry>>> {
        Box::pin(self.list_with_retry_impl(dir))
    }

    fn get(&self, p: String) -> BoxFuture<StorageBackendResult<StreamFile>> {
        Box::pin(self.get_with_retry_impl(p))
    }
}

impl OneDriveBackend {
    pub async fn request_refresh_token(code: String) -> StorageBackendResult<String> {
        let authed = refresh_token_by_code_impl(code).await?;
        Ok(authed.refresh_token)
    }
}