//! Opening a web page in the person's browser, never in the app's webview.

/// Opens an http(s) `url` in the default browser. Anything else is refused,
/// and `false`: the address can come from a remote file (`latest.json`, a
/// notice), and a `file:` or custom-scheme link must not be launched from an
/// app.
pub fn open(url: &str) -> bool {
    if !is_web_url(url) {
        return false;
    }
    let url = url.trim().to_string();
    std::thread::spawn(move || {
        let _ = if cfg!(target_os = "macos") {
            std::process::Command::new("open").arg(&url).status()
        } else if cfg!(windows) {
            // Not `cmd /C start`: cmd would read the `&` of a query string as
            // the start of another command.
            std::process::Command::new("rundll32")
                .args(["url.dll,FileProtocolHandler", &url])
                .status()
        } else {
            std::process::Command::new("xdg-open").arg(&url).status()
        };
    });
    true
}

/// A plain http(s) address, with nothing a shell could read as more than one.
pub fn is_web_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && !url
            .trim()
            .chars()
            .any(|c| c.is_whitespace() || "\"'`|^<>".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_web_urls_are_opened() {
        assert!(is_web_url(
            "https://mayorana.ch/downloads/surfacewatch/v0.2.0/x.dmg?src=updater"
        ));
        assert!(is_web_url(
            " https://billing.stripe.com/p/session/abc?x=1&y=2 "
        ));
        assert!(!is_web_url("file:///etc/passwd"));
        assert!(!is_web_url("javascript:alert(1)"));
        assert!(!is_web_url("https://example.com/a b"));
        assert!(!is_web_url("https://example.com/\"; rm -rf ~"));
        assert!(!open("smb://server/share"));
    }
}
