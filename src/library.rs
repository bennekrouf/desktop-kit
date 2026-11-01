//! The media library every Mayorana app shares: what PolishCast recorded and polished, what
//! TrackCut cut, what PublishKit queued and published, and anything dropped in by hand, each with
//! where it stands in the pipeline. On this computer only, beside the shared licence:
//! `…/mayorana/library/`, one small JSON file per item, written whole and renamed into place so
//! an app never reads half of one. Every app reads the folder again when it changes
//! ([`stamp`]), so what one app does shows in the others within a second.
//!
//! Items point at files where the apps keep them; nothing is copied. A file moved away shows as
//! missing ([`Item::missing`]).
//!
//! Each app writes its own items from what it has on disk, again whenever that changes. A
//! status set by hand ([`set_status`]) stays until the work moves past it; the apps' own
//! changes only ever move an item forward, so a re-export doesn't take a posted video back to
//! "ready".

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where an item is on its way out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Recording,
    Polishing,
    Ready,
    Queued,
    Posted,
    Published,
}

impl Status {
    pub const ALL: [Status; 6] = [
        Status::Recording,
        Status::Polishing,
        Status::Ready,
        Status::Queued,
        Status::Posted,
        Status::Published,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Status::Recording => "recording",
            Status::Polishing => "polishing",
            Status::Ready => "ready",
            Status::Queued => "queued",
            Status::Posted => "posted",
            Status::Published => "published",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Recording => "Recording",
            Status::Polishing => "Polishing",
            Status::Ready => "Ready",
            Status::Queued => "Queued",
            Status::Posted => "Posted",
            Status::Published => "Published",
        }
    }

    pub fn from_key(key: &str) -> Option<Status> {
        Status::ALL.into_iter().find(|s| s.key() == key)
    }

    /// Out in the world: posted on a network or published on the site.
    pub fn is_out(self) -> bool {
        matches!(self, Status::Posted | Status::Published)
    }
}

/// What an item is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Video,
    Audio,
    Image,
    Blog,
    Social,
}

impl Kind {
    pub const ALL: [Kind; 5] = [
        Kind::Video,
        Kind::Audio,
        Kind::Image,
        Kind::Blog,
        Kind::Social,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Kind::Video => "Video",
            Kind::Audio => "Audio",
            Kind::Image => "Image",
            Kind::Blog => "Blog post",
            Kind::Social => "Social post",
        }
    }

    /// The kind of a file, by its extension.
    pub fn of(path: &Path) -> Option<Kind> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "mp4" | "mov" | "m4v" | "webm" | "mkv" | "gif" => Some(Kind::Video),
            "mp3" | "wav" | "m4a" | "aac" | "flac" | "ogg" | "opus" => Some(Kind::Audio),
            "png" | "jpg" | "jpeg" | "webp" | "heic" => Some(Kind::Image),
            _ => None,
        }
    }
}

/// What a file is to its item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// What was recorded, before any editing.
    Source,
    /// A finished, full-length video.
    Video,
    /// A short cut from it.
    Clip,
    /// A finished audio file, or one of the tracks cut from it.
    Audio,
    Thumbnail,
    /// A picture: a post's image, a quote card.
    Image,
    /// Subtitles (SRT, WebVTT) or chapter marks.
    Captions,
    /// The post's own text, for a blog or social post.
    Text,
    Other,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Source => "Recording",
            Role::Video => "Video",
            Role::Clip => "Clip",
            Role::Audio => "Audio",
            Role::Thumbnail => "Thumbnail",
            Role::Image => "Picture",
            Role::Captions => "Captions",
            Role::Text => "Text",
            Role::Other => "File",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    pub path: PathBuf,
    pub role: Role,
}

/// Something in another app that this item is part of: the draft a video was attached to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    /// The app that opens it: `publishkit`, `polishcast`, `trackcut`.
    pub app: String,
    /// What the person reads: "TikTok draft: Clip 1".
    pub label: String,
    /// What the app opens it by: a post's file, a take's folder.
    pub target: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// The app's own name for it, unique across apps: `polishcast:<take>`.
    pub id: String,
    /// The app that made it, and opens it: `polishcast`, `trackcut`, `publishkit`, or `library`
    /// for a file dropped in by hand.
    pub app: String,
    pub kind: Kind,
    pub title: String,
    pub status: Status,
    /// The status was set by hand, or by another app (a video queued in PublishKit).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub manual: bool,
    /// The status its own app last gave it, from what it has on disk: the status moves only
    /// when this does, so a status set by hand stays until the work moves on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<Status>,
    /// For a post, its network: `linkedin`, `x`, `instagram`, `tiktok`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(default)]
    pub files: Vec<File>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
    /// Its day on the calendar, local: `2026-11-02`, or with the time, `2026-11-02T09:00`. A
    /// post's planned or actual date; for media, when its status last changed.
    pub date: String,
    /// What its app opens it by: a take's folder, a recording's path, a post's file.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub open: String,
    /// Seconds since 1970, when it was last written.
    #[serde(default)]
    pub updated: u64,
}

