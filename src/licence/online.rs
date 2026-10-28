//! The licence's requests to api0, and this app's copy of api0's last answer:
//! what every app used to write for itself.
//!
//! Each needs [`crate::init`]: before it, or in an app with no
//! [`crate::App::licence_file`], there is no licence to read and nothing is
//! sent.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use super::{
    check_due, free_reply, load_shared, normalise, plausible_email, reconcile, Answer, FreeError,
    Saved, Shared, Status, GATEWAY,
};
use crate::http::{self, Method};

const TIMEOUT: Duration = Duration::from_secs(20);

/// Where "Buy Pro" leads: the app's page on mayorana.ch.
pub fn buy_url() -> String {
    crate::feedback::site_url()
}

fn product() -> &'static str {
    crate::app().map_or("", |a| a.product)
}

fn file() -> Option<PathBuf> {
    (crate::app()?.licence_file)()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn save_shared(text: &str) {
    super::save_shared(text, product());
}

fn load_own() -> Option<Saved> {
    let text = std::fs::read_to_string(file()?).ok()?;
    serde_json::from_str(&text).ok()
}

/// The app's saved licence, brought in line with the shared file.
fn load() -> Option<Saved> {
    let path = file()?;
    let shared = load_shared();
    if shared == Shared::Removed {
        let _ = std::fs::remove_file(&path);
    }
    let (saved, share) = reconcile(shared, load_own());
    if share {
        if let Some(saved) = &saved {
            save_shared(&saved.licence);
        }
    }
    saved
}

fn save(saved: &Saved) -> Result<(), String> {
    let path = file().ok_or("The licence can't be saved on this computer.")?;
    let json = serde_json::to_vec_pretty(saved).map_err(|e| e.to_string())?;
    let write = || -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Written beside the target and renamed over it, so a crash can't
        // leave half a file behind.
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &path)
    };
    write().map_err(|e| format!("The licence couldn't be saved: {e}"))
}

/// What `saved` amounts to for this app at `now` (seconds since 1970).
pub fn status_now(saved: Option<&Saved>, now: u64) -> Status {
    super::status_at(saved, now, product())
}

/// The licence as of the last check, without asking api0.
pub fn current() -> Status {
    status_now(load().as_ref(), now())
}

/// What the window title adds after "<App> <version>".
pub fn title_badge(status: &Status) -> &'static str {
    status.title_badge_for(product())
}

/// Why a check didn't give an answer.
enum CheckError {
    /// api0 doesn't know this licence number.
    Unknown,
    /// Offline, api0 down, or an answer that made no sense.
    Unreachable(String),
}

/// A request to api0 about `licence`.
async fn call(method: Method, path: &str, licence: &str) -> Option<http::Response> {
    let headers = vec![
        ("Authorization", format!("Bearer {licence}")),
        ("X-Client-App", product().to_string()),
    ];
    http::send(
        method,
        &format!("{GATEWAY}{path}"),
        "licence",
        TIMEOUT,
        headers,
        None,
    )
    .await
}

async fn ask(licence: &str) -> Result<Answer, CheckError> {
    let response = call(Method::Get, "/v1/licence", licence)
        .await
        .ok_or_else(|| CheckError::Unreachable("api0 couldn't be reached".into()))?;
    match response.status {
        200 => serde_json::from_str(&response.body)
            .map_err(|_| CheckError::Unreachable("api0's answer couldn't be read".into())),
        401 => Err(CheckError::Unknown),
        code => Err(CheckError::Unreachable(format!("api0 answered {code}"))),
    }
}

/// Checks `typed` with api0 and, if api0 knows it, saves it for every app.
pub async fn activate(typed: &str) -> Result<Status, String> {
    let licence = normalise(typed).ok_or(
        "This isn't a licence number. It has 20 letters and digits, like ABCDE-FGHJK-LMNPQ-RSTUV.",
    )?;
    let answer = match ask(&licence).await {
        Ok(a) => a,
        Err(CheckError::Unknown) => {
            return Err("This licence number isn't known. Check it against the email.".into())
        }
        Err(CheckError::Unreachable(e)) => {
            return Err(format!(
                "The licence couldn't be checked ({e}). Check the connection and try again."
            ))
        }
    };
    let saved = Saved {
        licence,
        answer: Some(answer),
        checked: now(),
    };
    save(&saved)?;
    save_shared(&saved.licence);
    Ok(status_now(Some(&saved), now()))
}

/// Checks the saved licence with api0 again, when `force`d or a day after the
/// last check, and keeps the answer. Offline, the last answer stands.
pub async fn refresh(force: bool) -> Status {
    let Some(mut saved) = load() else {
        return Status::Free;
    };
    if !force && !check_due(&saved, now()) {
        return status_now(Some(&saved), now());
    }
    match ask(&saved.licence).await {
        Ok(answer) => {
            saved.answer = Some(answer);
            saved.checked = now();
            let _ = save(&saved);
        }
        Err(CheckError::Unknown) => {
            saved.answer = None;
            let _ = save(&saved);
        }
        Err(CheckError::Unreachable(_)) => {}
    }
    status_now(Some(&saved), now())
}

/// The address of the page where the subscription is changed or cancelled.
pub async fn portal_url(licence: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Portal {
        url: String,
    }
    let fail = |why: String| format!("The subscription page couldn't be opened ({why}).");
    let response = call(Method::Post, "/v1/licence/portal", licence)
        .await
        .ok_or_else(|| fail("api0 couldn't be reached".into()))?;
    if !(200..300).contains(&response.status) {
        return Err(fail(format!("api0 answered {}", response.status)));
    }
    serde_json::from_str::<Portal>(&response.body)
        .map(|p| p.url)
        .map_err(|_| fail("api0's answer couldn't be read".into()))
}

/// Asks api0 to email `email` a licence for the free allowance of AI credits.
/// The licence number itself only goes by email.
pub async fn request_free_credits(email: &str) -> Result<String, FreeError> {
    if !plausible_email(email) {
        return Err(FreeError::Refused(
            "Enter the email the licence should go to.".into(),
        ));
    }
    let body = serde_json::json!({ "email": email.trim() }).to_string();
    let response = http::send(
        Method::Post,
        &format!("{GATEWAY}/api/plans/free"),
        "licence",
        TIMEOUT,
        vec![("X-Client-App", product().to_string())],
        Some(body),
    )
    .await
    .ok_or_else(|| {
        FreeError::Refused(
            "The free AI credits couldn't be requested (api0 couldn't be reached).".into(),
        )
    })?;
    free_reply(response.status, &response.body)
}

/// Removes the licence from this computer (to move it to another one), for
/// every Mayorana app on it.
pub fn deactivate() {
    if let Some(path) = file() {
        let _ = std::fs::remove_file(path);
    }
    save_shared("");
}
