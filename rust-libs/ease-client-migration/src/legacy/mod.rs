//! Legacy v2/v3 schema definitions and redb glue used during database
//! upgrade. Everything in here is migration-internal.
//!
//! Some types in the legacy schema definitions are not referenced by current
//! migration logic but are kept as a faithful representation of the on-disk
//! format; we allow dead_code for that reason.
#![allow(dead_code)]

pub(crate) mod redb_v2;
pub(crate) mod redb_v3;
pub(crate) mod schema_bridge;
pub(crate) mod schema_v2;
pub(crate) mod schema_v3;
pub(crate) mod upgrader_v1_v2;
pub(crate) mod upgrader_v2_v3;

pub(crate) use upgrader_v1_v2::upgrade_v1_to_v2;
pub(crate) use upgrader_v2_v3::upgrade_v2_to_v3;
