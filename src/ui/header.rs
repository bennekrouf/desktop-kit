//! The header itself, and the two badges apps put in it.

use dioxus::prelude::*;

use super::{Theme, ThemeToggle, STYLE};
use crate::licence::Status;

/// The bar across the top. The name and version come from [`crate::init`];
/// Settings and MCP show only when the app passes what they open.
#[component]
pub fn Header(
    theme: Theme,
    /// The middle: what the app switches between (tabs, a site, a path).
    #[props(default)]
    children: Element,
    /// The app's own buttons, on the right before the shared ones.
    actions: Option<Element>,
    /// The model this app runs on: a [`ModelBadge`], or the app's own picker.
    model: Option<Element>,
    on_settings: Option<EventHandler<()>>,
    on_mcp: Option<EventHandler<()>>,
    /// A [`ProBadge`]: last, so the one thing about the person rather than the
    /// work sits in the corner.
    pro: Option<Element>,
) -> Element {
    let (name, version) = crate::app().map_or(("", ""), |a| (a.name, a.version));
    rsx! {
        style { {STYLE} }
        header { class: "kit-header",
            div { class: "kit-brand",
                span { class: "kit-name", "{name}" }
                if !version.is_empty() {
                    span { class: "kit-version", title: "Installed version", "v{version}" }
                }
            }
            div { class: "kit-middle", {children} }
            div { class: "kit-right",
                {actions}
                {model}
                if let Some(open) = on_settings {
                    button { class: "kit-btn", onclick: move |_| open.call(()), "Settings" }
                }
                if let Some(open) = on_mcp {
                    button {
                        class: "kit-btn",
                        title: "Use {name} from Claude Desktop or Claude Code",
                        onclick: move |_| open.call(()),
                        "MCP"
                    }
                }
                // A link, not a button: the webview hands it to the browser.
                a {
                    class: "kit-btn",
                    href: "{crate::feedback::contact_url()}",
                    target: "_blank",
                    title: "Ideas, bugs, questions: send them to the person who makes {name}",
                    "Feedback"
                }
                ThemeToggle { theme }
                {pro}
            }
        }
    }
}

/// The model the app runs on, e.g. Europe with its flag for api0, or an Ollama
/// model's name; clicking it opens where it is chosen.
#[component]
pub fn ModelBadge(
    label: String,
    /// The api0 region's id, to draw its flag before the label.
    region: Option<&'static str>,
    title: String,
    onclick: EventHandler<()>,
) -> Element {
    rsx! {
        button { class: "kit-btn kit-model", title, onclick: move |_| onclick.call(()),
            if let Some(id) = region {
                super::RegionFlag { id }
            }
            "{label}"
        }
    }
}

/// How the licence badge looks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProLook {
    /// Pro is active.
    Pro,
    /// There is a licence, but it doesn't make the app Pro right now.
    Renew,
    /// No licence, or free AI credits only.
    Get,
}

impl ProLook {
    /// The look for a licence status of the shared licence core.
    pub fn of(status: &Status) -> Self {
        match status {
            Status::Pro(_) => Self::Pro,
            Status::Free => Self::Get,
            Status::NotPro(a) if a.is_free() => Self::Get,
            Status::NotPro(_) | Status::Offline(_) | Status::Unknown(_) => Self::Renew,
        }
    }
}

/// The licence, in the header's corner. The app says what it means in `title`
/// and what clicking opens.
#[component]
pub fn ProBadge(look: ProLook, title: String, onclick: EventHandler<()>) -> Element {
    let (class, text) = match look {
        ProLook::Pro => ("kit-pro", "PRO"),
        ProLook::Renew => ("kit-pro kit-pro-renew", "RENEW PRO"),
        ProLook::Get => ("kit-btn kit-get-pro", "Get Pro\u{2026}"),
    };
    rsx! {
        button { class, title, onclick: move |_| onclick.call(()),
            if look != ProLook::Get {
                span { class: "kit-pro-star", "\u{2726}" }
            }
            span { class: "kit-pro-text", "{text}" }
        }
    }
}
