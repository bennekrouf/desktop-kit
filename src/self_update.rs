//! Installing a newer release in place: the update banner's "Install & restart".
//!
//! Downloads this computer's build into a folder of its own, checks it against
//! the digest published in `latest.json`, and swaps it in once the app has
//! quit. Nothing lands in Downloads and nothing is left behind: no `.dmg` to
//! eject and trash, no setup program to click through.
//!
//! * macOS: the `.dmg` is mounted, the `.app` inside copied next to the
//!   running one and its signature checked against the running one's team,
//!   all before the app quits, so a failure leaves the app as it was. A small
//!   shell script then waits for the app to exit, moves the new bundle into
//!   place and opens it.
//! * Windows: the setup's signature is checked against the running program's
//!   before the app quits. The setup then runs silently once it has, for the
//!   same scope (this user, or everyone) the app is installed for, and the app
//!   is started again.
//! * Linux: the program is taken out of the tarball and copied beside the
//!   running one. Once the app has quit it takes the old one's place, so the
//!   menu entry `setup-linux.sh` made still opens it, and is started again.
//!
//! Anything else is not installable here and the banner offers the plain
//! download: a build run from `cargo`, an app in a folder the person can't
//! write to, a release published without a digest.
//!
//! The attempt is written down before the app quits, with the reason when the
//! swap fails after it. The next launch reads it back ([`last_failure`]): an
//! app still on the old version says why, rather than looking as if nothing
//! was ever tried.
//!
//! The download goes through the system's `curl` (part of macOS, and of
//! Windows since 10 1803) rather than the app's HTTP call: that one returns
//! text, and a build of a hundred megabytes is not something to hold in a
//! `String`.

use std::fs;
use std::io;
use std::path::Path;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::update_check::UpdateInfo;

/// Where an install is.
#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    /// So many bytes are in.
    Downloading(u64),
    /// Comparing the download with the published digest and signature.
    Checking,
    /// The new version is ready; the app quits and the swap happens.
    Installing,
}

/// Whether [`install`] can put `info`'s build in place on this computer. Cheap:
/// no network, one file created and removed beside the app.
pub fn installable(info: &UpdateInfo) -> bool {
    !info.sha256.is_empty()
        && crate::browser::is_web_url(&info.download_url)
        && info
            .download_url
            .to_ascii_lowercase()
            .starts_with("https://")
        && target(&info.download_url).is_some()
}

/// Downloads, checks and prepares the new build, then starts what swaps it in
/// once this process exits. On `Ok` the caller quits the app at once. Blocking:
/// run it off the UI thread. `progress` is called from this thread.
pub fn install(info: &UpdateInfo, progress: &dyn Fn(Stage)) -> Result<(), String> {
    let target = target(&info.download_url).ok_or("this copy can't update itself")?;
    let app = crate::app().ok_or("the app isn't named")?;
    let dir = std::env::temp_dir().join(format!("{}-update-{}", app.id, info.latest_version));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("couldn't make {}: {e}", dir.display()))?;
    let file = dir.join(file_name(&info.download_url));

    let prepared = download(&info.download_url, &file, app, progress)
        .and_then(|()| {
            progress(Stage::Checking);
            check_digest(&file, &info.sha256)
        })
        .and_then(|()| target.prepare(&file, &dir));
    match prepared {
        Ok(swap) => {
            progress(Stage::Installing);
            let record = record_attempt(&info.latest_version);
            let started = swap.start(&dir, record.as_deref().unwrap_or(&dir.join("result")));
            if started.is_err() {
                if let Some(record) = record {
                    let _ = fs::remove_file(record);
                }
            }
            started
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&dir);
            Err(e)
        }
    }
}

/// Where the last attempt is written down: the version it went for, then the
/// reason it failed, if the swap got as far as knowing.
fn record_file() -> Option<PathBuf> {
    let app = crate::app()?;
    Some(
        dirs::data_local_dir()?
            .join("mayorana")
            .join("updates")
            .join(format!("{}.txt", app.id)),
    )
}

fn record_attempt(version: &str) -> Option<PathBuf> {
    let file = record_file()?;
    fs::create_dir_all(file.parent()?).ok()?;
    fs::write(&file, format!("{version}\n")).ok()?;
    Some(file)
}

