//! The "Use it from Claude or Codex" window: adds the app's MCP server to the clients on this
//! computer and takes it out again, with what each one has, whether it runs it now, and a test.
//! The steps by hand stay below, for any other client. What Claude can do there, and what to
//! ask it first, is the app's own.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use dioxus::prelude::*;

use crate::mcp::clients::{self, Client, Running, State};
use crate::mcp::Connect;

/// Only what this window adds: it is drawn as a Pro window, so it brings that
/// window's stylesheet too, whether or not the Pro window is open.
const STYLE: &str = include_str!("connect.css");

/// How often what runs is looked at again while the window is open. Asking Windows
/// starts PowerShell: less often there.
const LOOK: Duration = Duration::from_secs(if cfg!(windows) { 15 } else { 4 });

/// What is being done to a client.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Doing {
    Install,
    Uninstall,
    /// Claude Desktop has to be quit first: done as soon as it is.
    /// `since` tells one wait from the next, when one is cancelled and asked again.
    Waiting {
        install: bool,
        since: Instant,
    },
}

/// What the window knows of the clients, and does to them.
#[derive(Clone, Copy, PartialEq)]
struct View {
    states: Signal<Option<HashMap<Client, State>>>,
    running: Signal<Vec<Running>>,
    doing: Signal<HashMap<Client, Doing>>,
    failed: Signal<HashMap<Client, String>>,
    /// Something was changed: the sessions that are open do not know yet.
    changed: Signal<bool>,
    id: &'static str,
    command: Signal<String>,
}

impl View {
    async fn look(mut self) {
        let command = self.command.peek().clone();
        let states = clients::states(self.id, &command).await;
        self.states.set(Some(states.into_iter().collect()));
        self.look_running().await;
    }

    /// Whether Claude Desktop is open, what runs being kept.
    async fn look_running(mut self) -> bool {
        let command = self.command.peek().clone();
        let (running, open) = clients::running(&command).await;
        if *self.running.peek() != running {
            self.running.set(running);
        }
        open
    }

    fn state(&self, client: Client) -> Option<State> {
        self.states.read().as_ref()?.get(&client).cloned()
    }

    /// The clients installing, or uninstalling, would change.
    fn todo(&self, install: bool) -> Vec<Client> {
        let wanted = |c: &Client| {
            self.state(*c)
                .is_some_and(|s| if install { s.installable() } else { s.added() })
        };
        Client::ALL.into_iter().filter(wanted).collect()
    }

    fn start(self, client: Client, install: bool) {
        spawn(async move { self.run(client, install).await });
    }

    /// One after the other, Claude Desktop last: it may have to be waited for.
    fn start_all(self, install: bool) {
        spawn(async move {
            let mut all = self.todo(install);
            all.sort_by_key(|c| *c == Client::ClaudeDesktop);
            for client in all {
                self.run(client, install).await;
            }
        });
    }

