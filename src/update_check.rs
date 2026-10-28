//! Whether a newer release is published, and where this computer gets it.
//!
//! Fetches the `latest.json` published with each release, beside the builds on
//! mayorana.ch, and compares its version with the app's. Cheap and without side
//! effects, so it can run in the background at start-up. It used to be fetched
//! from the GitHub release, which broke every installed copy's update check
//! the moment a repository stopped being publicly readable: the app should not
//! depend on the source host to tell people a build exists.
//!
//! Setting `DISABLE_UPDATE_CHECK` turns it off. `MAYORANA_UPDATE_URL` replaces
//! the address, to try a staging `latest.json` before it goes live.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
struct LatestJson {
    version: String,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    platforms: Platforms,
}

/// Builds per OS. The publisher keys them by package format (`dmg`,
/// `exe_or_msi`, `tarball`…) in most apps and by CPU (`aarch64`, `x86_64`) in
/// others. A `BTreeMap`, so every pick is the same on every launch: `HashMap`
/// iteration order is randomised per process.
#[derive(Debug, Default, Deserialize)]
struct Platforms {
    #[serde(default)]
    macos: BTreeMap<String, Artifact>,
    #[serde(default)]
    windows: BTreeMap<String, Artifact>,
    #[serde(default)]
    linux: BTreeMap<String, Artifact>,
}

#[derive(Debug, Deserialize)]
struct Artifact {
    url: String,
    #[serde(default)]
    sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateInfo {
    pub latest_version: String,
    pub latest_tag: String,
    /// Direct link to this computer's build, so the banner's button downloads
    /// the build itself rather than opening a page to pick one from. The app's
    /// page on mayorana.ch when no build fits.
    pub download_url: String,
    /// The published digest of that build, empty when there is none.
    ///
    /// The banner hands the link to the person's browser, so the app never sees
    /// the downloaded bytes and cannot check them. An app may show the digest,
    /// so that someone who cares can.
    pub sha256: String,
}

/// `Some` if a newer release is published. Any network or parse failure is
/// `None`, as is any call before [`crate::init`]. Never panics.
pub async fn check() -> Option<UpdateInfo> {
    if std::env::var_os("DISABLE_UPDATE_CHECK").is_some() {
        return None;
    }
    let app = crate::app()?;
    let url = std::env::var("MAYORANA_UPDATE_URL").unwrap_or_else(|_| {
        format!(
            "https://mayorana.ch/downloads/{}/latest/latest.json",
            app.id
        )
    });
    let response = crate::http::get(&url, "updater", TIMEOUT).await?;
    newer_than(
        &response.body,
        app.version,
        std::env::consts::OS,
        std::env::consts::ARCH,
        &app_page(app.id),
    )
}

/// Where to send someone whose computer has no build in `latest.json`.
fn app_page(id: &str) -> String {
    format!("https://mayorana.ch/en/apps/{id}")
}

/// What `body` says, if it names a release newer than `current`.
fn newer_than(
    body: &str,
    current: &str,
    os: &str,
    arch: &str,
    fallback: &str,
) -> Option<UpdateInfo> {
    let latest: LatestJson = serde_json::from_str(body).ok()?;
    if !is_newer(&latest.version, current) {
        return None;
    }
    let (download_url, sha256) = match pick(os, arch, &latest.platforms) {
        // Marks the hit as coming from an existing install. The banner opens
        // this in the person's browser, so the updater's own User-Agent is not
        // what fetches the file: without the marker the request looks like a
        // first-time download off the website. nginx serves the file
        // regardless of the query string.
        Some(a) => (format!("{}?src=updater", a.url), a.sha256.clone()),
        None => (fallback.to_string(), String::new()),
    };
    Some(UpdateInfo {
        latest_version: latest.version,
        latest_tag: latest.tag,
        download_url,
        sha256,
    })
}

/// Formats to offer, best first, per OS. On Linux, an AppImage runs on any
/// distribution without installing anything as root.
fn preferred_formats(os: &str) -> &'static [&'static str] {
    match os {
        "macos" => &["dmg"],
        "windows" => &["msi", "exe", "exe_or_msi"],
        "linux" => &["appimage", "deb", "tarball"],
        _ => &[],
    }
}

/// The build for `os`: the preferred format that is published, else the one
/// for this CPU, else the only one. Never an arbitrary pick from several: with
/// an Apple Silicon and an Intel build listed, guessing would send half the
/// people a program their computer cannot run.
fn pick<'a>(os: &str, arch: &str, platforms: &'a Platforms) -> Option<&'a Artifact> {
    let builds = match os {
        "macos" => &platforms.macos,
        "windows" => &platforms.windows,
        "linux" => &platforms.linux,
        _ => return None,
    };
    preferred_formats(os)
        .iter()
        .find_map(|format| builds.get(*format))
        .or_else(|| for_cpu(arch, builds))
        .or_else(|| {
            (builds.len() == 1)
                .then(|| builds.values().next())
                .flatten()
        })
        .filter(|a| !a.url.is_empty())
}

