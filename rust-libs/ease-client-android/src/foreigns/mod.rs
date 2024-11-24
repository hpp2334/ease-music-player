use std::sync::Arc;

use ease_client::{
    IPermissionService, IRouterService, IToastService, IViewStateService, RootViewModelState,
    RoutesKey,
};
use ease_client_backend::{AssetLoadStatus, IAssetLoadDelegate, IPlayerDelegate, MusicToPlay};
use ease_client_shared::backends::music::MusicId;

macro_rules! generate_delegate {
   ($name:ident, $trait:ident, $foreign_trait:ident, { $($method:ident ( $($arg_name:ident : $arg_ty:ty),* ) $(-> $ret_ty:ty)?;)* }) => {
       #[uniffi::export(with_foreign)]
       pub trait $foreign_trait: Send + Sync + 'static {
           $(fn $method(&self, $($arg_name: $arg_ty),*) $(-> $ret_ty)?;)*
       }

       pub struct $name {
           inner: Arc<dyn $foreign_trait>,
       }
       impl $name {
           pub fn new(inner: Arc<dyn $foreign_trait>) -> Arc<Self> {
               Arc::new(Self { inner })
           }
       }
       impl $trait for $name {
           $(fn $method(&self, $($arg_name: $arg_ty),*) $(-> $ret_ty)? {
               self.inner.$method($($arg_name),*)
           })*
       }
   };
}

generate_delegate!(PermissionServiceDelegate, IPermissionService, IPermissionServiceForeign, {
   open_url(url: String);
   have_storage_permission() -> bool;
   request_storage_permission();
});

generate_delegate!(RouterServiceDelegate, IRouterService, IRouterServiceForeign, {
   naviagate(key: RoutesKey);
   pop();
});

generate_delegate!(ToastServiceDelegate, IToastService, IToastServiceForeign, {
   error(msg: String);
});

generate_delegate!(ViewStateServiceDelegate, IViewStateService, IViewStateServiceForeign, {
   handle_notify(v: RootViewModelState);
});

generate_delegate!(PlayerDelegate, IPlayerDelegate, IPlayerDelegateForeign, {
   is_playing() -> bool;
   resume();
   pause();
   stop();
   seek(arg: u64);
   set_music_url(item: MusicToPlay);
   get_current_duration_s() -> u64;
   request_total_duration(id: MusicId);
});

generate_delegate!(AssetLoadDelegate, IAssetLoadDelegate, IAssetLoadDelegateForeign, {
    on_status(status: AssetLoadStatus);
    on_chunk(chunk: Vec<u8>);
});

#[uniffi::export(with_foreign)]
pub trait IAsyncAdapterForeign: Send + Sync + 'static {
    fn on_spawn_locals(&self, app_id: Option<u64>);
}