/// Why the last Install & restart left this app on its old version, once:
/// the record is removed as it is read. `None` when there was none, or when
/// this launch is the version it went for.
pub fn last_failure() -> Option<String> {
    let file = record_file()?;
    let text = fs::read_to_string(&file).ok()?;
    let _ = fs::remove_file(&file);
    let current = crate::app()?.version;
    failure_in(&text, current)
}

fn failure_in(text: &str, current: &str) -> Option<String> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let wanted = lines.next()?;
    if !crate::update_check::is_newer(wanted, current) {
        return None;
    }
    Some(
        lines
            .next()
            .unwrap_or("the update didn't finish after the app quit")
            .to_string(),
    )
}

/// The last part of the address's path, without the query: `x.dmg`.
fn file_name(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or_default();
    // From a remote file: nothing that could climb out of the folder.
    if name.is_empty() || name.contains(['\\', ':']) || name.starts_with('.') {
        "update".to_string()
    } else {
        name.to_string()
    }
}

/// `curl` writing to `file`, with the size so far reported every fifth of a
/// second.
fn download(
    url: &str,
    file: &Path,
    app: &crate::App,
    progress: &dyn Fn(Stage),
) -> Result<(), String> {
    progress(Stage::Downloading(0));
    let mut child = quiet(Command::new(if cfg!(windows) {
        "curl.exe"
    } else {
        "curl"
    }))
    .args(["--fail", "--silent", "--show-error", "--location"])
    .args(["--proto", "=https", "--proto-redir", "=https"])
    .args(["--retry", "2", "--connect-timeout", "15"])
    .arg("--user-agent")
    .arg(format!("{}/{} (update)", app.id, app.version))
    .arg("--output")
    .arg(file)
    .arg("--url")
    .arg(url)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| format!("couldn't start curl: {e}"))?;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        progress(Stage::Downloading(
            fs::metadata(file).map_or(0, |m| m.len()),
        ));
        std::thread::sleep(Duration::from_millis(200));
    };
    if status.success() {
        return Ok(());
    }
    let mut err = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = io::Read::read_to_string(&mut stderr, &mut err);
    }
    let err = err.trim().trim_start_matches("curl: ").to_string();
    Err(if err.is_empty() {
        "the download failed".to_string()
    } else {
        format!("the download failed ({err})")
    })
}

fn check_digest(file: &Path, expected: &str) -> Result<(), String> {
    let got = sha256(file).map_err(|e| format!("couldn't read the download: {e}"))?;
    if got.eq_ignore_ascii_case(expected.trim()) {
        Ok(())
    } else {
        Err("the download doesn't match the published digest".to_string())
    }
}

