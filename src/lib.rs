//! What every mayorana desktop app shares, kept in one place so a fix lands in
//! all of them at once.
//!
//! * [`telemetry`]: anonymous usage statistics (gitagent keeps its own, richer
//!   version);
//! * [`update_check`]: whether a newer release is published, and the download
//!   link for this computer;
//! * [`notice`]: the one-line message mayorana.ch can show in every app;
//! * [`feedback`]: the links back to mayorana.ch;
//! * [`licence`]: the licence number, api0's answer about it, and the
//!   requests that check it;
//! * [`browser`]: opening a web page in the person's browser;
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
pub mod http;
pub mod licence;
pub mod notice;
pub mod region;
pub mod telemetry;
#[cfg(feature = "ui")]
pub mod ui;
pub mod update_check;

pub(crate) use app::app;
pub use app::{init, App};
