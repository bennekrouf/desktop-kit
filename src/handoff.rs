//! Notes one Mayorana app leaves for another: "open this video in Repurpose", "show this take".
//! A small JSON file in `…/mayorana/handoff/`, beside the shared licence, named for the app and
//! what it should do: `publishkit-repurpose.json`. The app it is for picks it up when it opens,
//! or while it runs, and deletes it. A note older than an hour is stale and ignored.

use serde_json::Value;
use std::path::PathBuf;

/// How long a note waits for its app.
pub const MAX_AGE_SECS: u64 = 3600;

/// `…/mayorana/handoff/<app>-<action>.json`.
pub fn path(app: &str, action: &str) -> Option<PathBuf> {
    Some(
        crate::licence::shared_path()?
            .parent()?
            .join("handoff")
            .join(format!("{app}-{action}.json")),
    )
}

/// Leaves `value` (a JSON object) for `app`, stamped with the time and the app it is `from`.
pub fn send(app: &str, action: &str, from: &str, mut value: Value) -> Result<PathBuf, String> {
    let note =
        path(app, action).ok_or("This computer has no settings folder to hand it over through")?;
    if let Some(dir) = note.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    }
    if let Some(o) = value.as_object_mut() {
        o.insert("from".into(), from.into());
        o.insert("at".into(), now().into());
    }
    std::fs::write(&note, serde_json::to_vec_pretty(&value).unwrap_or_default())
        .map_err(|e| format!("Couldn't write {}: {e}", note.display()))?;
    Ok(note)
}

/// The note left for this app, if a fresh one is waiting; it is deleted either way.
pub fn take(app: &str, action: &str) -> Option<Value> {
    let note = path(app, action)?;
    let text = std::fs::read_to_string(&note).ok()?;
    let _ = std::fs::remove_file(&note);
    fresh(&text, now())
}

fn fresh(text: &str, now: u64) -> Option<Value> {
    let v: Value = serde_json::from_str(text).ok()?;
    let at = v["at"].as_u64()?;
    (now.saturating_sub(at) <= MAX_AGE_SECS).then_some(v)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_fresh_note_is_taken() {
        assert_eq!(
            fresh(r#"{"open":"x","at":1000}"#, 1000 + MAX_AGE_SECS).unwrap()["open"],
            "x"
        );
        assert!(fresh(r#"{"open":"x","at":1000}"#, 1001 + MAX_AGE_SECS).is_none());
        assert!(fresh(r#"{"open":"x"}"#, 1000).is_none());
        assert!(fresh("not json", 1000).is_none());
    }
}
