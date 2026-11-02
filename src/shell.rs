//! What the person's login shell knows and an app opened from Finder does not.
//!
//! An app launched from Finder, the Dock or a launcher gets roughly
//! `/usr/bin:/bin:/usr/sbin:/sbin` and none of the variables a profile
//! exports. These ask a non-interactive login shell (`-lc`: it reads the
//! profile, and no prompt can block), giving up after three seconds so a hung
//! profile is a short pause rather than an app that never opens.

use std::collections::BTreeMap;

const MARKER: &str = "__DESKTOP_KIT_PATH__";

/// `PATH` as the login shell sets it. `None` off Unix, or when the shell
/// fails, hangs, or says nothing.
pub fn login_path() -> Option<String> {
    let out = run(&format!("printf '{MARKER}%s' \"$PATH\""))?;
    after_marker(&out)
}

/// The named variables as the login shell sees them, those set and non-empty.
pub fn login_vars<S: AsRef<str>>(names: &[S]) -> BTreeMap<String, String> {
    let script = names
        .iter()
        .map(|k| format!("printf '{k}=%s\\n' \"${k}\"", k = k.as_ref()))
        .collect::<Vec<_>>()
        .join("; ");
    run(&script).map_or_else(BTreeMap::new, |out| parse_vars(&out, names))
}

/// `:`-separated lists joined in order, each folder once, blanks dropped: the
/// first place a tool is found stays the one that wins.
pub fn merge_paths(lists: &[&str]) -> String {
    let mut out: Vec<&str> = vec![];
    for entry in lists.iter().flat_map(|l| l.split(':')).map(str::trim) {
        if !entry.is_empty() && !out.contains(&entry) {
            out.push(entry);
        }
    }
    out.join(":")
}

/// The text after the marker: profiles sometimes print banners before it.
fn after_marker(out: &str) -> Option<String> {
    let path = out.rsplit_once(MARKER)?.1.trim();
    (!path.is_empty()).then(|| path.to_string())
}

/// `KEY=value` lines, keeping only the names asked for. Profiles talk (nvm,
/// conda, banners), so anything else shaped like `KEY=value` is ignored.
fn parse_vars<S: AsRef<str>>(text: &str, names: &[S]) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        .filter(|(k, v)| !v.is_empty() && names.iter().any(|n| n.as_ref() == *k))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[cfg(all(unix, not(target_os = "android")))]
fn run(script: &str) -> Option<String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/zsh".into());
    let mut child = Command::new(shell)
        .args(["-lc", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Read while waiting, so a profile that prints a lot cannot fill the pipe.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        buf
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    let ok = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Err(_) => break false,
            Ok(None) if Instant::now() >= deadline => {
                // Kill and reap, or the hung shell lingers as a zombie.
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let out = reader.join().unwrap_or_default();
    ok.then_some(out)
}

/// Windows applications get the user's environment from the system: there is
/// no login shell to ask.
#[cfg(not(all(unix, not(target_os = "android"))))]
fn run(_: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_login_shell_is_believed_before_anything_else() {
        let merged = merge_paths(&["/opt/homebrew/bin:/usr/bin", "/usr/bin:/bin"]);
        assert_eq!(merged, "/opt/homebrew/bin:/usr/bin:/bin");
    }

    #[test]
    fn empty_and_ragged_input_does_not_produce_empty_entries() {
        assert_eq!(merge_paths(&["::/usr/bin: :", ""]), "/usr/bin");
    }

    #[test]
    fn the_path_is_read_after_profile_banners() {
        let out = format!("Welcome!\nPATH=/evil\n{MARKER}/a:/b\n");
        assert_eq!(after_marker(&out).as_deref(), Some("/a:/b"));
        assert_eq!(after_marker("no marker"), None);
    }

    #[test]
    fn only_the_variables_asked_for_are_taken() {
        let out = "nvm says hi\nEVIL=1\nGH_TOKEN=abc\nDO_NOT_TRACK=\n";
        let vars = parse_vars(out, &["GH_TOKEN", "DO_NOT_TRACK"]);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars["GH_TOKEN"], "abc");
    }

    #[cfg(unix)]
    #[test]
    fn the_login_shell_probe_returns_a_path_or_nothing() {
        if let Some(path) = login_path() {
            assert!(path.contains('/'), "got {path}");
        }
    }
}
