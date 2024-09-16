use crate::backend::{BackendResult, Entry, StorageBackend, StreamFile};
use crate::BackendError;

use async_trait::async_trait;

use reqwest::header::HeaderValue;
use reqwest::StatusCode;

use std::cmp::Ordering;

use std::sync::RwLock;
use std::time::Duration;

pub struct Webdav {
    addr: String,
    username: String,
    password: String,
    _is_anonymous: bool,
    last_www_authenticate: RwLock<Option<String>>,
    connect_timeout: Duration,
}

pub struct BuildWebdavArg {
    pub addr: String,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub connect_timeout: Duration,
}

mod webdav_list_types {
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    pub struct Collection {}

    #[derive(Deserialize, Debug)]
    pub struct ResourceType {
        pub collection: Option<Collection>,
    }

    #[derive(Deserialize, Debug)]
    pub struct Prop {
        pub displayname: Option<String>,
        pub resourcetype: ResourceType,
        pub getcontentlength: Option<usize>,
    }

    #[derive(Deserialize, Debug)]
    pub struct PropStat {
        pub prop: Prop,
    }

    #[derive(Deserialize, Debug)]
    pub struct Response {
        pub href: String,
        pub propstat: PropStat,
    }

    #[derive(Deserialize, Debug)]
    pub struct Root {
        pub response: Vec<Response>,
    }
}

fn build_authorization_header_value(
    www_authenticate: &str,
    username: &str,
    password: &str,
    uri: &str,
    method: &str,
) -> Option<String> {
    if www_authenticate == "" {
        return None;
    }
    let mut pw_client = http_auth::PasswordClient::try_from(www_authenticate).unwrap();
    let ret = pw_client
        .respond(&http_auth::PasswordParams {
            username,
            password,
            uri,
            method,
            body: Some(&[]),
        })
        .unwrap();
    return Some(ret);
}

fn is_auth_error<T>(r: &BackendResult<T>) -> bool {
    if let Err(e) = r {
        if let BackendError::RequestFail(e) = e {
            if let Some(StatusCode::UNAUTHORIZED) = e.status() {
                return true;
            }
        }
    }
    return false;
}

impl Webdav {
    pub fn new(arg: BuildWebdavArg) -> Self {
        Self {
            addr: arg.addr,
            username: arg.username,
            password: arg.password,
            _is_anonymous: arg.is_anonymous,
            last_www_authenticate: Default::default(),
            connect_timeout: arg.connect_timeout,
        }
    }

    fn post_handle_response(&self, resp: &reqwest::Response) {
        let headers = resp.headers();
        let www_authenticate = headers.get(reqwest::header::WWW_AUTHENTICATE);
        if let Some(www_authenticate) = www_authenticate {
            let www_authenticate = www_authenticate.to_str();
            if let Ok(www_authenticate) = www_authenticate {
                {
                    let mut writter = self.last_www_authenticate.write().unwrap();
                    *writter = Some(www_authenticate.to_string());
                }
            }
        }
    }

    fn build_base_header_map(
        &self,
        method: reqwest::Method,
        uri: &reqwest::Url,
    ) -> reqwest::header::HeaderMap {
        let mut header_map = reqwest::header::HeaderMap::new();
        header_map.append(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_bytes(b"application/xml").unwrap(),
        );
        header_map.append(
            reqwest::header::ACCEPT,
            HeaderValue::from_bytes(b"application/xml").unwrap(),
        );
        {
            let www_authenticate = self.last_www_authenticate.read().unwrap().clone();
            if www_authenticate.is_some() {
                let www_authenticate = www_authenticate.unwrap();
                let auth = build_authorization_header_value(
                    &www_authenticate,
                    &self.username,
                    &self.password,
                    uri.as_str(),
                    method.as_str(),
                );
                if auth.is_some() {
                    let mut val = HeaderValue::from_str(auth.as_ref().unwrap()).unwrap();
                    val.set_sensitive(true);
                    header_map.append(reqwest::header::AUTHORIZATION, val);
                }
            }
        }
        return header_map;
    }

