//! The Pro window every app shows: what the free version does, the licence on
//! this computer, and where to paste a licence number or ask for free AI
//! credits. What differs between apps is what Pro adds, which the app passes
//! in as its pitch.

use std::time::Duration;

use dioxus::prelude::*;

use super::{ProBadge, ProLook};
use crate::licence::{self, FreeError, Status};

/// The window's stylesheet. [`ProWindow`] adds it itself.
const STYLE: &str = include_str!("pro.css");

/// Sends `true` for each Escape pressed in the page. The handler is kept on
/// `window` and replaced on each mount, so two never stack.
const ESCAPE_JS: &str =
    "if (window._proEscape) window.removeEventListener('keydown', window._proEscape);\
     window._proEscape = (e) => { if (e.key === 'Escape') dioxus.send(true); };\
     window.addEventListener('keydown', window._proEscape);";

/// Whether the Pro window is open, and the licence as last checked. Shared by
/// everything that opens the window or depends on Pro.
#[derive(Clone, Copy, PartialEq)]
pub struct Pro {
    pub open: Signal<bool>,
    pub status: Signal<Status>,
}

/// The app's [`Pro`], provided as context for the rest of the window: the
/// licence as saved, checked with api0 at start and then whenever a day has
/// passed since the last check.
pub fn use_pro() -> Pro {
    let pro = use_context_provider(|| Pro {
        open: Signal::new(false),
        status: Signal::new(licence::current()),
    });
    use_future(move || async move {
        let mut status = pro.status;
        status.set(licence::refresh(true).await);
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            status.set(licence::refresh(false).await);
        }
    });
    pro
}

/// The badge in the header's corner, which opens the window. `hint` says what
/// Pro adds, in a few words, for its tooltip while the app isn't Pro.
#[component]
pub fn ProButton(pro: Pro, hint: String) -> Element {
    let name = crate::app().map_or("", |a| a.name);
    let status = pro.status.read().clone();
    let title = match &status {
        Status::Pro(a) => format!("{name} Pro, {} plan for {}", a.plan.name, a.email),
        Status::NotPro(a) if a.is_free() => {
            format!(
                "Free: {} AI credits left this month. {hint}",
                a.credits.left
            )
        }
        Status::NotPro(_) | Status::Offline(_) | Status::Unknown(_) => {
            format!("Your licence doesn't make this {name} Pro right now")
        }
        Status::Free => hint,
    };
    let mut open = pro.open;
    rsx! {
        ProBadge { look: ProLook::of(&status), title, onclick: move |_| open.set(true) }
    }
}

