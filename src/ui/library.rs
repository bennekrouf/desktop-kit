//! The media library and the pipeline calendar, the same in every app that shows them: what
//! PolishCast, TrackCut and PublishKit have made, where each piece stands, and the way to it.
//! Both read the shared [`crate::library`] and follow it as it changes, whichever app changes it.

use dioxus::html::HasFileData;
use dioxus::prelude::*;
use std::path::PathBuf;

use crate::library::{self, Item, Kind, Status};
use crate::{handoff, launch};

/// The library's stylesheet. [`LibraryView`] and [`PipelineCalendar`] add it themselves.
pub const LIBRARY_STYLE: &str = include_str!("library.css");

/// Every item in the library, read again within a second of any app changing it.
pub fn use_library() -> Signal<Vec<Item>> {
    let mut items = use_signal(library::load_all);
    use_future(move || async move {
        let mut seen = library::stamp();
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            let now = library::stamp();
            if now != seen {
                seen = now;
                items.set(library::load_all());
            }
        }
    });
    items
}

/// Opens `target` in `app`: in this app (`here`) through `on_open`; in another by leaving it a
/// note (`<app>-open.json`, `{"open": target}`) and opening it.
pub fn open_in(
    here: &str,
    app: &str,
    target: &str,
    on_open: &EventHandler<String>,
) -> Result<(), String> {
    if app == here {
        on_open.call(target.to_string());
        return Ok(());
    }
    handoff::send(app, "open", here, serde_json::json!({ "open": target }))?;
    launch::open_app(app)
}

/// What a click on an item opens: its place in the app that made it, or, for a file dropped in
/// by hand, the file itself.
fn open_item(here: &str, item: &Item, on_open: &EventHandler<String>) -> Result<(), String> {
    if item.status == Status::Planned && item.files.is_empty() {
        // Nothing made yet: its notes are on the row.
        return Ok(());
    }
    if item.app == "library" || item.open.is_empty() {
        return match item.main_file() {
            Some(f) => launch::open_file(&f.path),
            None => Err("It has no file to open.".into()),
        };
    }
    open_in(here, &item.app, &item.open, on_open)
}

/// Which kinds the library lists.
#[derive(Clone, Copy, PartialEq)]
enum Shown {
    /// Videos, audio and pictures: what the apps made.
    Media,
    Everything,
    Only(Kind),
}

impl Shown {
    fn all() -> Vec<Shown> {
        [Shown::Media, Shown::Everything]
            .into_iter()
            .chain(Kind::ALL.map(Shown::Only))
            .collect()
    }

    fn key(self) -> String {
        match self {
            Shown::Media => "media".into(),
            Shown::Everything => "all".into(),
            Shown::Only(k) => format!("{k:?}"),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Shown::Media => "Recordings, pictures, ideas",
            Shown::Everything => "Everything, posts too",
            Shown::Only(k) => k.label(),
        }
    }

    fn from_key(key: &str) -> Shown {
        Shown::all()
            .into_iter()
            .find(|s| s.key() == key)
            .unwrap_or(Shown::Media)
    }

    /// Planned ideas show with the media, whatever they will be.
    fn shows(self, item: &Item) -> bool {
        match self {
            Shown::Media => {
                matches!(item.kind, Kind::Video | Kind::Audio | Kind::Image)
                    || item.status == Status::Planned
            }
            Shown::Everything => true,
            Shown::Only(k) => k == item.kind,
        }
    }
}

fn is_post(item: &Item) -> bool {
    matches!(item.kind, Kind::Blog | Kind::Social)
}

fn kind_label(item: &Item) -> String {
    match (&item.kind, &item.network) {
        (Kind::Social, Some(n)) => network_name(n).to_string(),
        (kind, _) => kind.label().to_string(),
    }
}

fn network_name(key: &str) -> &str {
    match key {
        "linkedin" => "LinkedIn",
        "x" => "X",
        "instagram" => "Instagram",
        "tiktok" => "TikTok",
        other => other,
    }
}

