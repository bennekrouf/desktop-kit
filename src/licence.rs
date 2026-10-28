//! The licence on this computer, without the network: what every app agrees on.
//!
//! A Pro plan is bought on mayorana.ch and comes with a licence number
//! (`ABCDE-FGHJK-LMNPQ-RSTUV`, emailed when it is bought). Each app asks api0
//! about it at start and then once a day:
//!
//! ```text
//! GET https://gateway.api0.ai/v1/licence
//! Authorization: Bearer <licence>
//! X-Client-App: <product>
//! ```
//!
//! and keeps the last answer on disk, so an app started offline reads the last
//! one: Pro lasts [`GRACE`] after the last check that got through. The number
//! itself goes in the file every Mayorana app shares ([`load_shared`]), so it
//! is typed once per computer.
//!
//! Here: the number's format, api0's answer, what a saved answer amounts to at
//! a given time, and the shared file. The requests, and where an app keeps its
//! own answer, stay in each app, which uses its own HTTP client. Everything
//! that depends on the app takes its `product`: the id api0 knows it by, which
//! is not always its name (PolishCast is `small-video`).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Where every app asks about a licence.
pub const GATEWAY: &str = "https://gateway.api0.ai";

/// How often the licence is checked while an app is open.
pub const CHECK_EVERY: Duration = Duration::from_secs(24 * 3600);

/// How long Pro lasts without a check getting through (offline, api0 down).
pub const GRACE: Duration = Duration::from_secs(7 * 24 * 3600);

const ALPHABET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// A licence number as typed — any case, spaces or dashes — in the form api0
/// stores it, or `None` if it can't be one.
pub fn normalise(typed: &str) -> Option<String> {
    let chars: String = typed
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if chars.len() != 20 || !chars.chars().all(|c| ALPHABET.contains(c)) {
        return None;
    }
    Some(
        chars
            .as_bytes()
            .chunks(5)
            .map(|c| std::str::from_utf8(c).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("-"),
    )
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Credits {
    #[serde(default)]
    pub granted: i64,
    #[serde(default)]
    pub used: i64,
    #[serde(default)]
    pub left: i64,
}

/// api0's answer about a licence (`GET /v1/licence`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    #[serde(default)]
    pub licence: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub plan: Plan,
    /// The apps the plan makes Pro, by product id; "*" is every app.
    #[serde(default)]
    pub products: Vec<String>,
    /// Paid up. False once the subscription lapses or is cancelled.
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub cancel_at_period_end: bool,
    /// When the current month ends (RFC 3339).
    #[serde(default)]
    pub period_end: String,
    #[serde(default)]
    pub credits: Credits,
    #[serde(default)]
    pub checked_at: String,
}

impl Answer {
    /// The plan lists `product`, or every app.
    pub fn lists(&self, product: &str) -> bool {
        self.products.iter().any(|p| p == product || p == "*")
    }

    /// The plan is paid up and includes `product`.
    pub fn unlocks_for(&self, product: &str) -> bool {
        self.active && self.lists(product)
    }

    /// The day the current month ends, `YYYY-MM-DD`.
    pub fn period_end_day(&self) -> &str {
        self.period_end.get(..10).unwrap_or(&self.period_end)
    }

    /// The free allowance an app hands out: AI credits each month, no app made
    /// Pro.
    pub fn is_free(&self) -> bool {
        self.active && self.products.is_empty()
    }

    /// "Renews on …", or "Ends on …" once cancelled.
    pub fn renewal(&self) -> String {
        let verb = if self.cancel_at_period_end {
            "Ends"
        } else {
            "Renews"
        };
        format!("{verb} on {}", self.period_end_day())
    }
}

/// What is kept on disk: the licence number, and api0's last answer with when
/// it came (seconds since 1970, this computer's clock).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub licence: String,
    #[serde(default)]
    pub answer: Option<Answer>,
    #[serde(default)]
    pub checked: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// No licence on this computer.
    Free,
    /// The plan includes the app and is paid up, as of a check within [`GRACE`].
    Pro(Answer),
    /// Checked: the plan has lapsed, or doesn't include the app.
    NotPro(Answer),
    /// Pro at the last check, but no check has got through for [`GRACE`].
    Offline(Answer),
    /// A licence number api0 doesn't know, or one not checked yet.
    Unknown(String),
}

