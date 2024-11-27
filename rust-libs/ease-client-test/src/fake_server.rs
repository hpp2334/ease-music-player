use std::{
    convert::Infallible,
    net::SocketAddr,
    sync::{atomic::AtomicUsize, Arc, Mutex},
};

use dav_server::{fakels::FakeLs, localfs::LocalFs, DavHandler};
use hyper::{Response, StatusCode};

#[derive(Debug, Clone)]
pub enum ReqInteceptor {
    AuthenticationFailed,
    InternalError,
}

pub struct FakeServerInner {
    addr: String,
    tx: Option<tokio::sync::oneshot::Sender<()>>,
    req_inteceptor: Arc<Mutex<Option<ReqInteceptor>>>,
}

pub struct FakeServerRef {
    inner: Arc<FakeServerInner>,
}

impl FakeServerRef {
    pub fn setup(p: &str) -> Self {
        FakeServerRef {
            inner: Arc::new(FakeServerInner::setup(p)),
        }
    }
}

impl FakeServerInner {
    pub fn setup(p: &str) -> Self {
        let dav_server = DavHandler::builder()
            .filesystem(LocalFs::new(p, false, false, false))
            .locksystem(FakeLs::new())
            .autoindex(true)
            .build_handler();
        let req_inteceptor: Arc<Mutex<Option<ReqInteceptor>>> = Default::default();
        let cloned_req_inteceptor = req_inteceptor.clone();
        let req_session: Arc<AtomicUsize> = Default::default();
        let cloned_req_session = req_session.clone();
        let addr: SocketAddr = ([127, 0, 0, 1], 0).into();
        let make_service = hyper::service::make_service_fn(move |_| {
            let dav_server = dav_server.clone();
            let req_inteceptor = req_inteceptor.clone();
            let cloned_req_session = cloned_req_session.clone();
            async move {
                let func = move |req| {
                    let dav_server = dav_server.clone();
                    let req_inteceptor = req_inteceptor.clone();
                    let cloned_req_session = cloned_req_session.clone();
                    async move {
                        {
                            let req_inteceptor = {
                                let req_inteceptor = req_inteceptor.clone();
                                let req_inteceptor = req_inteceptor.lock().unwrap();
                                let req_inteceptor = req_inteceptor.clone();
                                req_inteceptor
                            };
                            if req_inteceptor.is_some() {
                                match req_inteceptor.as_ref().unwrap() {
                                    ReqInteceptor::AuthenticationFailed => {
                                        let mut resp =
                                            Response::new(dav_server::body::Body::empty());
                                        *resp.status_mut() = StatusCode::UNAUTHORIZED;
                                        return Ok(resp);
                                    }
                                    ReqInteceptor::InternalError => {
                                        let mut resp =
                                            Response::new(dav_server::body::Body::empty());
                                        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                                        return Ok(resp);
                                    }
                                }
                            }
                        }
                        let resp = dav_server.handle(req).await;
                        cloned_req_session.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        return Ok::<_, Infallible>(resp);
                    }
                };
                Ok::<_, Infallible>(hyper::service::service_fn(func))
            }
        });

        let (tx_abort_server, rx_abort_server) = tokio::sync::oneshot::channel::<()>();
        let server = hyper::Server::bind(&addr).serve(make_service);
        let port = server.local_addr().port();

        tokio::spawn(async move {
            let server = server.with_graceful_shutdown(async {
                rx_abort_server.await.ok();
            });
            server.await.unwrap();
        });

        FakeServerInner {
            addr: format!("http://127.0.0.1:{}", port),
            tx: Some(tx_abort_server),
            req_inteceptor: cloned_req_inteceptor,
        }
    }

    pub fn addr(&self) -> String {
        self.addr.clone()
    }

    pub fn set_inteceptor_req(&self, v: Option<ReqInteceptor>) {
        let req_inteceptor = self.req_inteceptor.clone();
        let mut req_inteceptor = req_inteceptor.lock().unwrap();
        *req_inteceptor = v;
    }
}

impl Drop for FakeServerInner {
    fn drop(&mut self) {
        let tx = self.tx.take().unwrap();
        let _ = tx.send(());
        tracing::info!("drop server");
    }
}

impl FakeServerRef {
    pub fn addr(&self) -> String {
        self.inner.addr()
    }
    pub fn set_inteceptor_req(&self, v: Option<ReqInteceptor>) {
        self.inner.set_inteceptor_req(v);
    }
}