fn owner(item: &Item) -> &str {
    if item.app == "library" {
        "Added by hand"
    } else {
        launch::name(&item.app)
    }
}

/// The library: everything the apps made and anything dropped on it, searchable, with each
/// item's status, tags and files, and the drafts it went into. `here` is this app's key
/// (`polishcast`); `on_open` opens one of this app's own items or drafts by its target.
#[component]
pub fn LibraryView(
    here: &'static str,
    items: Signal<Vec<Item>>,
    on_open: EventHandler<String>,
) -> Element {
    let mut query = use_signal(String::new);
    // Recordings and pictures at first; posts are a pick away.
    let mut kind = use_signal(|| Shown::Media);
    let mut status = use_signal(|| None::<Status>);
    let mut error = use_signal(|| None::<String>);
    let mut over = use_signal(|| false);
    // The "plan an idea" form, while it is open: title, day, kind, notes.
    let mut planning = use_signal(|| None::<(String, String, Kind, String)>);

    let mut report = move |r: Result<(), String>| error.set(r.err());
    let shown: Vec<Item> = items
        .read()
        .iter()
        .filter(|i| kind().shows(i))
        .filter(|i| status().is_none_or(|s| i.status == s))
        .filter(|i| i.matches(&query()))
        .cloned()
        .collect();
    let total = items.read().len();

    rsx! {
        style { {LIBRARY_STYLE} }
        div {
            class: if over() { "kit-library kit-drop-over" } else { "kit-library" },
            ondragover: move |e| {
                e.prevent_default();
                over.set(true);
            },
            ondragleave: move |_| over.set(false),
            ondrop: move |e| {
                e.prevent_default();
                over.set(false);
                let paths: Vec<PathBuf> = e.files().iter().map(|f| f.path()).collect();
                match library::add_files(&paths) {
                    Ok(added) if added.is_empty() && !paths.is_empty() => error.set(Some(
                        "Nothing added: drop videos, audio or pictures that aren't in the library yet.".into(),
                    )),
                    Ok(_) => {
                        error.set(None);
                        items.set(library::load_all());
                    }
                    Err(e) => error.set(Some(format!("Couldn't add them: {e}"))),
                }
            },
            div { class: "kit-lib-bar",
                input {
                    class: "kit-lib-search",
                    r#type: "search",
                    placeholder: "Search titles, tags, files…",
                    value: "{query}",
                    oninput: move |e| query.set(e.value()),
                }
                select {
                    class: "kit-lib-select",
                    onchange: move |e| kind.set(Shown::from_key(&e.value())),
                    for s in Shown::all() {
                        option { value: "{s.key()}", selected: kind() == s, "{s.label()}" }
                    }
                }
                div { class: "kit-lib-chips",
                    button {
                        class: if status().is_none() { "kit-chip kit-on" } else { "kit-chip" },
                        onclick: move |_| status.set(None),
                        "All"
                    }
                    for s in Status::ALL {
                        button {
                            class: if status() == Some(s) { "kit-chip kit-on" } else { "kit-chip" },
                            onclick: move |_| status.set(if status() == Some(s) { None } else { Some(s) }),
                            span { class: "kit-dot kit-status-{s.key()}" }
                            "{s.label()}"
                        }
                    }
                }
                span { class: "kit-lib-count", "{shown.len()} of {total}" }
                button {
                    class: "kit-chip",
                    title: "Put a video, audio or post you mean to make on the calendar",
                    onclick: move |_| {
                        let next = if planning.peek().is_some() { None } else { Some((String::new(), library::today(), Kind::Video, String::new())) };
                        planning.set(next);
                    },
                    "Plan…"
                }
            }
            if let Some((title, day, plan_kind, notes)) = planning() {
                div { class: "kit-lib-plan",
                    input {
                        class: "kit-lib-search",
                        r#type: "text",
                        placeholder: "What you'll make",
                        value: "{title}",
                        oninput: move |e| {
                            if let Some(p) = planning.write().as_mut() {
                                p.0 = e.value();
                            }
                        },
                    }
                    input {
                        class: "kit-lib-select",
                        r#type: "date",
                        value: "{day}",
                        oninput: move |e| {
                            if let Some(p) = planning.write().as_mut() {
                                p.1 = e.value();
                            }
                        },
                    }
                    select {
                        class: "kit-lib-select",
                        onchange: move |e| {
                            if let Some(p) = planning.write().as_mut() {
                                p.2 = Kind::ALL.into_iter().find(|k| format!("{k:?}") == e.value()).unwrap_or(Kind::Video);
                            }
                        },
                        for k in [Kind::Video, Kind::Audio, Kind::Blog, Kind::Social] {
                            option { value: "{k:?}", selected: plan_kind == k, "{k.label()}" }
                        }
                    }
                    textarea {
                        class: "kit-lib-notes-input",
                        placeholder: "Outline, script, what to show",
                        value: "{notes}",
                        oninput: move |e| {
                            if let Some(p) = planning.write().as_mut() {
                                p.3 = e.value();
                            }
                        },
                    }
                    button {
                        class: "kit-chip kit-on",
                        disabled: title.trim().is_empty() || day.is_empty(),
                        onclick: move |_| {
                            let Some((title, day, kind, notes)) = planning.peek().clone() else { return };
                            match library::plan(kind, &title, &day, &notes, Vec::new()) {
                                Ok(_) => {
                                    planning.set(None);
                                    error.set(None);
                                    items.set(library::load_all());
                                }
                                Err(e) => error.set(Some(format!("Couldn't plan it: {e}"))),
                            }
                        },
                        "Put it on the calendar"
                    }
                }
            }
            if let Some(e) = error() {
                p { class: "kit-lib-error", "{e}" }
            }
            if total == 0 {
                div { class: "kit-lib-empty",
                    p { "Nothing here yet." }
                    p { "Takes from PolishCast, recordings from TrackCut and posts from PublishKit show up here on their own. Drop videos, audio or pictures here to add them." }
                }
            } else if shown.is_empty() {
                p { class: "kit-lib-empty", "Nothing matches." }
            }
            div { class: "kit-lib-list",
                for item in shown {
                    {
                        let missing = item.missing().len();
                        let id = item.id.clone();
                        let id_tags = item.id.clone();
                        let id_remove = item.id.clone();
                        let main = item.main_file().map(|f| f.path.clone());
                        let reveal = main.clone();
                        let open = item.clone();
                        let post = is_post(&item);
                        rsx! {
                            div { class: "kit-lib-row", key: "{item.id}",
                                div { class: "kit-lib-main",
                                    div { class: "kit-lib-title",
                                        span { class: "kit-lib-kind", "{kind_label(&item)}" }
                                        button {
                                            class: "kit-lib-name",
                                            title: "Open in {owner(&item)}",
                                            onclick: move |_| report(open_item(here, &open, &on_open)),
                                            "{item.title}"
                                        }
                                    }
                                    div { class: "kit-lib-meta",
                                        span { "{owner(&item)}" }
                                        span { "{item.day()}" }
                                        for f in item.files.iter().take(4) {
                                            span {
                                                class: if f.path.exists() { "kit-lib-file" } else { "kit-lib-file kit-missing" },
                                                title: "{f.path.display()}",
                                                "{f.role.label()}: {f.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()}"
                                            }
                                        }
                                        if item.files.len() > 4 {
                                            span { "+{item.files.len() - 4} files" }
                                        }
                                        if missing > 0 {
                                            span { class: "kit-missing", "{missing} moved or deleted" }
                                        }
                                    }
                                    if !item.notes.is_empty() {
                                        p { class: "kit-lib-notes", "{item.notes}" }
                                    }
                                    if !item.links.is_empty() {
                                        div { class: "kit-lib-links",
                                            for link in item.links.clone() {
                                                button {
                                                    class: "kit-link",
                                                    title: "Open in {launch::name(&link.app)}",
                                                    onclick: move |_| report(open_in(here, &link.app, &link.target, &on_open)),
                                                    "→ {link.label}"
                                                }
                                            }
                                        }
                                    }
                                }
                                div { class: "kit-lib-side",
                                    select {
                                        class: "kit-lib-select kit-status-select kit-status-{item.status.key()}",
                                        disabled: post,
                                        title: if post { "Set by PublishKit: publish or schedule it there" } else { "Where it stands" },
                                        onchange: move |e| {
                                            if let Some(s) = Status::from_key(&e.value()) {
                                                report(library::set_status(&id, s).map_err(|e| e.to_string()));
                                                items.set(library::load_all());
                                            }
                                        },
                                        for s in Status::ALL {
                                            option { value: "{s.key()}", selected: item.status == s, "{s.label()}" }
                                        }
                                    }
                                    input {
                                        class: "kit-lib-tags",
                                        r#type: "text",
                                        placeholder: "Tags",
                                        value: "{item.tags.join(\", \")}",
                                        onchange: move |e| {
                                            let tags = e.value().split(',').map(str::to_string).collect();
                                            report(library::set_tags(&id_tags, tags).map_err(|e| e.to_string()));
                                            items.set(library::load_all());
                                        },
                                    }
                                    div { class: "kit-lib-actions",
                                        if let Some(path) = main {
                                            button {
                                                class: "kit-chip",
                                                title: "Open the file",
                                                onclick: move |_| report(launch::open_file(&path)),
                                                "Play"
                                            }
                                        }
                                        if let Some(url) = item.url.clone() {
                                            button {
                                                class: "kit-chip",
                                                title: "{url}",
                                                onclick: move |_| {
                                                    crate::browser::open(&url);
                                                },
                                                "Online"
                                            }
                                        }
                                        if let Some(path) = reveal {
                                            button {
                                                class: "kit-chip",
                                                title: "Show it in its folder",
                                                onclick: move |_| report(launch::reveal(&path)),
                                                "Show"
                                            }
                                        }
                                        if item.app == "library" || item.status == Status::Planned {
                                            button {
                                                class: "kit-chip",
                                                title: "Take it out of the library; the file stays where it is",
                                                onclick: move |_| {
                                                    report(library::remove(&id_remove).map_err(|e| e.to_string()));
                                                    items.set(library::load_all());
                                                },
                                                "Remove"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            p { class: "kit-lib-hint", "Drop videos, audio or pictures anywhere here to add them. Files stay where they are." }
        }
    }
}

/// Whether an item can be moved on the calendar here. A post's date is in its own file, which
/// PublishKit changes.
fn movable(item: &Item) -> bool {
    !is_post(item)
}

/// `day` with `item`'s time of day kept, as [`library::set_date`] takes it.
fn moved_date(item: &Item, day: chrono::NaiveDate) -> String {
    let day = day.format("%Y-%m-%d");
    match item.time() {
        Some(t) => format!("{day}T{t}"),
        None => day.to_string(),
    }
}

/// A month of the pipeline: every item on its day, coloured by where it stands. A click opens
/// it, in this app or the one that made it. An item dragged to another day moves there; ✎ (or a
/// right-click) types its date instead.
#[component]
pub fn PipelineCalendar(
    here: &'static str,
    items: Signal<Vec<Item>>,
    on_open: EventHandler<String>,
) -> Element {
    use chrono::{Datelike, Duration, Local, NaiveDate};
    use dioxus::html::input_data::MouseButton;
    let today = Local::now().date_naive();
    let mut month = use_signal(|| today.with_day(1).unwrap_or(today));
    let mut error = use_signal(|| None::<String>);
    // The item held down, while it is dragged, and the day it is over.
    let mut held = use_signal(|| None::<Item>);
    let mut over = use_signal(|| None::<NaiveDate>);
    // Where the pointer is while an item is dragged: its ghost follows it. Only [`DragGhost`]
    // reads it, so the month isn't drawn again at every move.
    let mut pointer = use_signal(|| None::<(f64, f64)>);
    // The item whose date is being typed: it, the day and the time.
    let mut editing = use_signal(|| None::<(Item, String, String)>);

    let mut move_to = move |id: &str, date: &str| {
        let moved = library::set_date(id, date).map_err(|e| format!("Couldn't move it: {e}"));
        error.set(moved.err());
        items.set(library::load_all());
    };

    let first = month();
    let next = first
        .checked_add_months(chrono::Months::new(1))
        .unwrap_or(first);
    let start = first - Duration::days(i64::from(first.weekday().num_days_from_monday()));
    let weeks = ((next - start).num_days() + 6) / 7;
    let days: Vec<NaiveDate> = (0..weeks * 7).map(|d| start + Duration::days(d)).collect();
    let all = items.read().clone();
    let on = |day: NaiveDate| -> Vec<Item> {
        let key = day.format("%Y-%m-%d").to_string();
        let mut on: Vec<Item> = all.iter().filter(|i| i.day() == key).cloned().collect();
        on.sort_by(|a, b| a.date.cmp(&b.date));
        on
    };
    let dragging = held.read().is_some();
    let ghost = held
        .read()
        .as_ref()
        .map(|i| (i.status.key(), i.title.clone()));

    rsx! {
        style { {LIBRARY_STYLE} }
        div { class: "kit-cal",
            div { class: "kit-lib-bar",
                button {
                    class: "kit-chip",
                    onclick: move |_| month.set(first.checked_sub_months(chrono::Months::new(1)).unwrap_or(first)),
                    "‹"
                }
                h3 { class: "kit-cal-month", "{first.format(\"%B %Y\")}" }
                button { class: "kit-chip", onclick: move |_| month.set(next), "›" }
                button {
                    class: "kit-chip",
                    onclick: move |_| month.set(today.with_day(1).unwrap_or(today)),
                    "Today"
                }
                div { class: "kit-lib-chips kit-cal-legend",
                    for s in Status::ALL {
                        span { class: "kit-legend", span { class: "kit-dot kit-status-{s.key()}" } "{s.label()}" }
                    }
                }
            }
            if let Some((item, day, time)) = editing() {
                div { class: "kit-lib-plan kit-cal-edit",
                    span { class: "kit-cal-edit-title", "Move “{item.title}” to" }
                    input {
                        class: "kit-lib-select",
                        r#type: "date",
                        value: "{day}",
                        oninput: move |e| {
                            if let Some(p) = editing.write().as_mut() {
                                p.1 = e.value();
                            }
                        },
                    }
                    input {
                        class: "kit-lib-select",
                        r#type: "time",
                        title: "Leave empty for the whole day",
                        value: "{time}",
                        oninput: move |e| {
                            if let Some(p) = editing.write().as_mut() {
                                p.2 = e.value();
                            }
                        },
                    }
                    button {
                        class: "kit-chip kit-on",
                        disabled: day.is_empty(),
                        onclick: move |_| {
                            let Some((item, day, time)) = editing.peek().clone() else { return };
                            let date = if time.is_empty() { day.clone() } else { format!("{day}T{}", time.get(..5).unwrap_or(&time)) };
                            move_to(&item.id, &date);
                            if error.peek().is_none() {
                                editing.set(None);
                                if let Ok(d) = NaiveDate::parse_from_str(&day, "%Y-%m-%d") {
                                    month.set(d.with_day(1).unwrap_or(d));
                                }
                            }
                        },
                        "Move"
                    }
                    button { class: "kit-chip", onclick: move |_| editing.set(None), "Cancel" }
                }
            }
            if let Some(e) = error() {
                p { class: "kit-lib-error", "{e}" }
            }
            div {
                class: if dragging { "kit-cal-grid kit-cal-dragging" } else { "kit-cal-grid" },
                onmouseleave: move |_| {
                    held.set(None);
                    over.set(None);
                    pointer.set(None);
                },
                onmousemove: move |e| {
                    if held.peek().is_some() {
                        let at = e.client_coordinates();
                        pointer.set(Some((at.x, at.y)));
                    }
                },
                onmouseup: move |_| pointer.set(None),
                if let Some((status, title)) = ghost {
                    DragGhost { pointer, class: format!("kit-cal-item kit-cal-ghost kit-status-{status}"), title }
                }
                for name in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
                    div { class: "kit-cal-head", "{name}" }
                }
                for day in days {
                    div {
                        class: format!(
                            "kit-cal-day{}{}{}",
                            if day.month() != first.month() { " kit-cal-out" } else { "" },
                            if day == today { " kit-cal-today" } else { "" },
                            if over() == Some(day) { " kit-cal-drop" } else { "" },
                        ),
                        onmouseenter: move |_| {
                            if held.peek().is_some() {
                                over.set(Some(day));
                            }
                        },
                        onmouseup: move |_| {
                            over.set(None);
                            let Some(item) = held.take() else { return };
                            if item.day() != day.format("%Y-%m-%d").to_string() {
                                move_to(&item.id, &moved_date(&item, day));
                            }
                        },
                        span { class: "kit-cal-num", "{day.day()}" }
                        for item in on(day) {
                            {
                                let open = item.clone();
                                let grab = item.clone();
                                let edit = item.clone();
                                let right = item.clone();
                                let can_move = movable(&item);
                                let is_held = held.read().as_ref().is_some_and(|h| h.id == item.id);
                                let tip = if can_move {
                                    format!("{} · {} · {} — drag it to another day to move it", kind_label(&item), owner(&item), item.status.label())
                                } else {
                                    format!("{} · {} · {} — move it in PublishKit", kind_label(&item), owner(&item), item.status.label())
                                };
                                let mut start_edit = move |item: &Item| {
                                    error.set(None);
                                    editing.set(Some((item.clone(), item.day().to_string(), item.time().unwrap_or_default().to_string())));
                                };
                                rsx! {
                                    div {
                                        class: format!(
                                            "kit-cal-item kit-status-{}{}{}",
                                            item.status.key(),
                                            if can_move { " kit-cal-movable" } else { "" },
                                            if is_held { " kit-cal-held" } else { "" },
                                        ),
                                        key: "{item.id}",
                                        role: "button",
                                        title: "{tip}",
                                        onmousedown: move |e| {
                                            if can_move && e.trigger_button() == Some(MouseButton::Primary) {
                                                held.set(Some(grab.clone()));
                                            }
                                        },
                                        onclick: move |_| error.set(open_item(here, &open, &on_open).err()),
                                        oncontextmenu: move |e| {
                                            if can_move {
                                                e.prevent_default();
                                                start_edit(&right);
                                            }
                                        },
                                        if let Some(t) = item.time() {
                                            span { class: "kit-cal-time", "{t}" }
                                        }
                                        span { class: "kit-cal-label", "{item.title}" }
                                        if can_move {
                                            span {
                                                class: "kit-cal-pen",
                                                title: "Change its date",
                                                onmousedown: move |e| e.stop_propagation(),
                                                onclick: move |e| {
                                                    e.stop_propagation();
                                                    start_edit(&edit);
                                                },
                                                "✎"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The card being dragged, under the pointer: drawn on its own, so only it follows each move.
/// It lets the pointer through, to the day it would land on. Nothing shows until the pointer
/// moves, so a click doesn't flash it.
#[component]
pub fn DragGhost(pointer: Signal<Option<(f64, f64)>>, class: String, title: String) -> Element {
    let Some((x, y)) = pointer() else {
        return rsx! {};
    };
    rsx! {
        div {
            class: "{class}",
            style: "position: fixed; left: {x + 12.0}px; top: {y + 8.0}px; pointer-events: none;",
            "{title}"
        }
    }
}
