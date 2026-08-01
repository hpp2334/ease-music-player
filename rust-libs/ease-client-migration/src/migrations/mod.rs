pub mod m20260715_000001_init;
pub mod m20260726_000002_plugin_kv;
pub mod m20260801_000003_storage_registry;

pub use m20260715_000001_init::Migration as InitMigration;
pub use m20260726_000002_plugin_kv::Migration as PluginKvMigration;
pub use m20260801_000003_storage_registry::Migration as StorageRegistryMigration;
