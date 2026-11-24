//! Adding the app's MCP server to the clients on this computer, taking it out
//! again, and what each one has: what the "Use it from Claude or Codex" window
//! does by itself instead of explaining.
//!
//! Claude Code and Codex are changed through their own commands (`claude mcp`,
//! `codex mcp`), which know how to write files a running session also writes.
//! Claude Desktop and VS Code have no such command, so their file is edited:
//! only this app's entry, every other byte of every other entry kept, after a
//! copy to `<file>.mayorana.bak`. A file that is not plain JSON (VS Code
//! allows comments) is left alone, rather than losing what is in it.

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;
use serde_json::{json, Value};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

/// How long a client's own command may take: `claude` needs seconds the first time.
const COMMAND: Duration = Duration::from_secs(60);
/// How long the app's server may take to answer a message when tested.
const ANSWER: Duration = Duration::from_secs(30);

/// An MCP client this can set up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Client {
    ClaudeDesktop,
    ClaudeCode,
    Codex,
    VsCode,
}

impl Client {
    pub const ALL: [Client; 4] = [
        Client::ClaudeDesktop,
        Client::ClaudeCode,
        Client::Codex,
        Client::VsCode,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Client::ClaudeDesktop => "Claude Desktop",
            Client::ClaudeCode => "Claude Code",
            Client::Codex => "Codex",
            Client::VsCode => "VS Code",
        }
    }

    /// What of it gets the server.
    pub fn about(self) -> &'static str {
        match self {
            Client::ClaudeDesktop => "the Chat side",
            Client::ClaudeCode => "terminal and Code tab",
            Client::Codex => "CLI, app and IDE extension",
            Client::VsCode => "GitHub Copilot",
        }
    }
}

/// What a client has of the app's server.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// The client is not on this computer.
    Missing,
    /// The client does not have the server.
    Absent,
    /// The client starts this executable.
    Installed,
    /// The client has a server of this name that starts something else: the
    /// app was moved, or this is another build of it.
    Elsewhere(String),
    /// Why it could not be told.
    Unreadable(String),
}

impl State {
    /// The client has a server of this name, whatever it starts.
    pub fn added(&self) -> bool {
        matches!(self, State::Installed | State::Elsewhere(_))
    }

    /// Installing would change something.
    pub fn installable(&self) -> bool {
        matches!(self, State::Absent | State::Elsewhere(_))
    }
}

/// A server of the app, running now.
#[derive(Clone, Debug, PartialEq)]
pub struct Running {
    pub pid: u32,
    /// The client that started it, when it is one of [`Client::ALL`].
    pub client: Option<Client>,
}

// What the window calls: off its thread, on this computer's real files.

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())
}

/// What every client has of server `id`, which should start `command`.
pub async fn states(id: &str, command: &str) -> Vec<(Client, State)> {
    let (id, command) = (id.to_string(), command.to_string());
    blocking(move || {
        let places = Places::here();
        Client::ALL
            .map(|c| (c, places.state(c, &id, &command)))
            .to_vec()
    })
    .await
    .unwrap_or_default()
}

/// Adds server `id`, started by `command`, or points it there.
pub async fn install(client: Client, id: &str, command: &str) -> Result<(), String> {
    let (id, command) = (id.to_string(), command.to_string());
    blocking(move || {
        if let Some(why) = unstable(&command) {
            return Err(why);
        }
        if client == Client::ClaudeDesktop && claude_desktop_open(&processes()) {
            return Err(DESKTOP_OPEN.into());
        }
        Places::here().install(client, &id, &command)
    })
    .await?
}

/// Takes server `id` out of the client. Nothing of the app itself is removed.
pub async fn uninstall(client: Client, id: &str) -> Result<(), String> {
    let id = id.to_string();
    blocking(move || {
        if client == Client::ClaudeDesktop && claude_desktop_open(&processes()) {
            return Err(DESKTOP_OPEN.into());
        }
        Places::here().uninstall(client, &id)
    })
    .await?
}

/// The app's servers running now, and whether Claude Desktop is open.
pub async fn running(command: &str) -> (Vec<Running>, bool) {
    let command = command.to_string();
    blocking(move || {
        let all = processes();
        let file = file_name(&command).to_string();
        (
            servers(&all, &file, std::process::id()),
            claude_desktop_open(&all),
        )
    })
    .await
    .unwrap_or_default()
}

