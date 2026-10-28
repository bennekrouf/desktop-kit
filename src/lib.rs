//! What every mayorana desktop app shares, kept in one place so a fix lands in
//! all of them at once.
//!
//! * [`telemetry`]: anonymous usage statistics (gitagent keeps its own, richer
//!   version);
//! * [`update_check`]: whether a newer release is published, and the download
//!   link for this computer;
//! * [`notice`]: the one-line message mayorana.ch can show in every app;
//! * [`feedback`]: the links back to mayorana.ch;
//! * [`licence`]: the licence number and api0's answer about it, without the
//!   network and disk parts, which stay with each app.
//!
//! There is no user interface here, and no HTTP client: each app calls
//! [`init`] once at start-up with its name, its version and its own HTTP GET.
//! Until then, nothing here touches the network or records anything.

mod app;
pub mod feedback;
pub mod http;
pub mod licence;
pub mod notice;
pub mod telemetry;
pub mod update_check;

pub(crate) use app::app;
pub use app::{init, App};