impl Status {
    /// What the window title adds after "<App> <version>": Pro, or a Pro that
    /// has lapsed (unpaid, or no check for the grace period).
    pub fn title_badge_for(&self, product: &str) -> &'static str {
        match self {
            Status::Pro(_) => " \u{b7} \u{2726} Pro",
            Status::Offline(_) => " \u{b7} Pro inactive",
            Status::NotPro(a) if !a.active && a.lists(product) => " \u{b7} Pro inactive",
            _ => "",
        }
    }

    /// Whether the plan makes the app Pro now.
    pub fn is_pro(&self) -> bool {
        matches!(self, Status::Pro(_))
    }

    /// api0's answer, when the licence is the free allowance of AI credits.
    pub fn free_credits(&self) -> Option<&Answer> {
        match self {
            Status::NotPro(a) if a.is_free() => Some(a),
            _ => None,
        }
    }

    /// The saved licence number, if any.
    pub fn licence(&self) -> Option<&str> {
        match self {
            Status::Free => None,
            Status::Pro(a) | Status::NotPro(a) | Status::Offline(a) => Some(&a.licence),
            Status::Unknown(l) => Some(l),
        }
    }
}

/// Why a request for free AI credits got no licence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FreeError {
    /// There are no free credits right now: the form is hidden.
    Unavailable,
    /// Anything else, to show as it is.
    Refused(String),
}

/// Whether `email` can be one, before asking api0.
pub fn plausible_email(email: &str) -> bool {
    let email = email.trim();
    match email.split_once('@') {
        Some((user, domain)) => {
            !user.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !email.contains(char::is_whitespace)
        }
        None => false,
    }
}

/// api0's reply to `POST /api/plans/free`, read: the message to show once the
/// licence number is on its way.
pub fn free_reply(status: u16, body: &str) -> Result<String, FreeError> {
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["message"].as_str().map(str::to_owned));
    match status {
        200..=299 => Ok(
            message.unwrap_or_else(|| "Your licence number is on its way to that email.".into())
        ),
        404 => Err(FreeError::Unavailable),
        409 => Err(FreeError::Refused(
            "This email already has a plan: paste its licence number instead.".into(),
        )),
        429 => Err(FreeError::Refused(
            "Too many requests: try again in a while.".into(),
        )),
        _ => Err(FreeError::Refused(message.unwrap_or_else(|| {
            format!("The free AI credits couldn't be requested (api0 answered {status}).")
        }))),
    }
}

/// What `saved` amounts to for `product` at `now` (seconds since 1970).
pub fn status_at(saved: Option<&Saved>, now: u64, product: &str) -> Status {
    let Some(saved) = saved else {
        return Status::Free;
    };
    let Some(answer) = saved.answer.clone() else {
        return Status::Unknown(saved.licence.clone());
    };
    if !answer.unlocks_for(product) {
        Status::NotPro(answer)
    } else if now.saturating_sub(saved.checked) > GRACE.as_secs() {
        Status::Offline(answer)
    } else {
        Status::Pro(answer)
    }
}

/// Whether the last check is old enough for another, at `now`.
pub fn check_due(saved: &Saved, now: u64) -> bool {
    saved.answer.is_none() || now.saturating_sub(saved.checked) >= CHECK_EVERY.as_secs()
}

/// What the licence file every Mayorana app shares says. One plan unlocks
/// every app, so its number is typed once per computer: each app keeps only
/// its own last answer from api0 beside it.
#[derive(Clone, Debug, PartialEq)]
pub enum Shared {
    /// No app has written it yet.
    Missing,
    /// Removed from this computer, in any app: none of them is licensed.
    Removed,
    Licence(String),
}

/// `~/Library/Application Support/mayorana/licence` on a Mac,
/// `%APPDATA%\mayorana\licence` on Windows, `~/.config/mayorana/licence` on Linux.
pub fn shared_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("mayorana").join("licence"))
}

/// The shared file's text, read.
pub fn shared_from(text: &str) -> Shared {
    normalise(text).map_or(Shared::Removed, Shared::Licence)
}

/// What the shared file says now.
pub fn load_shared() -> Shared {
    let Some(path) = shared_path() else {
        return Shared::Missing;
    };
    match std::fs::read_to_string(path) {
        Ok(text) => shared_from(&text),
        Err(_) => Shared::Missing,
    }
}

