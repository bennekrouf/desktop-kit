//! The file picker every app opens instead of Finder or Explorer when it needs a file another
//! app made: the library's files that fit, suggested for what they're wanted for, by name and
//! maker rather than by folder. The disk is one discreet click away — to see where a file
//! lives, or to choose one the library doesn't have yet, which then joins it.

use dioxus::html::HasFileData;
use dioxus::prelude::*;
use std::collections::BTreeSet;
use std::path::PathBuf;

use super::library::use_library;
use crate::launch;
use crate::library::{self, pick, Kind};

/// The picker's stylesheet. [`FilePicker`] adds it itself.
pub const PICKER_STYLE: &str = include_str!("picker.css");

#[cfg(target_os = "macos")]
const FILE_MANAGER: &str = "Finder";
#[cfg(windows)]
const FILE_MANAGER: &str = "Explorer";
#[cfg(not(any(target_os = "macos", windows)))]
const FILE_MANAGER: &str = "the folder";

/// How many of the best fits show above the rest.
const SUGGESTED: usize = 4;

/// A window over the app with the library's files that fit `want`, the best first. A click
/// chooses one (or, with [`pick::Want::multiple`], marks it, and Add chooses the marked ones);
/// `on_pick` gets the paths and the picker should then be closed. Esc or a click beside it
/// calls `on_close`.
#[component]
pub fn FilePicker(
    want: pick::Want,
    on_pick: EventHandler<Vec<PathBuf>>,
    on_close: EventHandler<()>,
) -> Element {
    let mut items = use_library();
    let mut query = use_signal(String::new);
    let mut marked = use_signal(BTreeSet::<PathBuf>::new);
    // The file whose place on the disk is shown at the bottom.
    let mut hovered = use_signal(|| None::<PathBuf>);
    let mut over = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let all = pick::candidates(&items.read(), &want, now);
    let total = all.len();
    let q = query();
    let shown: Vec<pick::Candidate> = all.into_iter().filter(|c| c.matches(&q)).collect();
    // Suggestions only while nothing is searched: a search is its own ranking.
    let split = if q.trim().is_empty() && shown.len() > SUGGESTED + 2 {
        shown
            .iter()
            .take(SUGGESTED)
            .filter(|c| c.reason.is_some())
            .count()
    } else {
        0
    };
    let (suggested, rest) = shown.split_at(split);
    let mut groups: Vec<(&str, Vec<pick::Candidate>)> = Vec::new();
    for c in rest {
        let label = maker(&c.app);
        match groups.iter_mut().find(|(l, _)| *l == label) {
            Some((_, g)) => g.push(c.clone()),
            None => groups.push((label, vec![c.clone()])),
        }
    }
    let first = shown.first().map(|c| c.path.clone());

    let multiple = want.multiple;
    // Files from the disk join the library, so they're suggested here next time.
    let mut take_from_disk = move |paths: Vec<PathBuf>| {
        if paths.is_empty() {
            return;
        }
        if let Err(e) = library::add_files(&paths) {
            error.set(Some(format!("Chosen, but not kept in the library: {e}")));
        }
        items.set(library::load_all());
        on_pick.call(paths);
    };
    let mut choose = move |path: PathBuf| {
        if multiple {
            let mut m = marked.write();
            if !m.remove(&path) {
                m.insert(path);
            }
        } else {
            on_pick.call(vec![path]);
        }
    };

    let row = move |c: pick::Candidate| {
        let on = marked.read().contains(&c.path);
        let path = c.path.clone();
        let hover = c.path.clone();
        let reveal = c.path.clone();
        rsx! {
            div {
                class: if on { "kit-pick-row kit-on" } else { "kit-pick-row" },
                key: "{c.path.display()}",
                role: "button",
                tabindex: "0",
                onclick: move |_| choose(path.clone()),
                onmouseenter: move |_| hovered.set(Some(hover.clone())),
                span { class: "kit-pick-kind kit-pick-{kind_key(c.kind)}", "{kind_icon(c.kind)}" }
                div { class: "kit-pick-main",
                    div { class: "kit-pick-title", "{c.title}" }
                    div { class: "kit-pick-meta",
                        span { "{c.detail}" }
                        span { "{maker(&c.app)}" }
                        span { "{c.day}" }
                        if !c.used_in.is_empty() {
                            span { class: "kit-pick-used", title: "{c.used_in.join(\"\\n\")}",
                                if c.used_in.len() == 1 { "In 1 draft" } else { "In {c.used_in.len()} drafts" }
                            }
                        }
                    }
                }
                if let Some(r) = c.reason {
                    span { class: "kit-pick-reason", "{r}" }
                }
                span { class: "kit-dot kit-status-{c.status.key()}", title: "{c.status.label()}" }
                button {
                    class: "kit-pick-where",
                    title: "Show it in {FILE_MANAGER}",
                    onclick: move |e| {
                        e.stop_propagation();
                        error.set(launch::reveal(&reveal).err());
                    },
                    "⌕"
                }
            }
        }
    };

    rsx! {
        style { {super::LIBRARY_STYLE} }
        style { {PICKER_STYLE} }
        div { class: "kit-pick-backdrop", onclick: move |_| on_close.call(()),
            div {
                class: if over() { "kit-pick-window kit-drop-over" } else { "kit-pick-window" },
                onclick: move |e| e.stop_propagation(),
                // Typing here is for the picker, not the app's shortcuts underneath.
                onkeydown: move |e| {
                    e.stop_propagation();
                    match e.key() {
                        Key::Escape => on_close.call(()),
                        Key::Enter if !multiple => {
                            if let Some(p) = first.clone() {
                                on_pick.call(vec![p]);
                            }
                        }
                        _ => {}
                    }
                },
                ondragover: move |e| {
                    e.prevent_default();
                    over.set(true);
                },
                ondragleave: move |_| over.set(false),
                ondrop: move |e| {
                    e.prevent_default();
                    over.set(false);
                    take_from_disk(e.files().iter().map(|f| f.path()).collect());
                },
                div { class: "kit-pick-head",
                    h2 { "{want.title}" }
                    button { class: "kit-pick-close", title: "Close (Esc)", onclick: move |_| on_close.call(()), "×" }
                }
                input {
                    class: "kit-lib-search",
                    r#type: "search",
                    autofocus: true,
                    placeholder: "Search by name, tag or app…",
                    value: "{query}",
                    oninput: move |e| query.set(e.value()),
                }
                if let Some(e) = error() {
                    p { class: "kit-lib-error", "{e}" }
                }
                div { class: "kit-pick-list", onmouseleave: move |_| hovered.set(None),
                    if total == 0 {
                        div { class: "kit-pick-empty",
                            p { "Nothing in the library fits yet." }
                            p { "What PolishCast, TrackCut and PublishKit make shows here on its own. Drop a file here or choose one from the disk below." }
                        }
                    } else if shown.is_empty() {
                        p { class: "kit-pick-empty", "Nothing matches “{q}”." }
                    }
                    if !suggested.is_empty() {
                        h3 { class: "kit-pick-section", "Suggested" }
                        for c in suggested.iter().cloned() {
                            {row(c)}
                        }
                    }
                    for (label, group) in groups {
                        if split > 0 || shown.len() > SUGGESTED + 2 {
                            h3 { class: "kit-pick-section", key: "{label}", "{label}" }
                        }
                        for c in group {
                            {row(c)}
                        }
                    }
                }
                div { class: "kit-pick-foot",
                    // Where the file under the pointer is kept: there to read, not to manage.
                    span { class: "kit-pick-path",
                        if let Some(p) = hovered() {
                            "{p.display()}"
                        } else {
                            "Files stay where each app keeps them."
                        }
                    }
                    label { class: "kit-pick-disk", title: "Choose a file that isn't in the library yet",
                        "From the disk…"
                        input {
                            r#type: "file",
                            hidden: true,
                            multiple,
                            accept: "{want.accept()}",
                            onchange: move |e| take_from_disk(e.files().iter().map(|f| f.path()).collect()),
                        }
                    }
                    if multiple {
                        button {
                            class: "kit-chip kit-on",
                            disabled: marked.read().is_empty(),
                            onclick: move |_| on_pick.call(marked.peek().iter().cloned().collect()),
                            match marked.read().len() {
                                0 => "Add".to_string(),
                                1 => "Add 1 file".to_string(),
                                n => format!("Add {n} files"),
                            }
                        }
                    }
                }
            }
        }
    }
}

fn maker(app: &str) -> &str {
    if app == "library" {
        "Added by hand"
    } else {
        launch::name(app)
    }
}

fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Video => "video",
        Kind::Audio => "audio",
        Kind::Image => "image",
        Kind::Blog | Kind::Social => "text",
    }
}

fn kind_icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Video => "▶",
        Kind::Audio => "♪",
        Kind::Image => "▣",
        Kind::Blog | Kind::Social => "¶",
    }
}
