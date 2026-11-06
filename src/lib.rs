//! Wellbeing measures whether this device can reach other wellbeing clients.
//!
//! `wellbeing-client` runs on every device. `wellbeing-server` stores reports
//! only on the top-ranked server-grade node and mounts `/apps/wellbeing/`.

pub mod log_setup;
pub mod directory;
pub mod node;
pub mod page;
pub mod proto;
pub mod store;

pub const CLIENT_ALIAS: &str = "wellbeing-client";
pub const SERVER_ALIAS: &str = "wellbeing-server";
pub const APP_PROTOCOL: &str = "application/wellbeing";
pub const DEFAULT_CLIENT_PUSH: u16 = 8800;
pub const DEFAULT_SERVER_PUSH: u16 = 8801;
pub const DEFAULT_HTTP_PORT: u16 = 8810;
pub const WEB_SLUG: &str = "wellbeing";
pub const WEB_TITLE: &str = "Wellbeing";