/// Writes `text` as the shared licence: a number, or nothing once removed.
/// `product` names the temporary file, so two apps writing at once don't
/// share it.
pub fn save_shared(text: &str, product: &str) {
    let write = || -> std::io::Result<()> {
        let path = shared_path().ok_or_else(|| std::io::Error::other("no config folder"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension(format!("{product}.tmp"));
        std::fs::write(&tmp, text)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp, &path)
    };
    let _ = write();
}

/// This app's saved licence, brought in line with the shared one: a number
/// typed in another app replaces this one (and its answer, to check again),
/// and one removed there is removed here. `true` when this app's number
/// should be shared, as it predates the shared file.
pub fn reconcile(shared: Shared, own: Option<Saved>) -> (Option<Saved>, bool) {
    match (shared, own) {
        (Shared::Licence(l), Some(own)) if own.licence == l => (Some(own), false),
        (Shared::Licence(licence), _) => (
            Some(Saved {
                licence,
                ..Saved::default()
            }),
            false,
        ),
        (Shared::Removed, _) => (None, false),
        (Shared::Missing, Some(own)) => (Some(own), true),
        (Shared::Missing, None) => (None, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 24 * 3600;
    const ME: &str = "publishkit";

    fn answer(products: &[&str], active: bool) -> Answer {
        Answer {
            licence: "ABCDE-FGHJK-LMNPQ-RSTUV".into(),
            email: "dev@example.ch".into(),
            plan: Plan {
                id: "pro".into(),
                name: "Pro".into(),
            },
            products: products.iter().map(|p| p.to_string()).collect(),
            active,
            status: "active".into(),
            period_end: "2026-11-21T10:00:00Z".into(),
            ..Answer::default()
        }
    }

    fn saved(answer: Answer, checked: u64) -> Saved {
        Saved {
            licence: answer.licence.clone(),
            answer: Some(answer),
            checked,
        }
    }

    #[test]
    fn licence_numbers_are_read_in_any_case_with_spaces_or_dashes() {
        let want = Some("ABCDE-FGHJK-LMNPQ-RSTUV".to_string());
        assert_eq!(normalise("ABCDE-FGHJK-LMNPQ-RSTUV"), want);
        assert_eq!(normalise(" abcde fghjk\nlmnpq-rstuv "), want);
        assert_eq!(normalise("ABCDEFGHJKLMNPQRSTUV"), want);
        // Too short, and letters that look like digits aren't in the alphabet.
        assert_eq!(normalise("ABCDE-FGHJK-LMNPQ"), None);
        assert_eq!(normalise("ABCDE-FGHJK-LMNPQ-RSTU0"), None);
        assert_eq!(normalise("ABCDE-FGHJK-LMNPQ-RSTUI"), None);
        assert_eq!(shared_from(""), Shared::Removed);
    }

    #[test]
    fn pro_needs_an_active_plan_that_lists_the_app_or_every_app() {
        assert!(answer(&[ME], true).unlocks_for(ME));
        assert!(answer(&["gitagent", "*"], true).unlocks_for(ME));
        assert!(!answer(&["gitagent", "jobscout"], true).unlocks_for(ME));
        assert!(!answer(&[ME], false).unlocks_for(ME));
        assert!(!answer(&[], true).unlocks_for(ME));

        let now = 1_000 * DAY;
        assert!(status_at(Some(&saved(answer(&["*"], true), now)), now, ME).is_pro());
        assert!(matches!(
            status_at(Some(&saved(answer(&[ME], false), now)), now, ME),
            Status::NotPro(_)
        ));
        assert!(matches!(
            status_at(Some(&saved(answer(&["gitagent"], true), now)), now, ME),
            Status::NotPro(_)
        ));
        assert_eq!(status_at(None, now, ME), Status::Free);
        let unchecked = Saved {
            licence: "ABCDE-FGHJK-LMNPQ-RSTUV".into(),
            ..Saved::default()
        };
        assert!(!status_at(Some(&unchecked), now, ME).is_pro());
    }

    #[test]
    fn the_title_says_pro_or_that_pro_has_lapsed() {
        let now = 1_000 * DAY;
        let pro = status_at(Some(&saved(answer(&[ME], true), now)), now, ME);
        assert_eq!(pro.title_badge_for(ME), " \u{b7} \u{2726} Pro");
        let unpaid = status_at(Some(&saved(answer(&[ME], false), now)), now, ME);
        assert_eq!(unpaid.title_badge_for(ME), " \u{b7} Pro inactive");
        let other_app = status_at(Some(&saved(answer(&["gitagent"], false), now)), now, ME);
        assert_eq!(other_app.title_badge_for(ME), "");
        assert_eq!(Status::Free.title_badge_for(ME), "");
    }

    #[test]
    fn a_free_licence_gives_ai_credits_but_not_pro() {
        let now = 1_000 * DAY;
        let mut free = answer(&[], true);
        free.credits.left = 50;
        let status = status_at(Some(&saved(free, now)), now, ME);
        assert!(!status.is_pro());
        assert!(matches!(status, Status::NotPro(_)));
        assert_eq!(status.free_credits().map(|a| a.credits.left), Some(50));
        // A paid plan, or a lapsed free one, isn't the free allowance.
        let paid = status_at(Some(&saved(answer(&[ME], true), now)), now, ME);
        assert!(paid.free_credits().is_none());
        assert!(status_at(Some(&saved(answer(&[], false), now)), now, ME)
            .free_credits()
            .is_none());
    }

    #[test]
    fn api0s_reply_to_a_free_credits_request_is_read() {
        assert_eq!(
            free_reply(
                200,
                r#"{"success":true,"emailed":true,"message":"On its way."}"#
            ),
            Ok("On its way.".into())
        );
        assert_eq!(
            free_reply(404, r#"{"message":"There are no free credits right now"}"#),
            Err(FreeError::Unavailable)
        );
        assert!(
            matches!(free_reply(409, "{}"), Err(FreeError::Refused(m)) if m.contains("already has a plan"))
        );
        assert_eq!(
            free_reply(400, r#"{"message":"Enter the email"}"#),
            Err(FreeError::Refused("Enter the email".into()))
        );
        assert!(plausible_email(" dev@example.ch "));
        assert!(!plausible_email("dev@example"));
        assert!(!plausible_email("example.ch"));
    }

    #[test]
    fn pro_lasts_seven_days_after_the_last_check_that_got_through() {
        let checked = 1_000 * DAY;
        let s = saved(answer(&[ME], true), checked);
        assert!(status_at(Some(&s), checked + 6 * DAY, ME).is_pro());
        assert!(status_at(Some(&s), checked + 7 * DAY, ME).is_pro());
        let late = status_at(Some(&s), checked + 7 * DAY + 1, ME);
        assert!(matches!(late, Status::Offline(_)));
        assert!(!late.is_pro());
        // A clock set back doesn't take Pro away.
        assert!(status_at(Some(&s), checked - DAY, ME).is_pro());
    }

    #[test]
    fn the_licence_is_checked_again_after_a_day() {
        let checked = 1_000 * DAY;
        let s = saved(answer(&[ME], true), checked);
        assert!(!check_due(&s, checked + DAY - 1));
        assert!(check_due(&s, checked + DAY));
        assert!(check_due(
            &Saved {
                licence: s.licence.clone(),
                ..Saved::default()
            },
            checked
        ));
    }

    #[test]
    fn api0s_answer_is_read() {
        let json = r#"{"object":"licence","licence":"ABCDE-FGHJK-LMNPQ-RSTUV","email":"dev@example.ch",
            "plan":{"id":"studio","name":"Studio"},"products":["publishkit","splitter"],"active":true,
            "status":"active","cancel_at_period_end":true,"period_end":"2026-11-21T10:00:00Z",
            "credits":{"granted":500,"used":120,"left":380},"checked_at":"2026-10-21T10:00:00Z"}"#;
        let a: Answer = serde_json::from_str(json).unwrap();
        assert!(a.unlocks_for(ME));
        assert_eq!(a.plan.name, "Studio");
        assert_eq!(a.credits.left, 380);
        assert_eq!(a.renewal(), "Ends on 2026-11-21");
    }

    #[test]
    fn the_licence_typed_in_one_app_is_shared_with_every_other() {
        let now = 1_000 * DAY;
        let mine = saved(answer(&["*"], true), now);
        let other = "BCDEF-GHJKL-MNPQR-STUVW".to_string();
        // Already the same: this app's last answer stands.
        assert_eq!(
            reconcile(Shared::Licence(mine.licence.clone()), Some(mine.clone())),
            (Some(mine.clone()), false)
        );
        // Typed in another app: taken, and checked again.
        let (taken, share) = reconcile(Shared::Licence(other.clone()), Some(mine.clone()));
        assert!(!share);
        let taken = taken.unwrap();
        assert_eq!(taken.licence, other);
        assert!(taken.answer.is_none());
        assert!(check_due(&taken, now));
        assert_eq!(
            reconcile(Shared::Licence(other.clone()), None)
                .0
                .map(|s| s.licence),
            Some(other)
        );
        // Removed in another app: removed here too.
        assert_eq!(
            reconcile(Shared::Removed, Some(mine.clone())),
            (None, false)
        );
        // Saved before the shared file existed: shared with the others.
        assert_eq!(
            reconcile(Shared::Missing, Some(mine.clone())),
            (Some(mine), true)
        );
        assert_eq!(reconcile(Shared::Missing, None), (None, false));
    }
}