    async fn list_core(&self, dir: &str) -> BackendResult<reqwest::Response> {
        let mut url = reqwest::Url::parse(&self.addr)
            .map_err(|e| BackendError::UrlParseError(e.to_string()))?;
        url.set_path(dir);

        let method = reqwest::Method::from_bytes(b"PROPFIND").unwrap();
        let resp = self
            .build_client()?
            .request(method.clone(), url.clone())
            .headers(self.build_base_header_map(method.clone(), &url))
            .header("Depth", 1)
            .body(
                r#"<?xml version="1.0" ?>
            <D:propfind xmlns:D="DAV:">
            <D:allprop/>
            </D:propfind>"#,
            )
            .send()
            .await?;
        self.post_handle_response(&resp);

        Ok(resp)
    }

    async fn list_impl(&self, dir: &str) -> BackendResult<Vec<Entry>> {
        let resp = self.list_core(dir).await?.error_for_status()?;
        let text: String = resp.text().await?;
        let obj: webdav_list_types::Root = quick_xml::de::from_str(&text).unwrap();

        let mut ret: Vec<Entry> = Default::default();
        for item in obj.response {
            let mut path = item.href;
            let mut name = item.propstat.prop.displayname.unwrap_or(Default::default());
            let is_dir = item.propstat.prop.resourcetype.collection.is_some();
            let size = item.propstat.prop.getcontentlength;

            if path == "/" {
                continue;
            }
            if path.ends_with("/") {
                path.pop();
            }
            if path == dir || (dir.ends_with('/') && dir[0..dir.len() - 1] == path) {
                continue;
            }
            if name.is_empty() {
                let splited: Vec<&str> = path.split("/").collect();
                if !splited.is_empty() {
                    name = splited.last().unwrap().to_string();
                }
            }

            ret.push(Entry {
                name,
                path,
                size,
                is_dir,
            });
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

    async fn list_with_retry_impl(&self, dir: &str) -> BackendResult<Vec<Entry>> {
        let r = self.list_impl(dir).await;
        if !is_auth_error(&r) {
            return r;
        }
        return self.list_impl(dir).await;
    }

    async fn get_impl(&self, p: &str) -> BackendResult<StreamFile> {
        let mut url = reqwest::Url::parse(&self.addr)
            .map_err(|e| BackendError::UrlParseError(e.to_string()))?;
        url.set_path(p);

        let resp = self
            .build_client()?
            .get(url.clone())
            .headers(self.build_base_header_map(reqwest::Method::GET, &url))
            .send()
            .await?;
        self.post_handle_response(&resp);

        let res = resp.error_for_status().map(|resp| StreamFile::new(resp))?;
        Ok(res)
    }

    async fn get_with_retry_impl(&self, p: &str) -> BackendResult<StreamFile> {
        let r = self.get_impl(p).await;
        if !is_auth_error(&r) {
            return r;
        }
        return self.get_impl(p).await;
    }

    fn build_client(&self) -> BackendResult<reqwest::Client> {
        let client = reqwest::Client::builder()
            .connect_timeout(self.connect_timeout)
            .build()?;
        Ok(client)
    }
}

#[async_trait]
impl StorageBackend for Webdav {
    async fn list(&self, dir: &str) -> BackendResult<Vec<Entry>> {
        self.list_with_retry_impl(dir).await
    }
    async fn remove(&self, _p: &str) {
        unimplemented!()
    }

    async fn get(&self, p: &str) -> BackendResult<StreamFile> {
        self.get_with_retry_impl(p).await
    }
    fn default_url(&self) -> String {
        return "/".to_string();
    }
}

#[cfg(test)]
mod test {
    use std::{convert::Infallible, net::SocketAddr, time::Duration};

    use dav_server::{fakels::FakeLs, localfs::LocalFs, DavHandler};
    use futures_util::{pin_mut, StreamExt};
    use tokio::task::JoinHandle;

    use crate::backend::StorageBackend;

    use super::{BuildWebdavArg, Webdav};

    struct SetupServerRes {
        addr: String,
        handle: JoinHandle<()>,
    }
    impl SetupServerRes {
        pub fn addr(&self) -> String {
            self.addr.clone()
        }
    }
    impl Drop for SetupServerRes {
        fn drop(&mut self) {
            self.handle.abort();
        }
    }

    async fn setup_server(p: &str) -> SetupServerRes {
        let dav_server = DavHandler::builder()
            .filesystem(LocalFs::new(p, false, false, false))
            .locksystem(FakeLs::new())
            .autoindex(true)
            .build_handler();

        let addr: SocketAddr = ([127, 0, 0, 1], 0).into();
        let make_service = hyper::service::make_service_fn(move |_| {
            let dav_server = dav_server.clone();
            async move {
                let func = move |req| {
                    let dav_server = dav_server.clone();
                    async move { Ok::<_, Infallible>(dav_server.handle(req).await) }
                };
                Ok::<_, Infallible>(hyper::service::service_fn(func))
            }
        });

        let server = hyper::Server::bind(&addr).serve(make_service);
        let port = server.local_addr().port();

        let handle = tokio::spawn(async move {
            server.await.unwrap();
        });
        tokio::time::sleep(Duration::from_millis(200)).await;

        SetupServerRes {
            addr: format!("http://127.0.0.1:{}", port),
            handle,
        }
    }

    #[tokio::test]
    async fn test_list() {
        let server = setup_server("test/assets/case_list").await;

        let backend = Webdav::new(BuildWebdavArg {
            addr: server.addr(),
            username: Default::default(),
            password: Default::default(),
            is_anonymous: true,
            connect_timeout: Duration::from_secs(10),
        });
        let list = backend.list("/").await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].path, "/a.txt");
        assert_eq!(list[1].path, "/b.log.txt");
    }