/// Starts `command mcp` as a client does and lists its tools.
pub async fn test(command: &str) -> Result<Vec<String>, String> {
    let command = command.to_string();
    blocking(move || tools_of(&command)).await?
}

/// Claude Desktop keeps its configuration in memory and writes it back over a
/// change made while it is open.
const DESKTOP_OPEN: &str = "Claude is open and would undo this change. Quit it, then try again.";

/// Why a client should not be given this path: it will not be there next time.
pub fn unstable(command: &str) -> Option<String> {
    let name = crate::app().map_or("the app", |a| a.name);
    (cfg!(target_os = "macos")
        && (command.contains("/AppTranslocation/") || command.starts_with("/Volumes/")))
    .then(|| format!("Move {name} to the Applications folder and open it from there first."))
}

// Where things are.

/// Where the clients keep what they know, and how their commands are run.
struct Places {
    desktop: Option<PathBuf>,
    vscode: Option<PathBuf>,
    claude_json: Option<PathBuf>,
    claude: Option<PathBuf>,
    codex: Option<PathBuf>,
    path: OsString,
    vars: Vec<(String, String)>,
}

/// Where a client is told to keep its files elsewhere than usual.
const VARS: [&str; 2] = ["CLAUDE_CONFIG_DIR", "CODEX_HOME"];

impl Places {
    fn here() -> Self {
        // The login shell is asked once: it can take seconds.
        static SHELL: OnceLock<(OsString, Vec<(String, String)>)> = OnceLock::new();
        let (path, vars) = SHELL.get_or_init(|| (search_path(), variables())).clone();
        let home = dirs::home_dir();
        let config_dir = vars
            .iter()
            .find(|(k, _)| k == "CLAUDE_CONFIG_DIR")
            .map(|(_, v)| PathBuf::from(v));
        Places {
            desktop: super::desktop_config_file(),
            vscode: vscode_config_file(),
            claude_json: config_dir.or(home).map(|d| d.join(".claude.json")),
            claude: find("claude", &path),
            codex: find("codex", &path),
            path,
            vars,
        }
    }

    fn state(&self, client: Client, id: &str, command: &str) -> State {
        let found = match client {
            Client::ClaudeDesktop | Client::VsCode => match self.file(client) {
                Some((file, key)) => in_file(file, key, id),
                None => return State::Missing,
            },
            Client::ClaudeCode => {
                let found = match &self.claude_json {
                    Some(file) => in_file(file, "mcpServers", id),
                    None => Ok(None),
                };
                // Left in the file by a command that is gone: still worth showing.
                if self.claude.is_none() && !matches!(found, Ok(Some(_))) {
                    return State::Missing;
                }
                found
            }
            Client::Codex => match &self.codex {
                Some(codex) => self.in_codex(codex, id),
                None => return State::Missing,
            },
        };
        match found {
            Err(why) => State::Unreadable(why),
            Ok(None) => State::Absent,
            Ok(Some(entry)) => {
                let theirs = entry["command"].as_str().unwrap_or_default();
                if same_file(theirs, command) && entry["args"] == json!(["mcp"]) {
                    State::Installed
                } else {
                    State::Elsewhere(theirs.to_string())
                }
            }
        }
    }

    /// The file of a client configured through one, when the client is here,
    /// and the key its servers are under.
    fn file(&self, client: Client) -> Option<(&Path, &'static str)> {
        let (file, key) = match client {
            Client::ClaudeDesktop => (self.desktop.as_deref()?, "mcpServers"),
            Client::VsCode => (self.vscode.as_deref()?, "servers"),
            _ => return None,
        };
        file.parent()?.is_dir().then_some((file, key))
    }

