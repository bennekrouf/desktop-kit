use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("desktop-kit-library-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn take(status: Status) -> Item {
    Item::new(
        "polishcast:take-1",
        "polishcast",
        Kind::Video,
        "Demo",
        status,
    )
    .with_file("/videos/demo.mp4", Role::Video)
}

fn read(dir: &Path, id: &str) -> Item {
    serde_json::from_slice(&std::fs::read(file_of(dir, id)).unwrap()).unwrap()
}

#[test]
fn an_app_moves_its_item_forward_and_keeps_what_others_added() {
    let dir = scratch("forward");
    upsert_in(&dir, take(Status::Polishing)).unwrap();
    let mut tagged = read(&dir, "polishcast:take-1");
    tagged.tags = vec!["launch".into()];
    write(&dir, &tagged).unwrap();

    let saved = upsert_in(&dir, take(Status::Ready)).unwrap();
    assert_eq!(saved.status, Status::Ready);
    assert_eq!(saved.tags, ["launch"]);
    assert_eq!(load_in(&dir).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_status_set_by_hand_stays_until_the_work_moves_on() {
    let dir = scratch("manual");
    upsert_in(&dir, take(Status::Polishing)).unwrap();
    set_status_in(&dir, "polishcast:take-1", Status::Ready).unwrap();
    // The app saves the take again, still being polished: the person's "Ready" stays.
    assert_eq!(
        upsert_in(&dir, take(Status::Polishing)).unwrap().status,
        Status::Ready
    );
    assert!(read(&dir, "polishcast:take-1").manual);

    // Set back by hand: an export that was already there doesn't undo it.
    upsert_in(&dir, take(Status::Ready)).unwrap();
    set_status_in(&dir, "polishcast:take-1", Status::Polishing).unwrap();
    assert_eq!(
        upsert_in(&dir, take(Status::Ready)).unwrap().status,
        Status::Polishing
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_video_used_in_a_draft_is_queued_and_a_new_export_keeps_it_so() {
    let dir = scratch("used");
    upsert_in(&dir, take(Status::Ready)).unwrap();
    let link = Link {
        app: "publishkit".into(),
        label: "LinkedIn draft".into(),
        target: "/site/linkedin/a.md".into(),
    };
    assert!(used_in(
        &dir,
        Path::new("/videos/demo.mp4"),
        link.clone(),
        Status::Queued
    )
    .unwrap());
    assert!(!used_in(
        &dir,
        Path::new("/videos/other.mp4"),
        link.clone(),
        Status::Queued
    )
    .unwrap());

    let saved = upsert_in(&dir, take(Status::Ready)).unwrap();
    assert_eq!(saved.status, Status::Queued);
    assert_eq!(saved.links, [link]);

    // Posted: further along, so being queued again doesn't take it back.
    let posted = Link {
        label: "LinkedIn post".into(),
        ..saved.links[0].clone()
    };
    used_in(
        &dir,
        Path::new("/videos/demo.mp4"),
        posted.clone(),
        Status::Posted,
    )
    .unwrap();
    used_in(&dir, Path::new("/videos/demo.mp4"), posted, Status::Queued).unwrap();
    assert_eq!(read(&dir, "polishcast:take-1").status, Status::Posted);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_post_takes_its_status_and_date_from_its_app() {
    let dir = scratch("post");
    let mut post = Item::new(
        "publishkit:a",
        "publishkit",
        Kind::Social,
        "Hello",
        Status::Published,
    );
    post.date = "2026-11-03T09:00".into();
    upsert_in(&dir, post.clone()).unwrap();
    post.status = Status::Queued;
    post.date = "2026-11-10T09:00".into();
    let saved = upsert_in(&dir, post).unwrap();
    assert_eq!(
        (saved.status, saved.day(), saved.time()),
        (Status::Queued, "2026-11-10", Some("09:00"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dropped_files_are_added_once_and_only_media() {
    let dir = scratch("drop");
    let files = [
        PathBuf::from("/x/Intro.mov"),
        PathBuf::from("/x/notes.txt"),
        PathBuf::from("/x/song.mp3"),
    ];
    let added = add_files_in(&dir, &files).unwrap();
    assert_eq!(
        added
            .iter()
            .map(|i| (i.kind, i.title.as_str()))
            .collect::<Vec<_>>(),
        [(Kind::Video, "Intro"), (Kind::Audio, "song")]
    );
    assert!(add_files_in(&dir, &files).unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn search_finds_every_word_anywhere() {
    let mut item = take(Status::Ready);
    item.tags = vec!["launch".into()];
    assert!(item.matches("demo LAUNCH"));
    assert!(item.matches("demo.mp4 ready"));
    assert!(!item.matches("demo audio"));
    assert_eq!(item.main_file().unwrap().role, Role::Video);
}
