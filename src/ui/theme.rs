//! Light or dark: dark by default, follows the OS setting live, and stops
//! following once the person picks one with the toggle. The light palette is
//! `body.light` in each app's stylesheet; this only decides when that class is
//! on.

use dioxus::prelude::*;

/// Which theme is showing, and whether the person picked it.
#[derive(Clone, Copy, PartialEq)]
pub struct Theme {
    pub light: Signal<bool>,
    /// Set by the toggle: from then on the OS setting is ignored.
    pub picked: Signal<bool>,
}

impl Theme {
    /// What the toggle does: the other theme, kept from now on.
    pub fn toggle(mut self) {
        self.picked.set(true);
        let next = !*self.light.peek();
        self.light.set(next);
    }
}

/// The theme for an app that keeps no choice of its own: follows the OS until
/// the toggle is used, and puts `light` on `body` while it is light.
pub fn use_theme() -> Theme {
    let mut light = use_signal(|| false);
    let picked = use_signal(|| false);

    // The webview reports the OS appearance, and reports it again whenever it
    // changes, so there is nothing to poll.
    use_future(move || async move {
        let mut os = document::eval(
            "const mq = window.matchMedia('(prefers-color-scheme: light)');
             dioxus.send(mq.matches);
             mq.addEventListener('change', (e) => dioxus.send(e.matches));
             await new Promise(() => {});",
        );
        while let Ok(is_light) = os.recv::<bool>().await {
            if !*picked.peek() {
                light.set(is_light);
            }
        }
    });

    use_effect(move || {
        document::eval(&format!(
            "document.body.classList.toggle('light', {});",
            light()
        ));
    });

    Theme { light, picked }
}

/// The sun or moon button in the header.
#[component]
pub fn ThemeToggle(theme: Theme) -> Element {
    let light = (theme.light)();
    let label = if light {
        "Switch to dark mode"
    } else {
        "Switch to light mode"
    };
    rsx! {
        button {
            class: "kit-btn kit-icon",
            title: label,
            aria_label: label,
            onclick: move |_| theme.toggle(),
            svg { view_box: "0 0 24 24", "aria-hidden": "true",
                if light {
                    path { d: "M20.5 13.3A8.5 8.5 0 1110.7 3.5a6.6 6.6 0 009.8 9.8z" }
                } else {
                    path { d: "M12 4V2M12 22v-2M4 12H2M22 12h-2M5.6 5.6L4.2 4.2M19.8 19.8l-1.4-1.4M18.4 5.6l1.4-1.4M4.2 19.8l1.4-1.4" }
                    path { d: "M12 7.5a4.5 4.5 0 100 9 4.5 4.5 0 000-9z" }
                }
            }
        }
    }
}