impl Item {
    /// A new item, dated now.
    pub fn new(
        id: impl Into<String>,
        app: &str,
        kind: Kind,
        title: impl Into<String>,
        status: Status,
    ) -> Item {
        Item {
            id: id.into(),
            app: app.to_string(),
            kind,
            title: title.into(),
            status,
            manual: false,
            auto: None,
            network: None,
            files: Vec::new(),
            tags: Vec::new(),
            links: Vec::new(),
            date: today(),
            open: String::new(),
            updated: now(),
        }
    }

    pub fn with_file(mut self, path: impl Into<PathBuf>, role: Role) -> Item {
        self.files.push(File {
            path: path.into(),
            role,
        });
        self
    }

    /// Its files no longer where they were.
    pub fn missing(&self) -> Vec<&File> {
        self.files.iter().filter(|f| !f.path.exists()).collect()
    }

    /// The file a click opens: the finished video or audio, else whatever it has first.
    pub fn main_file(&self) -> Option<&File> {
        [
            Role::Video,
            Role::Audio,
            Role::Clip,
            Role::Source,
            Role::Text,
            Role::Image,
            Role::Thumbnail,
        ]
        .iter()
        .find_map(|role| self.files.iter().find(|f| f.role == *role))
        .or(self.files.first())
    }

    /// The calendar day, `2026-11-02`.
    pub fn day(&self) -> &str {
        self.date.get(..10).unwrap_or(&self.date)
    }

    /// The time on that day, `09:00`, when it has one.
    pub fn time(&self) -> Option<&str> {
        self.date.get(11..16)
    }

    /// Whether `query`'s words are all in its title, tags, kind, status, network or file names.
    pub fn matches(&self, query: &str) -> bool {
        let haystack = format!(
            "{} {} {} {} {} {}",
            self.title,
            self.tags.join(" "),
            self.kind.label(),
            self.status.label(),
            self.network.as_deref().unwrap_or(""),
            self.files
                .iter()
                .filter_map(|f| f.path.file_name())
                .map(|n| n.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        )
        .to_lowercase();
        query
            .to_lowercase()
            .split_whitespace()
            .all(|w| haystack.contains(w))
    }
}

/// `…/mayorana/library`, beside the shared licence. `MAYORANA_LIBRARY` overrides it (for
/// development and tests).
pub fn dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("MAYORANA_LIBRARY") {
        return Some(PathBuf::from(d));
    }
    Some(dirs::config_dir()?.join("mayorana").join("library"))
}

/// The file an item is kept in: its id hashed, so any id makes a safe file name.
fn file_of(dir: &Path, id: &str) -> PathBuf {
    // FNV-1a: stable across runs and builds, unlike the standard library's hasher.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    dir.join(format!("{h:016x}.json"))
}

/// Every item, newest first.
pub fn load_all() -> Vec<Item> {
    dir().map(|d| load_in(&d)).unwrap_or_default()
}

fn load_in(dir: &Path) -> Vec<Item> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut items: Vec<Item> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| serde_json::from_slice(&std::fs::read(p).ok()?).ok())
        .collect();
    items.sort_by(|a: &Item, b: &Item| b.date.cmp(&a.date).then(b.updated.cmp(&a.updated)));
    items
}

pub fn get(id: &str) -> Option<Item> {
    let dir = dir()?;
    serde_json::from_slice(&std::fs::read(file_of(&dir, id)).ok()?).ok()
}

/// Something that changes whenever an item is written or removed: an app reads the library
/// again when it differs from the last one it saw.
pub fn stamp() -> u64 {
    let Some(dir) = dir() else { return 0 };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 0;
    };
    let mut stamp: u64 = 0;
    for e in entries.filter_map(Result::ok) {
        let modified = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos() as u64);
        // Order doesn't matter, and a removal changes the sum too.
        stamp = stamp
            .wrapping_add(modified ^ e.file_name().len() as u64)
            .wrapping_add(1);
    }
    stamp
}

fn write(dir: &Path, item: &Item) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = file_of(dir, &item.id);
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(item).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(&tmp, &path)
}

/// Writes an app's own item, as it sees it now. What other apps and the person added is kept:
/// tags, links, a status set by hand that the work hasn't passed, and a status further along
/// (a video another app posted stays posted). Its date moves only when its status does, except
/// for a post, whose date is its own. Returns the item as saved.
pub fn upsert(item: Item) -> std::io::Result<Item> {
    upsert_in(&need_dir()?, item)
}

fn upsert_in(dir: &Path, mut item: Item) -> std::io::Result<Item> {
    let old: Option<Item> = std::fs::read(file_of(dir, &item.id))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    item.auto = Some(item.status);
    if let Some(old) = old {
        for t in &old.tags {
            if !item.tags.contains(t) {
                item.tags.push(t.clone());
            }
        }
        for l in &old.links {
            if !item.links.iter().any(|n| n.target == l.target) {
                item.links.push(l.clone());
            }
        }
        // A post's status and date are its own app's; media keep theirs until the work moves on.
        let is_post = matches!(item.kind, Kind::Blog | Kind::Social);
        let moved = old.auto != Some(item.status) && item.status > old.status;
        if !is_post && !moved {
            item.status = old.status;
            item.manual = old.manual;
            item.date = old.date.clone();
        }
        item.updated = old.updated;
        if item == old {
            return Ok(item);
        }
    }
    item.updated = now();
    write(dir, &item)?;
    Ok(item)
}