fn sha256(file: &Path) -> io::Result<String> {
    let mut hasher = Sha256::new();
    io::copy(&mut fs::File::open(file)?, &mut hasher)?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// A command with no console window flashing up on Windows.
#[cfg_attr(not(windows), allow(unused_mut))]
fn quiet(mut cmd: Command) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[cfg_attr(target_os = "linux", allow(dead_code))]
fn extension(url: &str) -> String {
    file_name(url)
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

// ── macOS ─────────────────────────────────────────────────────────────────

/// The running `.app`, when the build for this Mac is a `.dmg` and the folder
/// holding the app is the person's to change.
#[cfg(target_os = "macos")]
fn target(url: &str) -> Option<Target> {
    if extension(url) != "dmg" {
        return None;
    }
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    // …/Name.app/Contents/MacOS/name. A binary run from `target/` is no app.
    let bundle = exe.parent()?.parent()?.parent()?;
    if bundle.extension()? != "app" || exe.parent()?.file_name()? != "MacOS" {
        return None;
    }
    let folder = bundle.parent()?;
    // The old bundle is removed by the script, as this user: one installed by
    // someone else would be left half-replaced.
    owned_by_me(bundle, folder).then(|| Target {
        bundle: bundle.to_path_buf(),
    })
}

/// Whether `path` belongs to this user and `folder` takes new files from them:
/// a file made in `folder` has the user's id, and its making proves the second.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn owned_by_me(path: &Path, folder: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let probe = folder.join(format!(".update-probe-{}", std::process::id()));
    let mine = fs::File::create(&probe)
        .and_then(|f| f.metadata())
        .map(|m| m.uid());
    let _ = fs::remove_file(&probe);
    match (mine, fs::metadata(path)) {
        (Ok(me), Ok(m)) => m.uid() == me,
        _ => false,
    }
}

#[cfg(target_os = "macos")]
struct Target {
    bundle: PathBuf,
}

#[cfg(target_os = "macos")]
struct Swap {
    /// The new bundle, copied beside the old one so the swap is two renames.
    staged: PathBuf,
    /// Where the new bundle goes: the old one's folder, under the new name.
    dest: PathBuf,
    old: PathBuf,
}

#[cfg(target_os = "macos")]
impl Target {
    /// Mounts the image, copies the app out beside the running one, checks its
    /// signature, then ejects and deletes the image.
    fn prepare(&self, dmg: &Path, dir: &Path) -> Result<Swap, String> {
        let mount = dir.join("mount");
        fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
        run(Command::new("hdiutil")
            .args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-quiet"])
            .arg("-mountpoint")
            .arg(&mount)
            .arg(dmg))
        .map_err(|e| format!("couldn't open the disk image ({e})"))?;
        let copied = self.copy_out(&mount);
        let _ = run(Command::new("hdiutil")
            .args(["detach", "-quiet", "-force"])
            .arg(&mount));
        let _ = fs::remove_file(dmg);
        copied
    }

    fn copy_out(&self, mount: &Path) -> Result<Swap, String> {
        let app = fs::read_dir(mount)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "app") && !p.is_symlink())
            .ok_or("the disk image holds no app")?;
        let name = app.file_name().ok_or("the disk image holds no app")?;
        let folder = self.bundle.parent().ok_or("the app has no folder")?;
        let dest = folder.join(name);
        let staged = folder.join(format!(".{}.update", name.to_string_lossy()));
        let _ = fs::remove_dir_all(&staged);
        // ditto keeps the signature, extended attributes and symlinks as they are.
        run(Command::new("ditto").arg(&app).arg(&staged))
            .map_err(|e| format!("couldn't copy the new version ({e})"))?;
        if let Err(e) = same_signer(&self.bundle, &staged) {
            let _ = fs::remove_dir_all(&staged);
            return Err(e);
        }
        Ok(Swap {
            staged,
            dest,
            old: self.bundle.clone(),
        })
    }
}

/// The new app's signature holds, and is the same developer's as the running
/// app's. A running app with no team (a local, ad-hoc build) has nothing to
/// compare with, and the digest is all there is.
#[cfg(target_os = "macos")]
fn same_signer(running: &Path, new: &Path) -> Result<(), String> {
    let Some(team) = team_of(running) else {
        return Ok(());
    };
    run(Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(new))
    .map_err(|_| "the new version's signature doesn't hold".to_string())?;
    if team_of(new).as_deref() == Some(team.as_str()) {
        Ok(())
    } else {
        Err("the new version is signed by someone else".to_string())
    }
}

#[cfg(target_os = "macos")]
fn team_of(app: &Path) -> Option<String> {
    let out = Command::new("codesign")
        .args(["-d", "--verbose=2"])
        .arg(app)
        .output()
        .ok()?;
    // codesign prints the details on stderr.
    String::from_utf8_lossy(&out.stderr)
        .lines()
        .find_map(|l| l.strip_prefix("TeamIdentifier="))
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "not set")
        .map(str::to_string)
}