    fn install(&self, client: Client, id: &str, command: &str) -> Result<(), String> {
        match client {
            Client::ClaudeDesktop | Client::VsCode => {
                let (file, key) = self.file(client).ok_or_else(|| absent(client))?;
                // VS Code wants the type, Claude Desktop does without.
                let entry = if client == Client::VsCode {
                    json!({ "type": "stdio", "command": command, "args": ["mcp"] })
                } else {
                    json!({ "command": command, "args": ["mcp"] })
                };
                edit_file(file, key, id, Some(&entry))
            }
            Client::ClaudeCode => {
                let claude = self.claude.as_deref().ok_or_else(|| absent(client))?;
                // It refuses a name it has: that one goes first, and comes back if the new one is refused.
                let old = match &self.claude_json {
                    Some(file) => in_file(file, "mcpServers", id)?,
                    None => None,
                };
                if old.is_some() {
                    self.uninstall(client, id)?;
                }
                let added = self.run(
                    claude,
                    &["mcp", "add", "--scope", "user", id, "--", command, "mcp"],
                );
                if let (Err(_), Some(old)) = (&added, old) {
                    let _ = self.run(
                        claude,
                        &["mcp", "add-json", "--scope", "user", id, &old.to_string()],
                    );
                }
                added.map(drop)
            }
            Client::Codex => {
                let codex = self.codex.as_deref().ok_or_else(|| absent(client))?;
                // It replaces a name it has.
                self.run(codex, &["mcp", "add", id, "--", command, "mcp"])
                    .map(drop)
            }
        }
    }

    fn uninstall(&self, client: Client, id: &str) -> Result<(), String> {
        match client {
            Client::ClaudeDesktop | Client::VsCode => {
                let (file, key) = self.file(client).ok_or_else(|| absent(client))?;
                edit_file(file, key, id, None)
            }
            Client::ClaudeCode => {
                let claude = self.claude.as_deref().ok_or_else(|| absent(client))?;
                self.run(claude, &["mcp", "remove", "--scope", "user", id])
                    .map(drop)
            }
            Client::Codex => {
                let codex = self.codex.as_deref().ok_or_else(|| absent(client))?;
                self.run(codex, &["mcp", "remove", id]).map(drop)
            }
        }
    }

    /// Codex's entry for `id`, as its own command gives it.
    fn in_codex(&self, codex: &Path, id: &str) -> Result<Option<Value>, String> {
        match self.run(codex, &["mcp", "get", id, "--json"]) {
            Ok(text) => serde_json::from_str::<Value>(text.trim())
                .map(|v| Some(v["transport"].clone()))
                .map_err(|e| format!("Codex's answer could not be read: {e}")),
            Err(why) if why.contains("No MCP server") => Ok(None),
            Err(why) => Err(why),
        }
    }

    /// Runs a client's own command: what it printed, or what it said went wrong.
    fn run(&self, program: &Path, args: &[&str]) -> Result<String, String> {
        let mut cmd = quiet(program);
        cmd.args(args).env("PATH", &self.path);
        for (k, v) in &self.vars {
            cmd.env(k, v);
        }
        // Not the app's folder: a project's own settings must play no part.
        if let Some(home) = dirs::home_dir() {
            cmd.current_dir(home);
        }
        output(cmd, COMMAND)
    }
}

fn absent(client: Client) -> String {
    format!("{} was not found on this computer.", client.name())
}

/// VS Code's own list of servers, for every project.
fn vscode_config_file() -> Option<PathBuf> {
    let base = if cfg!(target_os = "macos") {
        dirs::home_dir()?.join("Library/Application Support")
    } else {
        dirs::config_dir()?
    };
    Some(base.join("Code").join("User").join("mcp.json"))
}

/// Where commands are looked for: what the login shell says, what this process
/// has, then where the clients' installers put them.
fn search_path() -> OsString {
    let here = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut add = |d: PathBuf| {
        if !d.as_os_str().is_empty() && !dirs.contains(&d) {
            dirs.push(d);
        }
    };
    if let Some(login) = crate::shell::login_path() {
        std::env::split_paths(&login).for_each(&mut add);
    }
    std::env::split_paths(&here).for_each(&mut add);
    if let Some(home) = dirs::home_dir() {
        add(home.join(".local").join("bin"));
        add(home.join(".claude").join("local"));
    }
    if cfg!(unix) {
        add("/opt/homebrew/bin".into());
        add("/usr/local/bin".into());
    }
    std::env::join_paths(dirs).unwrap_or(here)
}