/// The Pro window, while `pro.open`.
#[component]
pub fn ProWindow(
    pro: Pro,
    /// What the free version does and what Pro adds, shown until the app is
    /// Pro. It ends by saying where Pro comes from, e.g. "It comes with a
    /// monthly plan on mayorana.ch."
    pitch: Element,
    /// What the licence gives right now, shown whatever the status (JobScout's
    /// searches used this month).
    details: Option<Element>,
    /// More buttons, before Close (ShipScreen's "Make it my free app").
    actions: Option<Element>,
    /// After an Activate that made the app Pro.
    on_pro: Option<EventHandler<()>>,
    /// Whenever the window closes.
    on_close: Option<EventHandler<()>>,
) -> Element {
    let name = crate::app().map_or("", |a| a.name);
    let mut key = use_signal(String::new);
    let mut error = use_signal(|| Option::<String>::None);
    let mut busy = use_signal(|| false);
    let mut free_email = use_signal(String::new);
    let mut free_note = use_signal(|| Option::<String>::None);
    let mut free_hidden = use_signal(|| false);
    // Set by an Activate that made the app Pro, so the window congratulates
    // rather than just reporting it.
    let mut activated = use_signal(|| false);
    let mut open = pro.open;
    let mut status_signal = pro.status;
    let mut close = move || {
        open.set(false);
        activated.set(false);
        error.set(None);
        if let Some(closed) = on_close {
            closed.call(());
        }
    };
    // Escape closes the window wherever the focus is: the Pro view has no
    // field to listen on.
    use_future(move || async move {
        let mut keys = document::eval(ESCAPE_JS);
        while keys.recv::<bool>().await.is_ok() {
            if *open.peek() {
                close();
            }
        }
    });
    if !open() {
        return rsx! {};
    }
    let status = status_signal.read().clone();
    let asks_number =
        matches!(status, Status::Free | Status::Unknown(_)) || status.free_credits().is_some();
    let offers_free = matches!(status, Status::Free | Status::Unknown(_)) && !free_hidden();

    let mut activate = move || {
        let pasted = key.read().clone();
        busy.set(true);
        spawn(async move {
            match licence::activate(&pasted).await {
                Ok(s) => {
                    let made_pro = s.is_pro();
                    activated.set(made_pro);
                    status_signal.set(s);
                    key.set(String::new());
                    error.set(None);
                    if let (true, Some(made)) = (made_pro, on_pro) {
                        made.call(());
                    }
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };
    let mut check_again = move || {
        busy.set(true);
        spawn(async move {
            status_signal.set(licence::refresh(true).await);
            busy.set(false);
        });
    };
    let mut ask_free = move || {
        let email = free_email.read().clone();
        busy.set(true);
        spawn(async move {
            match licence::request_free_credits(&email).await {
                Ok(message) => {
                    free_note.set(Some(format!(
                        "{message} Paste the licence number above when it arrives."
                    )));
                    error.set(None);
                }
                Err(FreeError::Unavailable) => free_hidden.set(true),
                Err(FreeError::Refused(e)) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };
    let mut manage = move |number: String| {
        busy.set(true);
        spawn(async move {
            match licence::portal_url(&number).await {
                Ok(url) => {
                    crate::browser::open(&url);
                    error.set(None);
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    if let Status::Pro(a) = &status {
        let number = a.licence.clone();
        return rsx! {
            style { {STYLE} }
            div { class: "kit-pro-backdrop", onclick: move |_| close(),
                div {
                    class: "kit-pro-window kit-pro-done",
                    role: "dialog",
                    // Clicks inside the window don't close it.
                    onclick: move |e| e.stop_propagation(),
                    button { class: "kit-pro-close", title: "Close", onclick: move |_| close(), "×" }
                    div { class: "kit-pro-mark", "✓" }
                    h2 { class: "kit-pro-done-title",
                        if activated() { "Congratulations, you're Pro!" } else { "You're Pro" }
                    }
                    p { class: "kit-pro-done-sub", "{a.plan.name} plan for {a.email}." }
                    p { class: "kit-pro-hint",
                        "{a.credits.left} AI credits left this month"
                        br {}
                        "{a.renewal()}"
                    }
                    if let Some(details) = details.clone() {
                        div { class: "kit-pro-details", {details} }
                    }
                    button {
                        class: "kit-pro-btn",
                        disabled: busy(),
                        title: "Change or cancel the plan on mayorana.ch",
                        onclick: move |_| manage(number.clone()),
                        "Manage subscription"
                    }
                    if let Some(e) = error.read().as_ref() {
                        p { class: "kit-pro-error", "{e}" }
                    }
                }
            }
        };
    }

    rsx! {
        style { {STYLE} }
        div { class: "kit-pro-backdrop", onclick: move |_| close(),
            div {
                class: "kit-pro-window",
                role: "dialog",
                onclick: move |e| e.stop_propagation(),
                div { class: "kit-pro-head",
                    h2 { "{name} Pro" }
                    button { class: "kit-pro-close", title: "Close", onclick: move |_| close(), "×" }
                }
                div { class: "kit-pro-body",
                    match &status {
                        Status::NotPro(a) if a.is_free() => rsx! {
                            p {
                                "Free — "
                                strong { "{a.credits.left} AI credits" }
                                " left this month, for "
                                strong { "{a.email}" }
                                ". {a.renewal()}."
                            }
                        },
                        Status::NotPro(a) if !a.active => rsx! {
                            p {
                                "The subscription for "
                                strong { "{a.email}" }
                                " isn't active, so {name} runs as the free version. Renew it to have Pro again."
                            }
                        },
                        Status::NotPro(a) => rsx! {
                            p {
                                "The "
                                strong { "{a.plan.name}" }
                                " plan doesn't include {name}, so it runs as the free version. A plan with {name} makes it Pro."
                            }
                        },
                        Status::Offline(a) => rsx! {
                            p {
                                "{name} hasn't been able to check the licence for "
                                strong { "{a.email}" }
                                " for a week, so it runs as the free version until it can. Check the connection, then check again."
                            }
                        },
                        Status::Unknown(_) => rsx! {
                            p { "The licence number on this computer isn't known. Check it against the email, or paste it again." }
                        },
                        Status::Free | Status::Pro(_) => rsx! {},
                    }
                    if asks_number {
                        div { class: "kit-pro-pitch", {pitch} }
                    }
                    if let Some(details) = details {
                        div { class: "kit-pro-details", {details} }
                    }
                    button {
                        class: "kit-pro-btn kit-pro-buy",
                        onclick: move |_| {
                            crate::browser::open(&licence::buy_url());
                        },
                        if asks_number { "Buy {name} Pro" } else { "Renew {name} Pro" }
                    }
                    if asks_number {
                        p { class: "kit-pro-hint", "Already subscribed? Paste the licence number from your email:" }
                        input {
                            class: "kit-pro-input",
                            placeholder: "ABCDE-FGHJK-LMNPQ-RSTUV",
                            value: "{key}",
                            oninput: move |e| key.set(e.value()),
                            onkeydown: move |e| {
                                if e.key() == Key::Enter && !key.read().trim().is_empty() && !busy() {
                                    activate();
                                }
                            },
                        }
                        p { class: "kit-pro-hint",
                            "The number is checked with api0 now and once a day. Pro keeps working for a week without a connection."
                        }
                    }
                    if offers_free {
                        div { class: "kit-pro-free",
                            p { class: "kit-pro-hint", "No plan? Get free AI credits every month, without Pro. The licence number comes by email." }
                            div { class: "kit-pro-row",
                                input {
                                    class: "kit-pro-input",
                                    r#type: "email",
                                    placeholder: "you@example.com",
                                    value: "{free_email}",
                                    oninput: move |e| free_email.set(e.value()),
                                }
                                button {
                                    class: "kit-pro-btn",
                                    disabled: free_email.read().trim().is_empty() || busy(),
                                    onclick: move |_| ask_free(),
                                    "Get free AI credits"
                                }
                            }
                            if let Some(n) = free_note.read().as_ref() {
                                p { class: "kit-pro-hint kit-pro-ok", "{n}" }
                            }
                        }
                    }
                    if let Some(e) = error.read().as_ref() {
                        p { class: "kit-pro-error", "{e}" }
                    }
                    div { class: "kit-pro-actions",
                        if asks_number {
                            button {
                                class: "kit-pro-btn",
                                disabled: key.read().trim().is_empty() || busy(),
                                onclick: move |_| activate(),
                                if busy() { span { class: "kit-pro-spinner", title: "Checking the licence" } } else { "Activate" }
                            }
                        }
                        if let (Some(number), false) = (status.licence().map(str::to_owned), asks_number) {
                            button {
                                class: "kit-pro-btn",
                                disabled: busy(),
                                title: "Change or cancel the plan on mayorana.ch",
                                onclick: move |_| manage(number.clone()),
                                "Manage subscription"
                            }
                            button {
                                class: "kit-pro-btn",
                                disabled: busy(),
                                onclick: move |_| check_again(),
                                "Check again"
                            }
                        }
                        if status.licence().is_some() {
                            button {
                                class: "kit-pro-btn",
                                title: "Removes it from every Mayorana app on this computer, to move it to another one",
                                onclick: move |_| {
                                    licence::deactivate();
                                    status_signal.set(licence::current());
                                },
                                "Remove the licence from this computer"
                            }
                        }
                        {actions}
                        span { class: "kit-pro-gap" }
                        button { class: "kit-pro-btn", onclick: move |_| close(), "Close" }
                    }
                }
            }
        }
    }
}