#[cfg(target_os = "macos")]
impl Swap {
    /// Waits for this process to exit (a minute at most), puts the new bundle
    /// in place, removes the old one and the download folder, and opens the
    /// new version. Should the rename fail, the old bundle goes back.
    fn start(self, dir: &Path, record: &Path) -> Result<(), String> {
        const SCRIPT: &str = r#"
pid=$1 staged=$2 dest=$3 old=$4 dir=$5 record=$6
i=0
while kill -0 "$pid" 2>/dev/null && [ $i -lt 300 ]; do sleep 0.2; i=$((i+1)); done
aside="$dest.replaced-$$"
[ -e "$dest" ] && mv "$dest" "$aside"
if mv "$staged" "$dest"; then
  rm -rf "$aside"
  [ "$old" != "$dest" ] && rm -rf "$old"
else
  [ -e "$aside" ] && mv "$aside" "$dest"
  rm -rf "$staged"
  echo "the new version couldn't take the old one's place" >> "$record"
fi
rm -rf "$dir"
open "$dest"
"#;
        use std::os::unix::process::CommandExt;
        Command::new("/bin/sh")
            .arg("-c")
            .arg(SCRIPT)
            .arg("sh")
            .arg(std::process::id().to_string())
            .arg(&self.staged)
            .arg(&self.dest)
            .arg(&self.old)
            .arg(dir)
            .arg(record)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // Its own group, so nothing aimed at the app's reaches it.
            .process_group(0)
            .spawn()
            .map(|_| ())
            .map_err(|e| {
                let _ = fs::remove_dir_all(&self.staged);
                format!("couldn't start the install ({e})")
            })
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn run(cmd: &mut Command) -> Result<(), String> {
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() {
            out.status.to_string()
        } else {
            err
        })
    }
}

// ── Windows ───────────────────────────────────────────────────────────────

/// The running program, when it was put there by the app's setup program
/// (Inno Setup leaves `unins000.exe` beside it) and the build is that setup.
#[cfg(windows)]
fn target(url: &str) -> Option<Target> {
    if extension(url) != "exe" {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let folder = exe.parent()?;
    let from_setup = fs::read_dir(folder).ok()?.filter_map(Result::ok).any(|e| {
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        name.starts_with("unins") && name.ends_with(".exe")
    });
    from_setup.then_some(Target { exe })
}

#[cfg(windows)]
struct Target {
    exe: PathBuf,
}

#[cfg(windows)]
struct Swap {
    setup: PathBuf,
    exe: PathBuf,
}

#[cfg(windows)]
impl Target {
    fn prepare(&self, setup: &Path, _dir: &Path) -> Result<Swap, String> {
        same_signer(&self.exe, setup)?;
        Ok(Swap {
            setup: setup.to_path_buf(),
            exe: self.exe.clone(),
        })
    }

    /// Installed for everyone (under Program Files) or for this user: the
    /// setup is told the same, or it would add a second copy beside the first.
    fn scope(exe: &Path) -> &'static str {
        let everyone = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
            .iter()
            .filter_map(std::env::var_os)
            .any(|dir| exe.starts_with(dir));
        if everyone {
            "/ALLUSERS"
        } else {
            "/CURRENTUSER"
        }
    }
}

/// The setup's Authenticode signature holds and names the same publisher as
/// the running program's. Compared by subject, not thumbprint: Trusted
/// Signing issues a new certificate every few days. A running program with no
/// valid signature (a local build) has nothing to compare with, and the
/// digest is all there is.
#[cfg(windows)]
fn same_signer(running: &Path, new: &Path) -> Result<(), String> {
    let Some(publisher) = signer(running) else {
        return Ok(());
    };
    match signer(new) {
        Some(s) if s == publisher => Ok(()),
        Some(_) => Err("the new version is signed by someone else".to_string()),
        None => Err("the new version isn't signed".to_string()),
    }
}

/// The subject of `file`'s certificate, when its signature is valid.
#[cfg(windows)]
fn signer(file: &Path) -> Option<String> {
    const SCRIPT: &str = "$s = Get-AuthenticodeSignature -LiteralPath $env:KIT_FILE; \
                          if ($s.Status -eq 'Valid') { $s.SignerCertificate.Subject }";
    let out = quiet(Command::new("powershell.exe"))
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .env("KIT_FILE", file)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let subject = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!subject.is_empty()).then_some(subject)
}

