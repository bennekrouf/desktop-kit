//! A message from us to the people running a build.
//!
//! The builds are free and the download logs are anonymous, so this is the one
//! channel that reaches exactly the people using an app: a small JSON file next
//! to the builds, fetched once at start-up and shown as a banner until
//! dismissed. Used for the founding-users ask ("it's free — we'd love your
//! feedback"), not for release news, which the update check covers.
//!
//! Best-effort throughout: no network, no file, or a malformed file all mean
//! no banner. Never panics, never slows start-up.
//!
//! Served from mayorana.ch:
//!   /downloads/<app id>/notice.json   — this app only (wins when present)
//!   /downloads/notice.json            — every app
//!
//! ```json
//! { "id": "founding-2026-09",
//!   "apps": null,                       // or ["spreadradar", ...] to target
//!   "text": "You're one of the first people running {app}. ...",
//!   "link_text": "Tell us how it's going",
//!   "url": "https://mayorana.ch/en/contact" }
//! ```
//!
//! `id` is what a dismissal remembers, so a new message needs a new id.

use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;

const GLOBAL_NOTICE_URL: &str = "https://mayorana.ch/downloads/notice.json";
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Notice {
    pub id: String,
    #[serde(default)]
    pub apps: Option<Vec<String>>,
    pub text: String,
    #[serde(default)]
    pub link_text: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

/// Returns the notice to show, or `None`: not published, not for this app,
/// already dismissed, unreachable, or called before [`crate::init`].
pub async fn fetch() -> Option<Notice> {
    if std::env::var_os("DISABLE_UPDATE_CHECK").is_some() {
        return None;
    }
    let app = crate::app()?;
    let app_url = format!("https://mayorana.ch/downloads/{}/notice.json", app.id);
    let notice = match get(&app_url).await {
        Some(n) => n,
        None => get(GLOBAL_NOTICE_URL).await?,
    };
    for_app(notice, app.id, app.name, &dismissed_ids())
}

/// `notice` as this app shows it, unless it targets other apps or was closed.
fn for_app(notice: Notice, id: &str, name: &str, dismissed: &[String]) -> Option<Notice> {
    if let Some(apps) = &notice.apps {
        if !apps.iter().any(|a| a == id) {
            return None;
        }
    }
    if dismissed.iter().any(|d| d == &notice.id) {
        return None;
    }
    Some(Notice {
        text: notice.text.replace("{app}", name),
        ..notice
    })
}

/// Remember that this notice was closed, so it does not come back at the next
/// start. Losing the file (or failing to write it) only means asking again.
pub fn dismiss(id: &str) {
    let mut ids = dismissed_ids();
    if ids.iter().any(|d| d == id) {
        return;
    }
    ids.push(id.to_string());
    if let Some(path) = dismissed_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string(&ids) {
            let _ = std::fs::write(path, json);
        }
    }
}

/// Distinct from the updater's purpose on purpose: the download statistics
/// count `(updater)` polls as live installs, and this fetch is not one.
async fn get(url: &str) -> Option<Notice> {
    let body = crate::http::get_ok(url, "notice", TIMEOUT).await?;
    serde_json::from_str(&body).ok()
}

/// One file for every mayorana app rather than one per app: the notice is
/// usually the same message, and dismissing it in one app should not mean
/// seeing it again in the next.
fn dismissed_path() -> Option<PathBuf> {
    Some(
        dirs::data_local_dir()?
            .join("mayorana")
            .join("dismissed-notices.json"),
    )
}

fn dismissed_ids() -> Vec<String> {
    dismissed_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(apps: Option<&[&str]>) -> Notice {
        Notice {
            id: "n1".into(),
            apps: apps.map(|a| a.iter().map(|s| s.to_string()).collect()),
            text: "Thanks for running {app}.".into(),
            link_text: None,
            url: None,
        }
    }

    #[test]
    fn the_app_name_is_filled_in() {
        let shown = for_app(notice(None), "trackcut", "TrackCut", &[]).unwrap();
        assert_eq!(shown.text, "Thanks for running TrackCut.");
    }

    #[test]
    fn a_notice_for_other_apps_is_not_shown() {
        assert!(for_app(notice(Some(&["gitagent"])), "trackcut", "TrackCut", &[]).is_none());
        assert!(for_app(
            notice(Some(&["gitagent", "trackcut"])),
            "trackcut",
            "TrackCut",
            &[]
        )
        .is_some());
    }

    #[test]
    fn a_dismissed_notice_stays_closed() {
        assert!(for_app(notice(None), "trackcut", "TrackCut", &["n1".into()]).is_none());
    }
}