fn variables() -> Vec<(String, String)> {
    let login = crate::shell::login_vars(&VARS);
    VARS.iter()
        .filter_map(|k| {
            let here = std::env::var(k).ok().filter(|v| !v.is_empty());
            here.or_else(|| login.get(*k).cloned())
                .map(|v| (k.to_string(), v))
        })
        .collect()
}

/// The first `name` on `path`. On Windows npm leaves a script without an
/// extension beside the `.cmd` that runs it: only what Windows starts counts.
fn find(name: &str, path: &OsString) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        vec![format!("{name}.exe"), format!("{name}.cmd")]
    } else {
        vec![name.to_string()]
    };
    std::env::split_paths(path)
        .flat_map(|d| names.iter().map(move |n| d.join(n)).collect::<Vec<_>>())
        .find(|p| p.is_file())
}

/// The last part of a path, whichever way its slashes go.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn same_file(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

// The files.

/// A JSON object in the order it is written, its values left as they are.
/// `serde_json`'s own map would sort the person's file.
struct Object(Vec<(String, Box<RawValue>)>);

impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Object;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Object, A::Error> {
                let mut all = Vec::new();
                while let Some(pair) = map.next_entry()? {
                    all.push(pair);
                }
                Ok(Object(all))
            }
        }
        d.deserialize_map(V)
    }
}

impl Object {
    fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v.get())
    }

    /// Replaces `key` where it is, or adds it at the end.
    fn set(&mut self, key: &str, text: String) -> Result<(), String> {
        let value = RawValue::from_string(text).map_err(|e| e.to_string())?;
        match self.0.iter_mut().find(|(k, _)| k == key) {
            Some((_, v)) => *v = value,
            None => self.0.push((key.to_string(), value)),
        }
        Ok(())
    }

    /// Written with one key per line, for an object `depth` levels in.
    fn text(&self, depth: usize) -> String {
        if self.0.is_empty() {
            return "{}".into();
        }
        let pad = "  ".repeat(depth + 1);
        let lines: Vec<String> = self
            .0
            .iter()
            .map(|(k, v)| format!("{pad}{}: {}", json!(k), v.get()))
            .collect();
        format!("{{\n{}\n{}}}", lines.join(",\n"), "  ".repeat(depth))
    }
}

/// `text` with server `id` under `key` set to `entry`, or taken out.
fn edited(text: &str, key: &str, id: &str, entry: Option<&Value>) -> Result<String, String> {
    let text = text.trim_start_matches('\u{feff}');
    let mut root = if text.trim().is_empty() {
        Object(Vec::new())
    } else {
        Object::parse(text)?
    };
    let mut servers = match root.get(key) {
        None | Some("null") => Object(Vec::new()),
        Some(raw) => Object::parse(raw).map_err(|e| format!("\"{key}\": {e}"))?,
    };
    match entry {
        Some(entry) => {
            let pretty = serde_json::to_string_pretty(entry).map_err(|e| e.to_string())?;
            // A line break in JSON is never inside a string: indenting after it is safe.
            servers.set(id, pretty.replace('\n', "\n    "))?;
        }
        None => servers.0.retain(|(k, _)| k != id),
    }
    root.set(key, servers.text(1))?;
    Ok(root.text(0) + "\n")
}

fn read(file: &Path) -> Result<String, String> {
    match std::fs::read_to_string(file) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("{}: {e}", file.display())),
    }
}

fn not_plain(file: &Path, why: &str) -> String {
    format!(
        "{} is not plain JSON, so it was left alone ({why}). Add the entry by hand, below.",
        file.display()
    )
}

/// The entry of server `id` in a client's file.
fn in_file(file: &Path, key: &str, id: &str) -> Result<Option<Value>, String> {
    let text = read(file)?;
    let text = text.trim_start_matches('\u{feff}');
    if text.trim().is_empty() {
        return Ok(None);
    }
    let all: Value = serde_json::from_str(text).map_err(|e| not_plain(file, &e.to_string()))?;
    Ok(all[key].get(id).cloned())
}

