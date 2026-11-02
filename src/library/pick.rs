//! Choosing a file by what it is rather than where it is. When one app needs a file another
//! made (PublishKit a PolishCast clip, PolishCast a TrackCut song), it asks for it by what it
//! is for ([`Want`]) and gets the library's files ranked for that ([`candidates`]): the same
//! recording's subtitles first, what's finished and not used yet, what was made this week, what
//! shares words with the post. Nobody has to know which folder an app keeps its exports in.
//!
//! The disk is still there for whoever wants it: each candidate keeps its path, and a file
//! chosen from the disk joins the library ([`super::add_files`]) so it is suggested next time.

use super::{Item, Kind, Role, Status};
use std::path::{Path, PathBuf};

/// What a file is wanted for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Want {
    /// What the picker says it is choosing: "Music for this video".
    pub title: String,
    /// The roles accepted, the best first: `[Clip, Video]` for TikTok. Empty: any.
    pub roles: Vec<Role>,
    /// The extensions accepted, lowercase without the dot. Empty: any.
    pub extensions: Vec<String>,
    /// Several files may be chosen at once.
    pub multiple: bool,
    /// Words about what it is for: the post's title and tags, the take's name.
    pub context: Vec<String>,
    /// A file whose item comes first: the recording a transcript is wanted for.
    pub near: Option<PathBuf>,
    /// Files already in place, left out: the post's photos.
    pub exclude: Vec<PathBuf>,
    /// The app asking: files the other apps made rank a little higher, since handing work
    /// between apps is what this is for.
    pub here: String,
}

impl Want {
    pub fn new(here: &str, title: impl Into<String>) -> Want {
        Want {
            here: here.to_string(),
            title: title.into(),
            ..Want::default()
        }
    }

    pub fn roles(mut self, roles: &[Role]) -> Want {
        self.roles = roles.to_vec();
        self
    }

    pub fn extensions(mut self, extensions: &[&str]) -> Want {
        self.extensions = extensions.iter().map(|e| e.to_lowercase()).collect();
        self
    }

    pub fn multiple(mut self) -> Want {
        self.multiple = true;
        self
    }

    /// Adds the words of `text` (a title, a body, tags) to what the file is for.
    pub fn about(mut self, text: &str) -> Want {
        for w in words(text) {
            if !self.context.contains(&w) {
                self.context.push(w);
            }
        }
        self
    }

    pub fn near(mut self, file: Option<PathBuf>) -> Want {
        self.near = file;
        self
    }

    pub fn exclude(mut self, files: impl IntoIterator<Item = PathBuf>) -> Want {
        self.exclude.extend(files);
        self
    }

    /// What the system file chooser should accept: `.mp3,.wav`. Empty: anything.
    pub fn accept(&self) -> String {
        self.extensions
            .iter()
            .map(|e| format!(".{e}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn takes(&self, path: &Path, role: Role) -> bool {
        let ext_ok = self.extensions.is_empty()
            || path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| self.extensions.contains(&e.to_lowercase()));
        let role_ok = self.roles.is_empty() || self.roles.contains(&role);
        ext_ok && role_ok
    }
}

/// One file the library has that fits, and why it is where it is in the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub path: PathBuf,
    pub item_id: String,
    /// The app that made it: `polishcast`, `trackcut`, `publishkit`, `library`.
    pub app: String,
    pub kind: Kind,
    pub role: Role,
    pub status: Status,
    /// What the person reads: the item's title, with the file's name when the item has
    /// several of the kind (a recording's tracks).
    pub title: String,
    /// "Clip", "Audio", with the network or tags when it has them.
    pub detail: String,
    /// The calendar day of its item.
    pub day: String,
    /// The drafts it already went into.
    pub used_in: Vec<String>,
    /// The strongest reason it was suggested: "Same recording", "Not used yet".
    pub reason: Option<&'static str>,
    pub score: i32,
}

impl Candidate {
    /// Whether `query`'s words are all in its title, detail, maker or file name.
    pub fn matches(&self, query: &str) -> bool {
        let haystack = format!(
            "{} {} {} {}",
            self.title,
            self.detail,
            self.app,
            self.path
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        )
        .to_lowercase();
        query
            .to_lowercase()
            .split_whitespace()
            .all(|w| haystack.contains(w))
    }
}

/// Every file in `items` that `want` takes, best first. Files moved away or deleted are left
/// out: a picker never offers what can't be opened.
pub fn candidates(items: &[Item], want: &Want, now: u64) -> Vec<Candidate> {
    candidates_with(items, want, now, |p| p.exists())
}

