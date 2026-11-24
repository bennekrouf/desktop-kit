use super::*;

/// A folder of its own for one test, gone when the test is.
struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("kit-clients-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temp(dir)
    }

    /// Every client's files under this folder, and none of their commands.
    fn places(&self) -> Places {
        Places {
            desktop: Some(self.0.join("Claude/claude_desktop_config.json")),
            vscode: Some(self.0.join("Code/User/mcp.json")),
            claude_json: Some(self.0.join(".claude.json")),
            claude: None,
            codex: None,
            path: std::env::var_os("PATH").unwrap_or_default(),
            vars: vec![
                ("CLAUDE_CONFIG_DIR".into(), self.0.display().to_string()),
                ("CODEX_HOME".into(), self.0.display().to_string()),
            ],
        }
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const CONFIG: &str = r#"{
  "zeta": 1.0,
  "mcpServers": {
    "other": {
      "command": "/bin/other",
      "env": { "KEY": "a\nb // not a comment" }
    }
  },
  "alpha": { "b": 1, "a": 12345678901234567890123 }
}
"#;

#[test]
fn adding_an_entry_keeps_every_other_byte() {
    let entry = json!({ "command": "/A B/trackcut", "args": ["mcp"] });
    let after = edited(CONFIG, "mcpServers", "trackcut", Some(&entry)).unwrap();
    let expected = CONFIG.replace(
        "    }\n  },\n  \"alpha\"",
        "    },\n    \"trackcut\": {\n      \"args\": [\n        \"mcp\"\n      ],\n      \
         \"command\": \"/A B/trackcut\"\n    }\n  },\n  \"alpha\"",
    );
    assert_eq!(after, expected);
    // And taking it out again gives back the very same file.
    assert_eq!(
        edited(&after, "mcpServers", "trackcut", None).unwrap(),
        CONFIG
    );
}

#[test]
fn an_entry_is_replaced_where_it_is() {
    let text = r#"{"mcpServers":{"a":{"command":"old"},"b":{}}}"#;
    let after = edited(text, "mcpServers", "a", Some(&json!({ "command": "new" }))).unwrap();
    let a = after.find("\"a\"").unwrap();
    assert!(a < after.find("\"b\"").unwrap(), "{after}");
    let v: Value = serde_json::from_str(&after).unwrap();
    assert_eq!(v["mcpServers"]["a"]["command"], "new");
}

#[test]
fn an_empty_file_and_one_without_servers_are_filled() {
    for text in ["", "  \n", "\u{feff}{}", r#"{"mcpServers":null}"#] {
        let after = edited(text, "mcpServers", "a", Some(&json!({ "command": "x" }))).unwrap();
        let v: Value = serde_json::from_str(&after).unwrap();
        assert_eq!(v["mcpServers"]["a"]["command"], "x", "{text:?}");
    }
}