/// The entry for this CPU. Keys are matched loosely: the publisher names them
/// by target triple (`aarch64-apple-darwin`) as often as by bare arch, and
/// `arm64` is the same machine as `aarch64`.
fn for_cpu<'a>(arch: &str, builds: &'a BTreeMap<String, Artifact>) -> Option<&'a Artifact> {
    let aliases: &[&str] = match arch {
        "aarch64" => &["aarch64", "arm64"],
        "x86_64" => &["x86_64", "amd64", "x64"],
        other => return builds.get(other),
    };
    aliases
        .iter()
        .find_map(|alias| builds.get(*alias))
        .or_else(|| {
            aliases.iter().find_map(|alias| {
                builds
                    .iter()
                    .find(|(k, _)| k.contains(alias))
                    .map(|(_, a)| a)
            })
        })
}

fn is_newer(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Option<(u32, u32, u32)> {
        let mut parts = s.trim_start_matches('v').split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.split(['-', '+']).next()?.parse().ok()?;
        Some((major, minor, patch))
    };
    match (parse(a), parse(b)) {
        (Some(av), Some(bv)) => av > bv,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = "https://mayorana.ch/en/apps/x";

    fn latest(platforms: &str) -> String {
        format!(r#"{{"version":"9.0.0","tag":"v9.0.0","platforms":{platforms}}}"#)
    }

    fn url_for(os: &str, arch: &str, platforms: &str) -> String {
        newer_than(&latest(platforms), "1.0.0", os, arch, PAGE)
            .unwrap()
            .download_url
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(is_newer("v1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0-beta", "0.1.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn the_same_or_an_older_release_is_no_update() {
        assert_eq!(
            newer_than(&latest("{}"), "9.0.0", "macos", "aarch64", PAGE),
            None
        );
        assert_eq!(
            newer_than("not json", "1.0.0", "macos", "aarch64", PAGE),
            None
        );
    }

    #[test]
    fn the_preferred_format_wins_and_is_marked_as_an_update() {
        let p = r#"{"linux":{"tarball":{"url":"https://x/t.tgz"},"appimage":{"url":"https://x/a.AppImage"}},
                    "windows":{"exe_or_msi":{"url":"https://x/s.exe"},"msi":{"url":"https://x/s.msi"}}}"#;
        assert_eq!(
            url_for("linux", "x86_64", p),
            "https://x/a.AppImage?src=updater"
        );
        assert_eq!(
            url_for("windows", "x86_64", p),
            "https://x/s.msi?src=updater"
        );
    }

    /// With two CPU builds listed, a `HashMap` pick offered whichever the hash
    /// order yielded, so an Apple Silicon user got an Intel build about half
    /// the time.
    #[test]
    fn the_build_for_this_cpu_is_chosen_not_an_arbitrary_one() {
        let p = r#"{"macos":{"x86_64-apple-darwin":{"url":"https://x/intel.dmg","sha256":"i"},
                             "aarch64-apple-darwin":{"url":"https://x/arm.dmg","sha256":"a"}}}"#;
        let arm = newer_than(&latest(p), "1.0.0", "macos", "aarch64", PAGE).unwrap();
        assert_eq!(arm.download_url, "https://x/arm.dmg?src=updater");
        assert_eq!(arm.sha256, "a");
        assert_eq!(
            url_for("macos", "x86_64", p),
            "https://x/intel.dmg?src=updater"
        );
        let arm64 = r#"{"macos":{"arm64":{"url":"https://x/arm.dmg"},"x64":{"url":"https://x/intel.dmg"}}}"#;
        assert_eq!(
            url_for("macos", "aarch64", arm64),
            "https://x/arm.dmg?src=updater"
        );
    }

    #[test]
    fn a_single_build_is_taken_as_it_is_and_several_unknown_ones_are_not_guessed() {
        let one = r#"{"linux":{"something":{"url":"https://x/only"}}}"#;
        assert_eq!(
            url_for("linux", "x86_64", one),
            "https://x/only?src=updater"
        );
        let two = r#"{"linux":{"a":{"url":"https://x/a"},"b":{"url":"https://x/b"}}}"#;
        assert_eq!(url_for("linux", "x86_64", two), PAGE);
    }

    #[test]
    fn no_build_for_this_computer_leads_to_the_app_page() {
        assert_eq!(url_for("macos", "aarch64", "{}"), PAGE);
        assert_eq!(
            url_for(
                "freebsd",
                "x86_64",
                r#"{"linux":{"deb":{"url":"https://x/d"}}}"#
            ),
            PAGE
        );
        let empty = r#"{"macos":{"dmg":{"url":""}}}"#;
        assert_eq!(url_for("macos", "aarch64", empty), PAGE);
        assert_eq!(app_page("trackcut"), "https://mayorana.ch/en/apps/trackcut");
    }
}