fn edit_file(file: &Path, key: &str, id: &str, entry: Option<&Value>) -> Result<(), String> {
    // A link (dotfiles kept elsewhere) stays one: what it points to is written.
    let file = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    let before = read(&file)?;
    let after = edited(&before, key, id, entry).map_err(|e| not_plain(&file, &e))?;
    let failed = |e: std::io::Error| format!("{}: {e}", file.display());
    if file.exists() {
        std::fs::copy(&file, beside(&file, ".mayorana.bak")).map_err(failed)?;
    }
    // Written whole beside it, then put in its place: never half a file. A copy
    // first, so the new one keeps who may read it (entries can hold keys).
    let new = beside(&file, ".mayorana.new");
    let _ = std::fs::remove_file(&new);
    if file.exists() {
        std::fs::copy(&file, &new).map_err(failed)?;
    }
    std::fs::write(&new, after).map_err(failed)?;
    std::fs::rename(&new, &file).map_err(|e| {
        let _ = std::fs::remove_file(&new);
        failed(e)
    })
}

fn beside(file: &Path, suffix: &str) -> PathBuf {
    let mut name = file.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    file.with_file_name(name)
}

// The processes.

#[derive(Clone, Debug, PartialEq)]
struct Process {
    pid: u32,
    parent: u32,
    /// Its command line.
    line: String,
}

#[cfg(unix)]
fn processes() -> Vec<Process> {
    let mut cmd = quiet("ps");
    cmd.args(["-axww", "-o", "pid=,ppid=,args="]);
    parse_processes(&output(cmd, COMMAND).unwrap_or_default())
}

#[cfg(windows)]
fn processes() -> Vec<Process> {
    let mut cmd = quiet("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Get-CimInstance Win32_Process | ForEach-Object { \
         '{0} {1} {2}' -f $_.ProcessId, $_.ParentProcessId, $(if ($_.CommandLine) { $_.CommandLine } else { $_.Name }) }",
    ]);
    parse_processes(&output(cmd, COMMAND).unwrap_or_default())
}

/// Lines of "pid parent command line".
fn parse_processes(text: &str) -> Vec<Process> {
    text.lines()
        .filter_map(|l| {
            let (pid, rest) = l.trim().split_once(char::is_whitespace)?;
            let (parent, line) = rest.trim_start().split_once(char::is_whitespace)?;
            Some(Process {
                pid: pid.parse().ok()?,
                parent: parent.parse().ok()?,
                line: line.trim().to_string(),
            })
        })
        .collect()
}

/// The program of a command line, as far as it can be told: quoted on Windows,
/// and elsewhere not, whatever spaces it holds.
fn program(line: &str) -> &str {
    if let Some(rest) = line.strip_prefix('"') {
        return rest.split('"').next().unwrap_or(rest);
    }
    line.split(" -").next().unwrap_or(line)
}

/// The client a program belongs to.
fn client_of(line: &str) -> Option<Client> {
    let p = program(line).replace('\\', "/").to_lowercase();
    let file = file_name(&p);
    // Before Claude Desktop, whose folder holds the Claude Code of its Code tab.
    if p.contains("/claude-code/") || p.contains("/claude/versions/") || is_cli(&p) {
        Some(Client::ClaudeCode)
    } else if p.contains("/claude.app/contents/")
        || p.contains("/anthropicclaude/")
        || p.contains("/windowsapps/claude")
        || p.contains("claude-desktop")
    {
        Some(Client::ClaudeDesktop)
    } else if file.starts_with("codex") || p.contains("/codex.app/") {
        Some(Client::Codex)
    } else if p.contains("/visual studio code.app/")
        || p.contains("/microsoft vs code/")
        || p.contains("/share/code/")
        || file == "code"
        || file == "code.exe"
    {
        Some(Client::VsCode)
    } else {
        None
    }
}

/// The `claude` command, wherever it is, but for Claude Desktop's own `Claude`.
fn is_cli(p: &str) -> bool {
    let file = file_name(p).split(' ').next().unwrap_or_default();
    (file == "claude" || file == "claude.exe")
        && !p.contains("/claude.app/")
        && !p.contains("/anthropicclaude/")
        && !p.contains("/windowsapps/claude")
}