/// Writes an item as it is, for an app that changed what another app wrote: the drafts a video
/// went into.
pub fn save(item: &Item) -> std::io::Result<()> {
    let mut item = item.clone();
    item.updated = now();
    write(&need_dir()?, &item)
}

/// Sets an item's status by hand, dated today.
pub fn set_status(id: &str, status: Status) -> std::io::Result<()> {
    set_status_in(&need_dir()?, id, status)
}

fn set_status_in(dir: &Path, id: &str, status: Status) -> std::io::Result<()> {
    change(dir, id, |item| {
        item.status = status;
        item.manual = true;
        item.date = today();
    })
}

pub fn set_tags(id: &str, tags: Vec<String>) -> std::io::Result<()> {
    change(&need_dir()?, id, |item| {
        item.tags = tags
            .iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
    })
}

/// Marks the item holding `file` as used in `link` (a draft): linked, and queued — or posted,
/// when the draft already went out — unless it's further along. `false` when no item holds it.
pub fn used(file: &Path, link: Link, status: Status) -> std::io::Result<bool> {
    let Some(dir) = dir() else { return Ok(false) };
    used_in(&dir, file, link, status)
}

fn used_in(dir: &Path, file: &Path, link: Link, status: Status) -> std::io::Result<bool> {
    let Some(mut item) = load_in(dir)
        .into_iter()
        .find(|i| i.files.iter().any(|f| same_file(&f.path, file)))
    else {
        return Ok(false);
    };
    let mut changed = false;
    match item.links.iter_mut().find(|l| l.target == link.target) {
        Some(l) if *l != link => {
            *l = link;
            changed = true;
        }
        Some(_) => {}
        None => {
            item.links.push(link);
            changed = true;
        }
    }
    if status > item.status {
        item.status = status;
        item.manual = false;
        item.date = today();
        changed = true;
    }
    if changed {
        item.updated = now();
        write(dir, &item)?;
    }
    Ok(true)
}

/// Takes `target` off every item's links, when the draft is gone.
pub fn unlink(target: &str) -> std::io::Result<()> {
    let Some(dir) = dir() else { return Ok(()) };
    for mut item in load_in(&dir) {
        let before = item.links.len();
        item.links.retain(|l| l.target != target);
        if item.links.len() != before {
            item.updated = now();
            write(&dir, &item)?;
        }
    }
    Ok(())
}

pub fn remove(id: &str) -> std::io::Result<()> {
    let Some(dir) = dir() else { return Ok(()) };
    match std::fs::remove_file(file_of(&dir, id)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// An app's items whose ids aren't in `keep`, removed: what it no longer has (a deleted take,
/// a post gone from the site). `prefix` limits it to one source, such as one site's posts.
pub fn retain(
    app: &str,
    prefix: &str,
    keep: &std::collections::HashSet<String>,
) -> std::io::Result<()> {
    let Some(dir) = dir() else { return Ok(()) };
    for item in load_in(&dir) {
        if item.app == app && item.id.starts_with(prefix) && !keep.contains(&item.id) {
            remove(&item.id)?;
        }
    }
    Ok(())
}

/// Files dropped on the library by hand, added as ready items of their own. Files already in
/// it, or that aren't video, audio or pictures, are left out. Returns the items added.
pub fn add_files(paths: &[PathBuf]) -> std::io::Result<Vec<Item>> {
    add_files_in(&need_dir()?, paths)
}

fn add_files_in(dir: &Path, paths: &[PathBuf]) -> std::io::Result<Vec<Item>> {
    let known = load_in(dir);
    let mut added = Vec::new();
    for path in paths {
        let Some(kind) = Kind::of(path) else { continue };
        if known
            .iter()
            .any(|i| i.files.iter().any(|f| same_file(&f.path, path)))
        {
            continue;
        }
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let role = match kind {
            Kind::Audio => Role::Audio,
            Kind::Image => Role::Image,
            _ => Role::Video,
        };
        let item = Item::new(
            format!("library:{}", path.display()),
            "library",
            kind,
            title,
            Status::Ready,
        )
        .with_file(path.clone(), role);
        added.push(upsert_in(dir, item)?);
    }
    Ok(added)
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

fn need_dir() -> std::io::Result<PathBuf> {
    dir().ok_or_else(|| std::io::Error::other("no settings folder on this computer"))
}

fn change(dir: &Path, id: &str, f: impl FnOnce(&mut Item)) -> std::io::Result<()> {
    let path = file_of(dir, id);
    let mut item: Item =
        serde_json::from_slice(&std::fs::read(&path)?).map_err(std::io::Error::other)?;
    f(&mut item);
    item.updated = now();
    write(dir, &item)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The local day of `at`: `2026-11-02`.
pub fn day_of(at: std::time::SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(at)
        .format("%Y-%m-%d")
        .to_string()
}

/// Today, local: `2026-11-02`.
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests;