fn candidates_with(
    items: &[Item],
    want: &Want,
    now: u64,
    exists: impl Fn(&Path) -> bool,
) -> Vec<Candidate> {
    let excluded = |p: &Path| want.exclude.iter().any(|e| super::same_file(e, p));
    let near = |item: &Item| {
        want.near
            .as_deref()
            .is_some_and(|n| item.files.iter().any(|f| super::same_file(&f.path, n)))
    };
    let mut out = Vec::new();
    for item in items {
        let is_near = near(item);
        for file in &item.files {
            if !want.takes(&file.path, file.role)
                || want.near.as_deref() == Some(file.path.as_path())
                || excluded(&file.path)
                || !exists(&file.path)
            {
                continue;
            }
            let siblings = item.files.iter().filter(|f| f.role == file.role).count();
            out.push(rank(
                item, file.role, &file.path, siblings, is_near, want, now,
            ));
        }
    }
    out.sort_by(|a, b| b.score.cmp(&a.score).then(b.day.cmp(&a.day)));
    out
}

fn rank(
    item: &Item,
    role: Role,
    path: &Path,
    siblings: usize,
    near: bool,
    want: &Want,
    now: u64,
) -> Candidate {
    // Each reason with its weight; the heaviest is the one shown.
    let mut reasons: Vec<(i32, &'static str)> = Vec::new();
    if near {
        reasons.push((100, "Same recording"));
    }
    let matched = {
        let mine: Vec<String> = words(&format!("{} {}", item.title, item.tags.join(" ")));
        want.context.iter().filter(|w| mine.contains(w)).count() as i32
    };
    if matched > 0 {
        reasons.push((12 * matched.min(4), "Matches this post"));
    }
    if item.links.is_empty() && !item.status.is_out() {
        reasons.push((15, "Not used yet"));
    }
    let age = now.saturating_sub(item.updated);
    if age < 86_400 {
        reasons.push((14, "Made today"));
    } else if age < 7 * 86_400 {
        reasons.push((8, "This week"));
    }
    if item.status == Status::Ready {
        reasons.push((10, "Ready"));
    }
    if !want.here.is_empty() && item.app != want.here && item.app != "library" {
        reasons.push((5, "From another app"));
    }

    // The roles asked for first count most, without being a reason of their own.
    let role_weight = match want.roles.iter().position(|r| *r == role) {
        Some(i) => 6 * (want.roles.len() - i) as i32,
        None => 0,
    };
    let score = role_weight + reasons.iter().map(|(w, _)| w).sum::<i32>();
    let reason = reasons
        .iter()
        .max_by_key(|(w, _)| *w)
        .map(|(_, r)| *r)
        .filter(|r| *r != "From another app");

    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let title = if siblings > 1 || item.title.trim().is_empty() {
        if item.title.trim().is_empty() {
            stem.clone()
        } else {
            format!("{} · {stem}", item.title)
        }
    } else {
        item.title.clone()
    };
    let mut detail = role.label().to_string();
    if let Some(n) = &item.network {
        detail.push_str(&format!(" · {n}"));
    }
    if !item.tags.is_empty() {
        detail.push_str(&format!(" · {}", item.tags.join(", ")));
    }

    Candidate {
        path: path.to_path_buf(),
        item_id: item.id.clone(),
        app: item.app.clone(),
        kind: item.kind,
        role,
        status: item.status,
        title,
        detail,
        day: item.day().to_string(),
        used_in: item.links.iter().map(|l| l.label.clone()).collect(),
        reason,
        score,
    }
}

/// The file of `role` that belongs with `file` in the library: a video's subtitles, a
/// recording's first track. `None` when no item holds `file`, or it has none of that role.
pub fn companion(items: &[Item], file: &Path, role: Role) -> Option<PathBuf> {
    items
        .iter()
        .find(|i| i.files.iter().any(|f| super::same_file(&f.path, file)))?
        .files
        .iter()
        .find(|f| f.role == role && f.path.exists())
        .map(|f| f.path.clone())
}

/// The words that say what something is about: lowercase, three letters or more, without the
/// commonest English and French ones.
fn words(text: &str) -> Vec<String> {
    const COMMON: [&str; 24] = [
        "the", "and", "for", "with", "you", "your", "this", "that", "from", "are", "was", "how",
        "les", "des", "une", "pour", "avec", "dans", "est", "qui", "que", "sur", "par", "pas",
    ];
    let mut out: Vec<String> = Vec::new();
    for w in text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3 && !COMMON.contains(w))
    {
        if !out.iter().any(|o| o == w) {
            out.push(w.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Link;

    const NOW: u64 = 1_800_000_000;

    fn item(id: &str, app: &str, kind: Kind, title: &str) -> Item {
        let mut i = Item::new(id, app, kind, title, Status::Ready);
        i.updated = NOW - 30 * 86_400;
        i
    }

    fn pick(items: &[Item], want: &Want) -> Vec<Candidate> {
        candidates_with(items, want, NOW, |_| true)
    }

    #[test]
    fn only_what_is_asked_for_is_offered() {
        let take = item("polishcast:a", "polishcast", Kind::Video, "Demo")
            .with_file("/t/a/screen.mov", Role::Source)
            .with_file("/t/a/demo.mp4", Role::Video)
            .with_file("/t/a/demo.srt", Role::Captions);
        let songs = item("trackcut:b", "trackcut", Kind::Audio, "Live set")
            .with_file("/r/set.wav", Role::Source)
            .with_file("/r/set/01 Intro.mp3", Role::Audio)
            .with_file("/r/set/02 Main.mp3", Role::Audio);
        let want = Want::new("polishcast", "Music")
            .roles(&[Role::Audio])
            .extensions(&["mp3", "wav"]);
        let got = pick(&[take, songs], &want);
        let titles: Vec<_> = got.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles.len(), 2);
        // A recording's tracks are told apart by their names.
        assert!(titles.contains(&"Live set · 01 Intro"));
        assert!(titles.contains(&"Live set · 02 Main"));
    }

    #[test]
    fn the_same_recordings_subtitles_come_first() {
        let a = item("polishcast:a", "polishcast", Kind::Video, "Launch demo")
            .with_file("/t/a/demo.mp4", Role::Video)
            .with_file("/t/a/demo.srt", Role::Captions);
        let mut b = item("polishcast:b", "polishcast", Kind::Video, "Other")
            .with_file("/t/b/other.mp4", Role::Video)
            .with_file("/t/b/other.srt", Role::Captions);
        b.updated = NOW; // newer, but not the one the transcript is for
        let want = Want::new("publishkit", "Transcript")
            .roles(&[Role::Captions])
            .near(Some("/t/a/demo.mp4".into()));
        let got = pick(&[b, a], &want);
        assert_eq!(got[0].path, PathBuf::from("/t/a/demo.srt"));
        assert_eq!(got[0].reason, Some("Same recording"));
    }

    #[test]
    fn unused_matching_work_ranks_above_what_already_went_out() {
        let mut posted = item(
            "polishcast:a",
            "polishcast",
            Kind::Video,
            "Pricing page tour",
        )
        .with_file("/t/a/clip.mp4", Role::Clip);
        posted.status = Status::Posted;
        posted.links.push(Link {
            app: "publishkit".into(),
            label: "TikTok draft".into(),
            target: "x.md".into(),
        });
        let fresh = item(
            "polishcast:b",
            "polishcast",
            Kind::Video,
            "Pricing page tour v2",
        )
        .with_file("/t/b/clip.mp4", Role::Clip);
        let unrelated = item("polishcast:c", "polishcast", Kind::Video, "Holiday")
            .with_file("/t/c/clip.mp4", Role::Clip);
        let want = Want::new("publishkit", "Media")
            .roles(&[Role::Clip, Role::Video])
            .about("Our new pricing page");
        let got = pick(&[posted, unrelated, fresh], &want);
        assert_eq!(got[0].item_id, "polishcast:b");
        assert_eq!(got[2].item_id, "polishcast:a");
        assert_eq!(got[2].used_in, ["TikTok draft"]);
    }

    #[test]
    fn the_role_asked_for_first_wins_a_tie() {
        let v = item("polishcast:a", "polishcast", Kind::Video, "A")
            .with_file("/t/a/full.mp4", Role::Video)
            .with_file("/t/a/short.mp4", Role::Clip);
        let want = Want::new("publishkit", "TikTok").roles(&[Role::Clip, Role::Video]);
        let got = pick(&[v], &want);
        assert_eq!(got[0].role, Role::Clip);
    }

    #[test]
    fn files_already_in_place_and_missing_ones_are_left_out() {
        let v = item("library:a", "library", Kind::Image, "A")
            .with_file("/p/a.png", Role::Image)
            .with_file("/p/b.png", Role::Image);
        let want = Want::new("publishkit", "Photos")
            .roles(&[Role::Image])
            .exclude([PathBuf::from("/p/a.png")]);
        assert_eq!(pick(std::slice::from_ref(&v), &want).len(), 1);
        assert!(candidates_with(&[v], &want, NOW, |_| false).is_empty());
    }

    #[test]
    fn the_chooser_accepts_the_same_extensions() {
        let want = Want::new("x", "y").extensions(&["MP3", "wav"]);
        assert_eq!(want.accept(), ".mp3,.wav");
    }

    #[test]
    fn common_words_say_nothing_about_a_post() {
        assert_eq!(
            words("How the Pricing page works, pour vous"),
            ["pricing", "page", "works", "vous"]
        );
    }
}