#[cfg(windows)]
impl Swap {
    /// Waits for this process to exit, runs the setup with no questions (it
    /// removes the old version itself), deletes the download folder and starts
    /// the app again. The paths go through the environment, never into the
    /// script's text. A setup that doesn't start (Windows or an antivirus can
    /// stop it) or doesn't succeed is written down for the next launch.
    fn start(self, dir: &Path, record: &Path) -> Result<(), String> {
        const SCRIPT: &str = "
$ErrorActionPreference = 'SilentlyContinue'
Wait-Process -Id $env:KIT_PID -Timeout 60
$p = Start-Process -FilePath $env:KIT_SETUP -Wait -PassThru -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', $env:KIT_SCOPE)
if ($null -eq $p) {
  Add-Content -LiteralPath $env:KIT_RECORD -Value 'Windows didn''t let the setup start (an antivirus may have blocked it)'
} elseif ($p.ExitCode -ne 0) {
  Add-Content -LiteralPath $env:KIT_RECORD -Value \"the setup stopped with code $($p.ExitCode)\"
}
Remove-Item -LiteralPath $env:KIT_DIR -Recurse -Force
Start-Process -FilePath $env:KIT_APP
";
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
            ])
            .args(["-WindowStyle", "Hidden", "-Command", SCRIPT])
            .env("KIT_PID", std::process::id().to_string())
            .env("KIT_SETUP", &self.setup)
            .env("KIT_SCOPE", Target::scope(&self.exe))
            .env("KIT_DIR", dir)
            .env("KIT_APP", &self.exe)
            .env("KIT_RECORD", record)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("couldn't start the install ({e})"))
    }
}

// ── Linux ─────────────────────────────────────────────────────────────────

/// The running program, when the build is a tarball, the program isn't a
/// `cargo` build and its folder is the person's to change. `setup-linux.sh`
/// leaves the program where the tarball was extracted, so that folder usually
/// is.
#[cfg(target_os = "linux")]
fn target(url: &str) -> Option<Target> {
    let name = file_name(url).to_ascii_lowercase();
    if !(name.ends_with(".tar.gz") || name.ends_with(".tgz")) {
        return None;
    }
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let folder = exe.parent()?;
    // cargo keeps its bookkeeping beside what it builds.
    if folder.join(".fingerprint").exists() {
        return None;
    }
    let mine = owned_by_me(&exe, folder);
    mine.then_some(Target { exe })
}

#[cfg(target_os = "linux")]
struct Target {
    exe: PathBuf,
}

#[cfg(target_os = "linux")]
struct Swap {
    /// The new program, copied beside the old one so the swap is one rename.
    staged: PathBuf,
    exe: PathBuf,
}

#[cfg(target_os = "linux")]
impl Target {
    /// Unpacks the tarball, finds the program of the same name in it, and
    /// copies it beside the running one, ready to run.
    fn prepare(&self, tarball: &Path, dir: &Path) -> Result<Swap, String> {
        use std::io::Read;
        use std::os::unix::fs::PermissionsExt;

        let unpacked = dir.join("unpacked");
        fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
        run(Command::new("tar")
            .arg("-xzf")
            .arg(tarball)
            .arg("-C")
            .arg(&unpacked))
        .map_err(|e| format!("couldn't unpack the download ({e})"))?;
        let _ = fs::remove_file(tarball);

        let name = self.exe.file_name().ok_or("the program has no name")?;
        let found = find_file(&unpacked, name, 2)
            .ok_or_else(|| format!("the download holds no {}", name.to_string_lossy()))?;
        let mut magic = [0u8; 4];
        fs::File::open(&found)
            .and_then(|mut f| f.read_exact(&mut magic))
            .map_err(|e| format!("couldn't read the new version: {e}"))?;
        if &magic != b"\x7fELF" {
            return Err("the download holds no Linux program".to_string());
        }

        let folder = self.exe.parent().ok_or("the program has no folder")?;
        let staged = folder.join(format!(".{}.update", name.to_string_lossy()));
        fs::copy(&found, &staged)
            .and_then(|_| fs::set_permissions(&staged, fs::Permissions::from_mode(0o755)))
            .map_err(|e| {
                let _ = fs::remove_file(&staged);
                format!("couldn't copy the new version ({e})")
            })?;
        Ok(Swap {
            staged,
            exe: self.exe.clone(),
        })
    }
}

