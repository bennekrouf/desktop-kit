//! Opening the other Mayorana apps, and the person's files in the apps they chose for them.

use std::path::Path;
#[cfg(not(target_os = "macos"))]
use std::path::PathBuf;

/// The apps that share the media library: their key, as in a library item's `app`, and name.
pub const APPS: [(&str, &str); 3] = [
    ("polishcast", "PolishCast"),
    ("trackcut", "TrackCut"),
    ("publishkit", "PublishKit"),
];

/// An app's name, from its key: `polishcast` → `PolishCast`.
pub fn name(app: &str) -> &str {
    APPS.iter().find(|(k, _)| *k == app).map_or(app, |(_, n)| n)
}

/// Opens one of the apps, or brings it forward when it is running.
pub fn open_app(app: &str) -> Result<(), String> {
    let name = name(app);
    let missing =
        || format!("{name} isn't installed: get it at https://mayorana.ch/en/apps/{app}.");
    #[cfg(target_os = "macos")]
    {
        let ok = std::process::Command::new("open")
            .args(["-a", name])
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            return Err(missing());
        }
    }
    #[cfg(windows)]
    {
        // Where its installer puts it: {autopf}\<Name>, per user or for everyone.
        let installed = [
            dirs::data_local_dir().map(|d| d.join("Programs")),
            std::env::var_os("ProgramFiles").map(PathBuf::from),
        ];
        let exe = installed
            .into_iter()
            .flatten()
            .map(|d| d.join(name).join(format!("{app}.exe")))
            .find(|e| e.is_file());
        let Some(exe) = exe else {
            return Err(missing());
        };
        std::process::Command::new(exe)
            .spawn()
            .map_err(|e| format!("Couldn't open {name}: {e}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new(app)
            .spawn()
            .map_err(|_| missing())?;
    }
    Ok(())
}

/// Opens a file in the app the person uses for it.
pub fn open_file(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("{} isn't there any more.", path.display()));
    }
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(windows)]
    let mut cmd = {
        let mut c = std::process::Command::new("explorer");
        c.arg("/open,");
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Couldn't open {}: {e}", path.display()))
}

/// Shows a file in Finder or Explorer, selected.
pub fn reveal(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("{} isn't there any more.", path.display()));
    }
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
    #[cfg(windows)]
    let spawned = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let spawned = std::process::Command::new("xdg-open")
        .arg(path.parent().map_or(PathBuf::from("."), Path::to_path_buf))
        .spawn();
    spawned
        .map(|_| ())
        .map_err(|e| format!("Couldn't show {}: {e}", path.display()))
}
