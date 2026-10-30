//! The messages that can show under the header: a newer release, a notice
//! from mayorana.ch and, once, the usage-statistics notice.
//!
//! Each looks for itself a few seconds after start-up, so neither slows a cold
//! start, and says nothing when it finds nothing or can't ask. The update
//! check looks again once a day while the app stays open.

use std::time::Duration;

use dioxus::prelude::*;

use crate::notice::{self, Notice};
use crate::telemetry::{self, Event};
use crate::update_check::{self, UpdateInfo};

/// The banners' stylesheet. [`Banners`] adds it itself.
const STYLE: &str = include_str!("banners.css");

/// The update and notice banners under the header, and the statistics notice
/// along the bottom. Put it once, beside the [`super::Header`].
#[component]
pub fn Banners(
    /// The shared usage statistics: records the launch, sends what was
    /// recorded, and asks once. Off for an app that keeps none, or its own.
    #[props(default)]
    statistics: bool,
    /// For an app with its own statistics: the version an update banner
    /// offers, when it shows, and when its Download is clicked.
    on_update_offered: Option<EventHandler<String>>,
    on_update_clicked: Option<EventHandler<String>>,
    /// The app's own banners, under these, in the same style: a `div` with
    /// the `kit-banner` class.
    #[props(default)]
    children: Element,
) -> Element {
    let mut update = use_signal(|| Option::<UpdateInfo>::None);
    let mut notice = use_signal(|| Option::<Notice>::None);
    let mut ask = use_signal(|| false);

    // At start-up, then once a day for as long as the app stays open: a
    // release published while it runs would otherwise wait for the next
    // launch, which for an app left open for days is no notice at all.
    use_future(move || async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        // Each version is offered once per launch, so a banner the person
        // hid stays hidden until a newer release, and the statistics count
        // one offer, not one a day.
        let mut offered = None::<String>;
        loop {
            if let Some(info) = update_check::check().await {
                if offered.as_deref() != Some(info.latest_version.as_str()) {
                    let to = info.latest_version.clone();
                    offered = Some(to.clone());
                    match on_update_offered {
                        Some(offered) => offered.call(to),
                        None if statistics => telemetry::record(Event::UpdateOffered { to }),
                        None => {}
                    }
                    update.set(Some(info));
                }
            }
            wait_a_day().await;
        }
    });
    use_future(move || async move {
        tokio::time::sleep(Duration::from_secs(4)).await;
        notice.set(notice::fetch().await);
    });
    use_future(move || async move {
        if !statistics {
            return;
        }
        if telemetry::start().await {
            ask.set(true);
        }
        telemetry::flush_forever().await;
    });
    // Recorded while the notice is on screen, so collection starts from the
    // next event, never from one the person had no chance to read about.
    use_effect(move || {
        if ask() {
            telemetry::mark_informed();
        }
    });

    let name = crate::app().map_or("", |a| a.name);
    let version = crate::app().map_or("", |a| a.version);
    rsx! {
        style { {STYLE} }
        div { class: "kit-banners",
            if let Some(info) = update() {
                div { class: "kit-banner", role: "status",
                    span { class: "kit-banner-text",
                        "{name} "
                        strong { "{info.latest_version}" }
                        " is available (you have {version})."
                    }
                    // A button, not a link: the address comes from a remote
                    // file and opens in the browser, never in the app.
                    button {
                        class: "kit-banner-link",
                        onclick: {
                            let info = info.clone();
                            move |_| {
                                let to = info.latest_version.clone();
                                match on_update_clicked {
                                    Some(clicked) => clicked.call(to),
                                    None if statistics => telemetry::record(Event::UpdateClicked { to }),
                                    None => {}
                                }
                                crate::browser::open(&info.download_url);
                            }
                        },
                        "Download"
                    }
                    if !info.sha256.is_empty() {
                        span {
                            class: "kit-banner-sha",
                            title: "SHA-256 of the published build: compare it with the file you downloaded. Full value: {info.sha256}",
                            "sha256 {short(&info.sha256)}…"
                        }
                    }
                    button {
                        class: "kit-banner-close",
                        title: "Hide until the next launch",
                        aria_label: "Hide the update notice",
                        onclick: move |_| update.set(None),
                        "×"
                    }
                }
            }
            if let Some(n) = notice() {
                div { class: "kit-banner kit-notice", role: "status",
                    span { class: "kit-banner-text", "{n.text}" }
                    if let Some(url) = n.url.clone() {
                        button {
                            class: "kit-banner-link",
                            onclick: move |_| {
                                crate::browser::open(&url);
                            },
                            {n.link_text.clone().unwrap_or_else(|| "Open".to_string())}
                        }
                    }
                    button {
                        class: "kit-banner-close",
                        title: "Don't show this message again",
                        aria_label: "Dismiss the message",
                        onclick: move |_| {
                            notice::dismiss(&n.id);
                            notice.set(None);
                        },
                        "×"
                    }
                }
            }
            {children}
        }
        // Along the bottom, so it never sits under the other banners. Either
        // answer is remembered.
        if ask() {
            div { class: "kit-banner kit-consent", role: "status",
                span { class: "kit-banner-text",
                    strong { "{name} shares anonymous usage statistics. " }
                    "Whether it is installed and opened, and its version and operating system \
                     — never your files, data, accounts or anything you type."
                }
                button {
                    class: "kit-banner-link",
                    onclick: move |_| {
                        telemetry::set_consent(true);
                        ask.set(false);
                    },
                    "OK"
                }
                button {
                    class: "kit-banner-link",
                    onclick: move |_| {
                        telemetry::set_consent(false);
                        ask.set(false);
                    },
                    "Turn off"
                }
            }
        }
    }
}

/// Returns once a day of wall-clock time has passed. Measured against the
/// clock, an hour at a time, rather than with one long sleep: tokio's timer
/// stops while the computer sleeps, so a 24-hour sleep on a laptop closed
/// every night could take a week.
async fn wait_a_day() {
    const DAY: Duration = Duration::from_secs(24 * 60 * 60);
    let since = std::time::SystemTime::now();
    loop {
        tokio::time::sleep(Duration::from_secs(60 * 60)).await;
        // A clock set back counts as a day gone, rather than as a wait that
        // never ends.
        if since.elapsed().map_or(true, |waited| waited >= DAY) {
            return;
        }
    }
}

/// The first 12 characters of a digest, enough to compare by eye.
fn short(digest: &str) -> &str {
    digest.get(..12).unwrap_or(digest)
}