/// A regular file called `name` in `dir`, or in its folders `depth` deep: the
/// tarball holds one folder, with the program in it.
#[cfg(target_os = "linux")]
fn find_file(dir: &Path, name: &std::ffi::OsStr, depth: u32) -> Option<PathBuf> {
    let entries: Vec<_> = fs::read_dir(dir).ok()?.filter_map(Result::ok).collect();
    let file = entries
        .iter()
        .find(|e| e.file_name() == name && e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path());
    file.or_else(|| {
        (depth > 0).then_some(())?;
        entries
            .iter()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .find_map(|e| find_file(&e.path(), name, depth - 1))
    })
}

#[cfg(target_os = "linux")]
impl Swap {
    /// Waits for this process to exit (a minute at most), moves the new
    /// program over the old one, removes the download folder and starts it.
    fn start(self, dir: &Path, record: &Path) -> Result<(), String> {
        const SCRIPT: &str = r#"
pid=$1 staged=$2 exe=$3 dir=$4 record=$5
i=0
while kill -0 "$pid" 2>/dev/null && [ $i -lt 300 ]; do sleep 0.2; i=$((i+1)); done
if ! mv -f "$staged" "$exe"; then
  rm -f "$staged"
  echo "the new version couldn't take the old one's place" >> "$record"
fi
rm -rf "$dir"
exec "$exe"
"#;
        use std::os::unix::process::CommandExt;
        Command::new("/bin/sh")
            .arg("-c")
            .arg(SCRIPT)
            .arg("sh")
            .arg(std::process::id().to_string())
            .arg(&self.staged)
            .arg(&self.exe)
            .arg(dir)
            .arg(record)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // Its own group, so nothing aimed at the app's reaches it.
            .process_group(0)
            .spawn()
            .map(|_| ())
            .map_err(|e| {
                let _ = fs::remove_file(&self.staged);
                format!("couldn't start the install ({e})")
            })
    }
}

// ── Elsewhere ─────────────────────────────────────────────────────────────

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn target(_url: &str) -> Option<Target> {
    None
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
enum Target {}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
enum Swap {}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
impl Target {
    fn prepare(&self, _: &Path, _: &Path) -> Result<Swap, String> {
        match *self {}
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
impl Swap {
    fn start(self, _: &Path, _: &Path) -> Result<(), String> {
        match self {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_is_named_after_the_address_without_its_query() {
        assert_eq!(
            file_name("https://x/v1/gitagent-macos-arm64.dmg?src=updater"),
            "gitagent-macos-arm64.dmg"
        );
        assert_eq!(extension("https://x/v1/Setup.EXE?src=updater"), "exe");
        assert_eq!(file_name("https://x/"), "update");
        assert_eq!(file_name("https://x/..?a"), "update");
        assert_eq!(file_name("https://x/a\\..\\b.exe"), "update");
    }

    #[test]
    fn the_digest_is_checked() {
        let dir = std::env::temp_dir().join(format!("kit-digest-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("f");
        fs::write(&file, b"abc").unwrap();
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(check_digest(&file, abc), Ok(()));
        assert_eq!(check_digest(&file, &abc.to_uppercase()), Ok(()));
        assert!(check_digest(&file, &abc.replace('b', "c")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_launch_still_on_the_old_version_says_why() {
        assert_eq!(failure_in("0.2.0\n", "0.2.0"), None);
        assert_eq!(failure_in("0.2.0\n", "0.3.0"), None);
        assert_eq!(
            failure_in("0.2.0\n", "0.1.9").as_deref(),
            Some("the update didn't finish after the app quit")
        );
        assert_eq!(
            failure_in("0.2.0\nthe setup stopped with code 5\n", "0.1.9").as_deref(),
            Some("the setup stopped with code 5")
        );
        assert_eq!(failure_in("", "0.1.9"), None);
    }

    /// A test binary runs from `target/`, not from an installed app: it must
    /// never offer to replace itself.
    #[test]
    fn a_build_run_from_cargo_is_not_installable() {
        let info = UpdateInfo {
            latest_version: "9.0.0".into(),
            latest_tag: "v9.0.0".into(),
            download_url: "https://x/a.dmg?src=updater".into(),
            sha256: "ab".into(),
        };
        assert!(!installable(&info));
        let unsigned = UpdateInfo {
            sha256: String::new(),
            ..info
        };
        assert!(!installable(&unsigned));
    }
}
