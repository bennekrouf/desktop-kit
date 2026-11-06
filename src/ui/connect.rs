//! The "Use it from Claude or Codex" window: how to connect Claude Desktop, Claude Code or
//! Codex to the app's MCP server. The steps are the same in every app; what Claude can do
//! there, and what to ask it first, is the app's own.

use dioxus::prelude::*;

use crate::mcp::Connect;

/// Only what this window adds: it is drawn as a Pro window, so it brings that
/// window's stylesheet too, whether or not the Pro window is open.
const STYLE: &str = include_str!("connect.css");

#[component]
pub fn ConnectWindow(
    /// What Claude can do with the app, first thing in the window.
    lead: String,
    /// What to ask first, quotes included: “What do my repositories need?”
    try_it: String,
    /// A caution before the closing note (where the data goes, which key is used).
    #[props(default)]
    caution: Option<String>,
    /// Settings of the app's own, after the lead (PolishCast's recording switch).
    #[props(default)]
    extra: Option<Element>,
    on_close: EventHandler<()>,
) -> Element {
    let (id, name) = crate::app().map_or(("", ""), |a| (a.id, a.name));
    let Connect {
        command,
        desktop,
        code,
        codex,
        config_path,
    } = Connect::new(id);
    let mut open_error = use_signal(|| None::<String>);
    super::use_escape(on_close);
    rsx! {
        style { {super::pro::STYLE} }
        style { {STYLE} }
        div { class: "kit-pro-backdrop", onclick: move |_| on_close.call(()),
            div {
                class: "kit-pro-window kit-connect-window",
                onclick: move |e| e.stop_propagation(),
                div { class: "kit-pro-head",
                    h2 { "Use {name} from Claude or Codex" }
                    button { class: "kit-pro-close", title: "Close", onclick: move |_| on_close.call(()), "×" }
                }
                div { class: "kit-pro-body",
                    p { "{lead}" }
                    {extra}
                    div { class: "kit-connect-step",
                        h3 { span { class: "kit-connect-num", "1" } "Claude Desktop \u{2013} Chat" }
                        p { class: "kit-pro-hint",
                            "Open Claude\u{2019}s config file, paste this in and save, then quit Claude completely \
                             and open it again. {name} then shows under Connectors. If the file already has an "
                            code { "mcpServers" }
                            " section, add just the "
                            code { "{id}" }
                            " entry to it."
                        }
                        Snippet { text: desktop }
                        p { class: "kit-pro-hint",
                            "The file: {config_path} "
                            button {
                                class: "kit-connect-copy",
                                onclick: move |_| open_error.set(crate::mcp::open_desktop_config().err()),
                                "Open config file"
                            }
                        }
                        if let Some(why) = open_error() {
                            p { class: "kit-pro-hint", "{why}" }
                        }
                    }
                    div { class: "kit-connect-step",
                        h3 { span { class: "kit-connect-num", "2" } "Claude Code \u{2013} terminal and the desktop app\u{2019}s Code tab" }
                        p { class: "kit-pro-hint",
                            "Run this once in a terminal. It connects the claude command and the Code tab, not the \
                             Chat side above. If it says the server already exists, it is already connected there."
                        }
                        Snippet { text: code }
                    }
                    div { class: "kit-connect-step",
                        h3 { span { class: "kit-connect-num", "3" } "Codex" }
                        p { class: "kit-pro-hint", "Run this once in a terminal:" }
                        Snippet { text: codex }
                    }
                    div { class: "kit-connect-step",
                        h3 { span { class: "kit-connect-num", "4" } "Try it" }
                        p { class: "kit-pro-hint", "Ask: {try_it}" }
                    }
                    p { class: "kit-pro-hint kit-connect-note",
                        if let Some(caution) = caution {
                            "{caution} "
                        }
                        "Claude in the browser (claude.ai) can\u{2019}t use this: it only connects to servers on the \
                         internet, and {name} runs on your computer. Any other app that speaks MCP can, with the \
                         same command: "
                        code { "{command} mcp" }
                    }
                }
            }
        }
    }
}

/// A block of text to paste somewhere else, with its own Copy button. Copied by the page, with
/// the old way as a fallback for a web view that refuses the clipboard API.
#[component]
fn Snippet(text: String) -> Element {
    let mut copied = use_signal(|| false);
    let quoted = serde_json::to_string(&text).unwrap_or_default();
    rsx! {
        div { class: "kit-connect-snippet",
            pre { "{text}" }
            button {
                class: "kit-connect-copy",
                onclick: move |_| {
                    document::eval(&format!(
                        "{{const t={quoted};const fallback=()=>{{const a=document.createElement('textarea');\
                         a.value=t;document.body.appendChild(a);a.select();document.execCommand('copy');a.remove();}};\
                         if(navigator.clipboard){{navigator.clipboard.writeText(t).catch(fallback);}}else{{fallback();}}}}"
                    ));
                    copied.set(true);
                },
                if copied() { "Copied ✓" } else { "Copy" }
            }
        }
    }
}