#[test]
fn a_file_that_is_not_plain_json_is_left_alone() {
    let t = Temp::new("jsonc");
    let p = t.places();
    let file = p.vscode.clone().unwrap();
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let text = "{\n  // mine\n  \"servers\": {},\n}\n";
    std::fs::write(&file, text).unwrap();
    let why = p.install(Client::VsCode, "trackcut", "/x").unwrap_err();
    assert!(why.contains("left alone"), "{why}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    assert!(!beside(&file, ".mayorana.bak").exists());
    assert!(matches!(
        p.state(Client::VsCode, "trackcut", "/x"),
        State::Unreadable(_)
    ));
    // Servers that are not an object are not replaced by one either.
    assert!(edited(r#"{"servers":[]}"#, "servers", "a", Some(&json!({}))).is_err());
}

#[test]
fn a_client_configured_by_file_goes_from_missing_to_installed_and_back() {
    let t = Temp::new("file");
    let p = t.places();
    for client in [Client::ClaudeDesktop, Client::VsCode] {
        let (id, cmd) = ("trackcut", "/A B/trackcut");
        assert_eq!(p.state(client, id, cmd), State::Missing);
        assert!(p.install(client, id, cmd).is_err());
        let file = match client {
            Client::VsCode => p.vscode.clone().unwrap(),
            _ => p.desktop.clone().unwrap(),
        };
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        assert_eq!(p.state(client, id, cmd), State::Absent);
        p.install(client, id, cmd).unwrap();
        assert_eq!(p.state(client, id, cmd), State::Installed);
        assert_eq!(
            p.state(client, id, "/elsewhere/trackcut"),
            State::Elsewhere(cmd.into())
        );
        p.install(client, id, "/elsewhere/trackcut").unwrap();
        assert_eq!(p.state(client, id, "/elsewhere/trackcut"), State::Installed);
        p.uninstall(client, id).unwrap();
        assert_eq!(p.state(client, id, cmd), State::Absent);
        // Taking out what is not there is no failure.
        p.uninstall(client, id).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn the_file_keeps_its_permissions_and_its_link_and_a_copy_is_left() {
    use std::os::unix::fs::PermissionsExt;
    let t = Temp::new("perms");
    let p = t.places();
    let link = p.desktop.clone().unwrap();
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    let real = t.0.join("dotfiles.json");
    std::fs::write(&real, CONFIG).unwrap();
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    p.install(Client::ClaudeDesktop, "trackcut", "/x").unwrap();
    assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
    let mode = |f: &Path| std::fs::metadata(f).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&real), 0o600);
    let bak = beside(&real, ".mayorana.bak");
    assert_eq!(mode(&bak), 0o600);
    assert_eq!(std::fs::read_to_string(bak).unwrap(), CONFIG);
    assert!(!beside(&real, ".mayorana.new").exists());
}

#[test]
fn clients_without_their_command_are_missing() {
    let t = Temp::new("nocli");
    let p = t.places();
    assert_eq!(p.state(Client::ClaudeCode, "a", "/x"), State::Missing);
    assert_eq!(p.state(Client::Codex, "a", "/x"), State::Missing);
    assert!(p.install(Client::Codex, "a", "/x").is_err());
    // An entry left behind is still told.
    std::fs::write(
        t.0.join(".claude.json"),
        r#"{"mcpServers":{"a":{"command":"/x","args":["mcp"]}}}"#,
    )
    .unwrap();
    assert_eq!(p.state(Client::ClaudeCode, "a", "/x"), State::Installed);
}

/// The real `claude` and `codex`, with their settings in a folder of their own:
/// `cargo test -- --ignored`.
#[test]
#[ignore = "runs the claude and codex commands"]
fn the_real_commands_install_update_and_uninstall() {
    let t = Temp::new("real");
    let mut p = t.places();
    p.path = search_path();
    p.claude = find("claude", &p.path);
    p.codex = find("codex", &p.path);
    for client in [Client::ClaudeCode, Client::Codex] {
        let id = "kit-test";
        assert_eq!(p.state(client, id, "/A B/x"), State::Absent, "{client:?}");
        p.install(client, id, "/A B/x").unwrap();
        assert_eq!(p.state(client, id, "/A B/x"), State::Installed);
        assert_eq!(
            p.state(client, id, "/A C/x"),
            State::Elsewhere("/A B/x".into())
        );
        p.install(client, id, "/A C/x").unwrap();
        assert_eq!(p.state(client, id, "/A C/x"), State::Installed);
        p.uninstall(client, id).unwrap();
        assert_eq!(p.state(client, id, "/A C/x"), State::Absent);
    }
}

/// As `ps` printed it, on a Mac with Claude Desktop, its Code tab and Codex.
const PS: &str = "\
    1     0 /sbin/launchd
59210     1 /Applications/Claude.app/Contents/MacOS/Claude
59300 59210 /Applications/Claude.app/Contents/Helpers/disclaimer --pgroup -- /Users/me/Library/Application Support/Claude/claude-code/2.1.330/ab/claude.app/Contents/MacOS/claude
59301 59300 /Users/me/Library/Application Support/Claude/claude-code/2.1.330/ab/claude.app/Contents/MacOS/claude --output-format stream-json
59302 59301 /Applications/TrackCut.app/Contents/MacOS/trackcut mcp
59400 59300 /Applications/TrackCut.app/Contents/MacOS/trackcut mcp
60000     1 /Users/me/.local/bin/claude
60001 60000 /Applications/TrackCut.app/Contents/MacOS/trackcut mcp
61000     1 node /opt/homebrew/bin/codex
61001 61000 /Applications/TrackCut.app/Contents/MacOS/trackcut mcp
62000     1 /Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper (Plugin).app/Contents/MacOS/Code Helper (Plugin) --type=utility
62001 62000 /Applications/TrackCut.app/Contents/MacOS/trackcut mcp
63000     1 /Applications/Cursor.app/Contents/MacOS/Cursor
63001 63000 /Users/me/My Builds/trackcut mcp
64000     1 /Applications/TrackCut.app/Contents/MacOS/trackcut
64001 64000 /Applications/TrackCut.app/Contents/MacOS/trackcut mcp
65000     1 /Applications/GitAgent.app/Contents/MacOS/gitagent mcp
66000     1 claude
66001 66000 /Applications/TrackCut.app/Contents/MacOS/trackcut mcp
";

#[test]
fn running_servers_are_told_by_the_client_that_started_them() {
    let all = parse_processes(PS);
    let found: Vec<(u32, Option<Client>)> = servers(&all, "trackcut", 64000)
        .into_iter()
        .map(|r| (r.pid, r.client))
        .collect();
    assert_eq!(
        found,
        [
            (59302, Some(Client::ClaudeCode)),
            (59400, Some(Client::ClaudeDesktop)),
            (60001, Some(Client::ClaudeCode)),
            (61001, Some(Client::Codex)),
            (62001, Some(Client::VsCode)),
            (63001, None),
            // 64001 is this app's own test.
            (66001, Some(Client::ClaudeCode)),
        ]
    );
    assert_eq!(
        servers(&all, "gitagent", 1).len(),
        0,
        "under launchd as `except`"
    );
    assert_eq!(servers(&all, "gitagent", 9).len(), 1);
}

#[test]
fn claude_desktop_is_open_only_when_its_own_window_is() {
    assert!(claude_desktop_open(&parse_processes(PS)));
    let only = |line: &str| claude_desktop_open(&parse_processes(&format!("5 1 {line}")));
    assert!(!only("/Users/me/.local/bin/claude --resume"));
    assert!(!only("claude"));
    assert!(!only(
        "/Users/me/Library/Application Support/Claude/claude-code/2.1.330/ab/claude.app/Contents/MacOS/claude"
    ));
    assert!(only(
        r#""C:\Users\me\AppData\Local\AnthropicClaude\app-1.0\claude.exe" --flag"#
    ));
    assert!(only(
        r#""C:\Program Files\WindowsApps\Claude_1.0_x64__abc\app\Claude.exe""#
    ));
    assert!(!only(r#""C:\Users\me\.local\bin\claude.exe""#));
}

#[test]
fn a_windows_command_line_is_read_with_its_quotes() {
    let all = parse_processes(
        "5 1 \"C:\\Users\\me\\.local\\bin\\claude.exe\" --x\n7 5 \"C:\\Program Files\\TrackCut\\trackcut.exe\" mcp\r\n",
    );
    assert_eq!(
        servers(&all, "TrackCut.exe", 0),
        [Running {
            pid: 7,
            client: Some(Client::ClaudeCode)
        }]
    );
}

#[cfg(unix)]
#[test]
fn the_test_lists_the_tools_and_says_why_it_cannot() {
    use std::os::unix::fs::PermissionsExt;
    let t = Temp::new("test");
    let script = |name: &str, body: &str| {
        let file = t.0.join(name);
        std::fs::write(&file, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        file.display().to_string()
    };
    let good = script(
        "good",
        r#"read a; echo '{"id":1,"result":{}}'; read b; read c; echo '{"id":2,"result":{"tools":[{"name":"hello"}]}}'"#,
    );
    assert_eq!(tools_of(&good), Ok(vec!["hello".to_string()]));
    let noisy = script("noisy", "read a; echo hello there");
    assert!(tools_of(&noisy).unwrap_err().contains("hello there"));
    assert!(tools_of(&script("dead", "exit 1")).is_err());
    assert!(tools_of("/nowhere/at/all").is_err());
}

#[cfg(unix)]
#[test]
fn a_command_that_hangs_is_stopped() {
    let mut cmd = quiet("sleep");
    cmd.arg("30");
    let started = Instant::now();
    let why = output(cmd, Duration::from_millis(200)).unwrap_err();
    assert!(why.contains("too long"), "{why}");
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[cfg(target_os = "macos")]
#[test]
fn an_app_that_will_move_is_not_installed() {
    assert!(
        unstable("/private/var/folders/x/AppTranslocation/y/d/A.app/Contents/MacOS/a").is_some()
    );
    assert!(unstable("/Volumes/TrackCut/TrackCut.app/Contents/MacOS/trackcut").is_some());
    assert!(unstable("/Applications/TrackCut.app/Contents/MacOS/trackcut").is_none());
}