    async fn run(mut self, client: Client, install: bool) {
        if self.doing.peek().contains_key(&client) {
            return;
        }
        self.failed.write().remove(&client);
        let now = if install {
            Doing::Install
        } else {
            Doing::Uninstall
        };
        self.doing.write().insert(client, now);
        if client == Client::ClaudeDesktop && self.look_running().await {
            let waiting = Doing::Waiting {
                install,
                since: Instant::now(),
            };
            self.doing.write().insert(client, waiting);
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                // Cancelled.
                if self.doing.peek().get(&client) != Some(&waiting) {
                    return;
                }
                if !self.look_running().await {
                    break;
                }
            }
            // It writes its file as it closes.
            tokio::time::sleep(Duration::from_secs(1)).await;
            if self.doing.peek().get(&client) != Some(&waiting) {
                return;
            }
            self.doing.write().insert(client, now);
        }
        let command = self.command.peek().clone();
        let done = if install {
            clients::install(client, self.id, &command).await
        } else {
            clients::uninstall(client, self.id).await
        };
        match done {
            Ok(()) => self.changed.set(true),
            Err(why) => {
                self.failed.write().insert(client, why);
            }
        }
        self.look().await;
        self.doing.write().remove(&client);
    }

    fn cancel(mut self, client: Client) {
        self.doing.write().remove(&client);
    }
}

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
    let connect = use_hook(|| Connect::new(id));
    let first = connect.command.clone();
    let view = View {
        states: use_signal(|| None),
        running: use_signal(Vec::new),
        doing: use_signal(HashMap::new),
        failed: use_signal(HashMap::new),
        changed: use_signal(|| false),
        id,
        command: use_signal(move || first),
    };
    use_future(move || async move {
        view.look().await;
        loop {
            tokio::time::sleep(LOOK).await;
            view.look_running().await;
        }
    });
    super::use_escape(on_close);

    let unstable = clients::unstable(&connect.command);
    let busy = !view.doing.read().is_empty();
    let (to_install, to_uninstall) = (view.todo(true), view.todo(false));
    let others = view
        .running
        .read()
        .iter()
        .filter(|r| r.client.is_none())
        .count();
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
                    if let Some(why) = unstable.clone() {
                        p { class: "kit-pro-error", "{why}" }
                    }
                    div { class: "kit-connect-clients",
                        for client in Client::ALL {
                            ClientRow { key: "{client:?}", view, client, blocked: unstable.is_some() }
                        }
                    }
                    div { class: "kit-connect-actions",
                        button {
                            class: "kit-pro-btn kit-connect-primary",
                            disabled: busy || unstable.is_some() || to_install.is_empty(),
                            onclick: move |_| view.start_all(true),
                            "Install everywhere"
                        }
                        button {
                            class: "kit-pro-btn",
                            disabled: busy || to_uninstall.is_empty(),
                            onclick: move |_| view.start_all(false),
                            "Uninstall everywhere"
                        }
                        button {
                            class: "kit-pro-btn",
                            disabled: busy,
                            onclick: move |_| { spawn(view.look()); },
                            "Refresh"
                        }
                        TestButton { command: connect.command.clone() }
                    }
                    if others > 0 {
                        p { class: "kit-pro-hint",
                            "Also running in another client"
                            if others > 1 { " ({others} times)" }
                            "."
                        }
                    }
                    if (view.changed)() {
                        p { class: "kit-pro-hint",
                            "Done. What is open keeps the list it started with: begin a new session to see the change."
                        }
                    }
                    p { class: "kit-pro-hint kit-connect-try", strong { "Try it. " } "Ask: {try_it}" }
                    ByHand { id, name, connect: connect.clone() }
                    p { class: "kit-pro-hint kit-connect-note",
                        if let Some(caution) = caution {
                            "{caution} "
                        }
                        "Claude and ChatGPT in the browser (claude.ai, chatgpt.com) can\u{2019}t use this: they only connect \
                         to servers on the internet, and {name} runs on your computer. If you use ChatGPT, \
                         install it in Codex."
                    }
                }
            }
        }
    }
}