fn claude_desktop_open(all: &[Process]) -> bool {
    all.iter().any(|p| {
        let path = program(&p.line).replace('\\', "/").to_lowercase();
        client_of(&p.line) == Some(Client::ClaudeDesktop)
            && (path.ends_with("/claude.app/contents/macos/claude")
                || path.ends_with("/claude.exe")
                || file_name(&path).starts_with("claude-desktop"))
    })
}

/// The servers of the app among `all`: every `<file> mcp`, with the client that
/// started each one. Not the ones under `except`, started by this app's test.
fn servers(all: &[Process], file: &str, except: u32) -> Vec<Running> {
    all.iter()
        .filter(|p| {
            p.line
                .strip_suffix(" mcp")
                .map(|path| path.trim().trim_matches('"'))
                .is_some_and(|path| file_name(path).eq_ignore_ascii_case(file))
        })
        .filter_map(|p| {
            let mut client = None;
            let mut at = p.parent;
            // A pid can be used again: never follow parents for ever.
            for _ in 0..32 {
                if at == except {
                    return None;
                }
                let Some(parent) = all.iter().find(|q| q.pid == at) else {
                    break;
                };
                client = client_of(&parent.line);
                if client.is_some() {
                    break;
                }
                at = parent.parent;
            }
            Some(Running { pid: p.pid, client })
        })
        .collect()
}

// Commands.

/// A command that reads nothing and opens no console window on Windows.
fn quiet(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// What the command printed, or why it failed; stopped at the timeout.
fn output(mut cmd: Command, timeout: Duration) -> Result<String, String> {
    let name = file_name(&cmd.get_program().to_string_lossy()).to_string();
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{name}: {e}"))?;
    // Read while waiting, so a command that prints a lot cannot fill a pipe.
    fn reader(pipe: Option<impl Read + Send + 'static>) -> mpsc::Receiver<String> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            let _ = tx.send(String::from_utf8_lossy(&bytes).into_owned());
        });
        rx
    }
    let (out, err) = (reader(child.stdout.take()), reader(child.stderr.take()));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Err(e) => return Err(format!("{name}: {e}")),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{name} took too long."));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    // A child of its own that outlives it can hold the pipes: not waited for.
    let text =
        |rx: mpsc::Receiver<String>| rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
    let (out, err) = (text(out), text(err));
    if status.success() {
        Ok(out)
    } else {
        let said = if err.trim().is_empty() { out } else { err };
        let said = said.trim();
        Err(if said.is_empty() {
            format!("{name} failed.")
        } else {
            said.to_string()
        })
    }
}

// The test.

fn tools_of(command: &str) -> Result<Vec<String>, String> {
    let mut child = quiet(command)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{command}: {e}"))?;
    let result = talk(&mut child);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn talk(child: &mut std::process::Child) -> Result<Vec<String>, String> {
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut ask = |message: Value| -> Result<Value, String> {
        writeln!(stdin, "{message}")
            .and_then(|()| stdin.flush())
            .map_err(|e| format!("The server stopped: {e}"))?;
        if message.get("id").is_none() {
            return Ok(Value::Null);
        }
        let deadline = Instant::now() + ANSWER;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = rx.recv_timeout(left).map_err(|e| match e {
                mpsc::RecvTimeoutError::Timeout => "The server did not answer.".to_string(),
                mpsc::RecvTimeoutError::Disconnected => {
                    "The server stopped before answering.".to_string()
                }
            })?;
            let Ok(answer) = serde_json::from_str::<Value>(&line) else {
                return Err(format!("The server printed something else: {line}"));
            };
            if answer["id"] == message["id"] {
                return match answer.get("error") {
                    Some(e) => Err(e["message"].as_str().unwrap_or("error").to_string()),
                    None => Ok(answer["result"].clone()),
                };
            }
        }
    };
    ask(
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": super::PROTOCOL, "capabilities": {},
        "clientInfo": { "name": "desktop-kit", "version": env!("CARGO_PKG_VERSION") } } }),
    )?;
    ask(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))?;
    let list = ask(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }))?;
    let names: Vec<String> = list["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t["name"].as_str().map(str::to_string))
        .collect();
    if names.is_empty() {
        Err("The server answers but lists no tool.".into())
    } else {
        Ok(names)
    }
}

#[cfg(test)]
mod tests;
