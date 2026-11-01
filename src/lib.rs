//! What every mayorana desktop app shares, kept in one place so a fix lands in
//! all of them at once.
//!
//! * [`telemetry`]: anonymous usage statistics;
//! * [`update_check`]: whether a newer release is published, and the download
//!   link for this computer;
//! * [`self_update`]: downloading that build, checking it and putting it in
//!   place of the running app;
//! * [`notice`]: the one-line message mayorana.ch can show in every app;
//! * [`feedback`]: the links back to mayorana.ch;
//! * [`mcp`]: the MCP server over stdio, apart from each app's tools;
//! * [`licence`]: the licence number, api0's answer about it, and the
//!   requests that check it;
//! * [`browser`]: opening a web page in the person's browser;
//! * [`library`]: the media library and pipeline statuses the apps share,
//!   with [`handoff`] notes from one app to another and [`launch`] to open
//!   them and the person's files;
//! * [`region`]: the AI regions api0 offers, with their flags;
//! * [`ui`] (with the `ui` feature): the header, the banners under it and the
//!   Pro window every app shows.
//!
//! There is no HTTP client here: each app calls [`init`] once at start-up with
//! its name, its version and its own HTTP request. Until then, nothing here
//! touches the network or records anything.

mod app;
pub mod browser;
pub mod feedback;
pub mod handoff;
pub mod http;
pub mod launch;
pub mod library;
pub mod licence;
pub mod mcp;
pub mod notice;
pub mod region;
pub mod self_update;
pub mod telemetry;
#[cfg(feature = "ui")]
pub mod ui;
pub mod update_check;

pub(crate) use app::app;
pub use app::{init, App};