#[component]
fn ClientRow(view: View, client: Client, blocked: bool) -> Element {
    let state = view.state(client);
    let doing = view.doing.read().get(&client).copied();
    let failed = view.failed.read().get(&client).cloned();
    let running = view
        .running
        .read()
        .iter()
        .filter(|r| r.client == Some(client))
        .count();
    let (look, text) = match &state {
        None => ("", "Checking\u{2026}"),
        Some(State::Missing) => ("", "Not on this computer"),
        Some(State::Absent) => ("", "Not installed"),
        Some(State::Installed) => ("good", "Installed"),
        Some(State::Elsewhere(_)) => ("warn", "Installed from another place"),
        Some(State::Unreadable(_)) => ("warn", "Could not check"),
    };
    let detail = match &state {
        Some(State::Elsewhere(path)) => {
            Some(format!("It starts {path}. Update points it to this one."))
        }
        Some(State::Unreadable(why)) => Some(why.clone()),
        _ => None,
    };
    let updates = matches!(state, Some(State::Elsewhere(_)));
    let installable = state.as_ref().is_some_and(State::installable);
    let added = state.as_ref().is_some_and(State::added);
    rsx! {
        div { class: "kit-connect-client",
            div { class: "kit-connect-who",
                strong { "{client.name()}" }
                span { class: "kit-connect-about", "{client.about()}" }
            }
            if running > 0 {
                span {
                    class: "kit-connect-running",
                    title: "{client.name()} has started the server: {running} running now",
                    "Running"
                }
            }
            span { class: "kit-connect-state {look}", "{text}" }
            match doing {
                Some(Doing::Install) => rsx! { span { class: "kit-connect-about", "Installing\u{2026}" } },
                Some(Doing::Uninstall) => rsx! { span { class: "kit-connect-about", "Uninstalling\u{2026}" } },
                Some(Doing::Waiting { .. }) => rsx! {
                    button { class: "kit-pro-btn", onclick: move |_| view.cancel(client), "Cancel" }
                },
                None => rsx! {
                    if installable {
                        button {
                            class: "kit-pro-btn",
                            disabled: blocked,
                            onclick: move |_| view.start(client, true),
                            if updates { "Update" } else { "Install" }
                        }
                    }
                    if added {
                        button {
                            class: "kit-pro-btn",
                            onclick: move |_| view.start(client, false),
                            "Uninstall"
                        }
                    }
                },
            }
            if let Some(Doing::Waiting { install, .. }) = doing {
                p { class: "kit-connect-detail",
                    "Quit Claude completely: it would undo the change while it is open. "
                    if install { "It is installed" } else { "It is uninstalled" }
                    " as soon as Claude closes. Keep this window open."
                }
            }
            if let Some(why) = failed {
                p { class: "kit-connect-detail warn", "{why}" }
            } else if let Some(detail) = detail {
                p { class: "kit-connect-detail", "{detail}" }
            }
        }
    }
}

/// Starts the server as a client does, and says what it answered.
#[component]
fn TestButton(command: String) -> Element {
    let mut testing = use_signal(|| false);
    let mut result = use_signal(|| None::<Result<Vec<String>, String>>);
    rsx! {
        button {
            class: "kit-pro-btn",
            disabled: testing(),
            title: "Start the server as a client does, and list its tools",
            onclick: move |_| {
                let command = command.clone();
                async move {
                    testing.set(true);
                    result.set(None);
                    let answer = clients::test(&command).await;
                    result.set(Some(answer));
                    testing.set(false);
                }
            },
            if testing() { "Testing\u{2026}" } else { "Test" }
        }
        match result() {
            Some(Ok(tools)) => rsx! {
                span { class: "kit-connect-state good", title: tools.join(", "), "It works: {tools.len()} tools" }
            },
            Some(Err(why)) => rsx! { p { class: "kit-connect-detail warn", "{why}" } },
            None => rsx! {},
        }
    }
}

/// The steps for a client this window does not set up, or a file it leaves alone.
#[component]
fn ByHand(id: &'static str, name: &'static str, connect: Connect) -> Element {
    let mut open_error = use_signal(|| None::<String>);
    let Connect {
        command,
        desktop,
        code,
        codex,
        config_path,
    } = connect;
    rsx! {
        details { class: "kit-connect-hand",
            summary { "By hand, or in another client" }
            p { class: "kit-pro-hint",
                "Any client that speaks MCP starts {name} with: "
                code { "{command} mcp" }
            }
            h3 { "Claude Desktop" }
            p { class: "kit-pro-hint",
                "With Claude quit, add this to {config_path}. If the file already has an "
                code { "mcpServers" }
                " section, add just the "
                code { "{id}" }
                " entry to it. "
                button {
                    class: "kit-pro-btn",
                    onclick: move |_| open_error.set(crate::mcp::open_desktop_config().err()),
                    "Open config file"
                }
            }
            if let Some(why) = open_error() {
                p { class: "kit-pro-hint", "{why}" }
            }
            Snippet { text: desktop }
            h3 { "Claude Code" }
            Snippet { text: code }
            h3 { "Codex" }
            Snippet { text: codex }
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