    #[tokio::test]
    async fn test_file_content_1() {
        let server = setup_server("test/assets/case_content").await;

        let backend = Webdav::new(BuildWebdavArg {
            addr: server.addr(),
            username: Default::default(),
            password: Default::default(),
            is_anonymous: true,
            connect_timeout: Duration::from_secs(10),
        });
        let mut list = backend.list("/").await.unwrap();
        assert_eq!(list.len(), 1);

        let item = list.pop().unwrap();
        assert_eq!(item.path, "/a.bin");
        assert_eq!(item.size, Some(3));

        let file = backend.get(&item.path).await.unwrap();
        assert_eq!(file.size(), Some(3));

        let stream = file.into_stream();
        pin_mut!(stream);
        let chunk = stream.next().await;
        assert_eq!(chunk.is_some(), true);
        let chunk = chunk.unwrap().unwrap();
        assert_eq!(chunk.as_ref(), [49, 50, 51]);
    }

    #[tokio::test]
    async fn test_file_content_2() {
        let server = setup_server("test/assets/case_content_2").await;

        let backend = Webdav::new(BuildWebdavArg {
            addr: server.addr(),
            username: Default::default(),
            password: Default::default(),
            is_anonymous: true,
            connect_timeout: Duration::from_secs(10),
        });
        let list = backend.list("/").await.unwrap();
        assert_eq!(list.len(), 2);
        let item = &list[0];
        assert_eq!(item.path, "/b-folder");
        assert_eq!(item.size, None);
        let item = &list[1];
        assert_eq!(item.path, "/a.bin");
        assert_eq!(item.size, Some(3));

        let list = backend.list("/b-folder").await.unwrap();
        assert_eq!(list.len(), 1);
        let item = &list[0];
        assert_eq!(item.path, "/b-folder/b.bin");
        assert_eq!(item.size, Some(3));

        let file = backend.get(&item.path).await.unwrap();
        assert_eq!(file.size(), Some(3));

        let stream = file.into_stream();
        pin_mut!(stream);
        let chunk = stream.next().await;
        assert_eq!(chunk.is_some(), true);
        let chunk = chunk.unwrap().unwrap();
        assert_eq!(chunk.as_ref(), [49, 50, 51]);
    }
}
