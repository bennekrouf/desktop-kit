//! The header itself, the two badges apps put in it, and the toolbar under it.

use dioxus::prelude::*;

use super::STYLE;
use crate::licence::Status;

/// The bar across the top, the same in every app: the model's flag, the gear
/// that opens Settings, then MCP, Feedback and the licence. Nothing else: what
/// an app switches between goes in a [`Toolbar`] under it, and everything it
/// sets up goes in its Settings.
#[component]
pub fn Header(
    /// The model this app runs on: a [`ModelBadge`]. None for an app without one.
    model: Option<Element>,
    on_settings: EventHandler<()>,
    on_mcp: Option<EventHandler<()>>,
    /// A [`ProBadge`]: last, so the one thing about the person rather than the
    /// work sits in the corner.
    pro: Option<Element>,
) -> Element {
    let name = crate::app().map_or("", |a| a.name);
    rsx! {
        style { {STYLE} }
        header { class: "kit-header",
            div { class: "kit-right",
                {model}
                button {
                    class: "kit-btn kit-icon",
                    title: "Settings",
                    aria_label: "Settings",
                    onclick: move |_| on_settings.call(()),
                    svg { view_box: "0 0 24 24", "aria-hidden": "true",
                        path { d: "M12 15a3 3 0 100-6 3 3 0 000 6z" }
                        path { d: "M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 11-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 11-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 11-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 110-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 112.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 114 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 112.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 110 4h-.09a1.65 1.65 0 00-1.51 1z" }
                    }
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
                {pro}
            }
        }
    }
}

/// The slim bar under the header for what an app switches between (its tabs)
/// and the one or two things it is for (Record, Open…). Apps' full-height
/// layouts keep using `--kit-header-h`: it grows by this bar's height while
/// the bar is there.
#[component]
pub fn Toolbar(children: Element) -> Element {
    rsx! {
        style { {STYLE} }
        div { class: "kit-toolbar", {children} }
    }
}

/// The model the app runs on, as its flag alone, e.g. Europe's for api0; a
/// grey one for a model with no region (an Ollama model). The name shows on
/// hover, and clicking opens where it is chosen.
#[component]
pub fn ModelBadge(
    label: String,
    /// The api0 region's id, to draw its flag.
    region: Option<&'static str>,
    title: String,
    onclick: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            class: "kit-btn kit-icon kit-model",
            title: "{label} \u{2014} {title}",
            aria_label: "{label}",
            onclick: move |_| onclick.call(()),
            super::RegionFlag { id: region.unwrap_or("") }
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
