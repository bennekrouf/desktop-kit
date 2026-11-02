//! What every app's Settings has: the theme and the installed version. An app
//! with a Settings screen of its own puts [`SettingsGeneral`] in it; one
//! without opens [`SettingsWindow`] from the header's gear, with its own
//! buttons inside.

use dioxus::prelude::*;

use super::{Theme, STYLE};

/// Light or dark, and the installed version.
#[component]
pub fn SettingsGeneral(theme: Theme) -> Element {
    let (name, version) = crate::app().map_or(("", ""), |a| (a.name, a.version));
    let light = (theme.light)();
    let pick = move |want_light: bool| {
        if light != want_light {
            theme.toggle();
        }
    };
    rsx! {
        style { {STYLE} }
        div { class: "kit-set-general",
            div { class: "kit-set-row",
                span { class: "kit-set-label", "Appearance" }
                div { class: "kit-set-seg",
                    button { class: if light { "" } else { "kit-on" }, onclick: move |_| pick(false), "Dark" }
                    button { class: if light { "kit-on" } else { "" }, onclick: move |_| pick(true), "Light" }
                }
            }
            if !version.is_empty() {
                div { class: "kit-set-row",
                    span { class: "kit-set-label", "Version" }
                    span { class: "kit-set-muted", "{name} v{version}" }
                }
            }
        }
    }
}

/// The Settings window for an app without a screen of its own: its buttons
/// (`children`, as `kit-set-btn`s in a `kit-set-actions` row), then
/// [`SettingsGeneral`]. Esc or a click beside it calls `on_close`.
#[component]
pub fn SettingsWindow(
    theme: Theme,
    on_close: EventHandler<()>,
    #[props(default)] children: Element,
) -> Element {
    rsx! {
        style { {STYLE} }
        div { class: "kit-set-backdrop", onclick: move |_| on_close.call(()),
            div {
                class: "kit-set-window",
                // Focused when it opens, so Esc reaches it without a click first.
                tabindex: "-1",
                onmounted: move |e: Event<MountedData>| {
                    spawn(async move {
                        let _ = e.set_focus(true).await;
                    });
                },
                onclick: move |e| e.stop_propagation(),
                onkeydown: move |e| {
                    if e.key() == Key::Escape {
                        e.stop_propagation();
                        on_close.call(());
                    }
                },
                div { class: "kit-set-head",
                    h2 { "Settings" }
                    button { class: "kit-set-close", title: "Close (Esc)", onclick: move |_| on_close.call(()), "×" }
                }
                {children}
                SettingsGeneral { theme }
            }
        }
    }
}
