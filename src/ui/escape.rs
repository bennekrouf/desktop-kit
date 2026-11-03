//! Esc closes what is open. Listened for on the window rather than on the
//! popup, so it works whatever has focus: after a click on plain text in the
//! popup, or with nothing focused at all.

use std::sync::atomic::{AtomicUsize, Ordering};

use dioxus::prelude::*;

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// Calls `on_escape` each time Esc is pressed while the component that calls
/// this is on screen. The listener goes when the component does, so a popup
/// opened and closed many times never leaves one behind.
pub fn use_escape(on_escape: EventHandler<()>) {
    let key = use_hook(|| format!("_kitEscape{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    let listen = key.clone();
    use_future(move || {
        let key = listen.clone();
        async move {
            let mut keys = document::eval(&format!(
                "if (window.{key}) window.removeEventListener('keydown', window.{key});\
                 window.{key} = (e) => {{ if (e.key === 'Escape') dioxus.send(true); }};\
                 window.addEventListener('keydown', window.{key});"
            ));
            while keys.recv::<bool>().await.is_ok() {
                on_escape.call(());
            }
        }
    });
    use_drop(move || {
        document::eval(&format!(
            "if (window.{key}) {{ window.removeEventListener('keydown', window.{key}); delete window.{key}; }}"
        ));
    });
}
