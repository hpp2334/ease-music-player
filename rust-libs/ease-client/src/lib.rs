mod actions;
mod async_adapter;
mod client;
mod error;
mod to_host;
pub(crate) mod utils;
pub mod view_models;

uniffi::setup_scaffolding!();

pub use to_host::permission::{IPermissionService, PermissionService};
pub use to_host::player::{IMusicPlayerService, MusicPlayerService, MusicToPlay};
pub use to_host::router::{IRouterService, RouterService, RoutesKey};
pub use to_host::toast::{IToastService, ToastService};
pub use to_host::view_state::{IViewStateService, ViewStateService};
pub use view_models::view_state::views::RootViewModelState;

pub use actions::widget::{Widget, WidgetAction, WidgetActionType};
pub use actions::{Action, ViewAction};
pub use view_models::main::{MainAction, MainBodyWidget};
pub use view_models::music::control::{MusicControlWidget, PlayerEvent};
pub use view_models::music::detail::MusicDetailWidget;
pub use view_models::music::lyric::MusicLyricWidget;
pub use view_models::music::time_to_pause::TimeToPauseWidget;
pub use view_models::playlist::create::PlaylistCreateWidget;
pub use view_models::playlist::detail::PlaylistDetailWidget;
pub use view_models::playlist::edit::PlaylistEditWidget;
pub use view_models::playlist::list::PlaylistListWidget;
pub use view_models::storage::import::StorageImportWidget;
pub use view_models::storage::list::StorageListWidget;
pub use view_models::storage::upsert::StorageUpsertWidget;

pub use client::build_client;
pub use error::EaseError;
