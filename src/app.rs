//! Which app this is, set once at start-up.

use std::path::PathBuf;
use std::sync::OnceLock;

use crate::http;

/// Everything the shared modules need to know about the app using them.
#[derive(Clone, Copy, Debug)]
pub struct App {
    /// The app's id on mayorana.ch: its folder under `/downloads/`, and the
    /// first word of the User-Agent the download statistics count by. Lower
    /// case, e.g. `"spreadradar"`.
    pub id: &'static str,
    /// The name people see, e.g. `"SpreadRadar"`. Replaces `{app}` in a notice.
    pub name: &'static str,
    /// The app's own version: `env!("CARGO_PKG_VERSION")` written in the app
    /// crate, since in this crate it would be this crate's version.
    pub version: &'static str,
    /// The folder under `mayorana/telemetry/` holding this install's identity
    /// and the person's answer. Usually `id`; an app that was renamed keeps
    /// its old name here, so a "turn it off" given under that name still holds.
    pub telemetry_folder: &'static str,
    /// The id api0 knows the app by: a plan lists it to make the app Pro. Not
    /// always `id` (PolishCast is `small-video`).
    pub product: &'static str,
    /// Where the app keeps api0's last answer about its licence; `None` for an
    /// app without one. The number itself is in the file every app shares.
    pub licence_file: fn() -> Option<PathBuf>,
    /// One HTTP request with the app's own client. See [`http::Call`].
    pub http: http::Call,
}

static APP: OnceLock<App> = OnceLock::new();

/// Names the app. Call once, first thing in `main`; later calls are ignored.
pub fn init(app: App) {
    let _ = APP.set(app);
}

/// The app, once [`init`] has named it.
pub(crate) fn app() -> Option<&'static App> {
    APP.get()
}
