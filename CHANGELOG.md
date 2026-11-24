# Changelog

What changed in desktop-kit, newest first. The apps build against its main
branch, with no tag or version in between, so a change reaches every app with
its next build. What a user will notice belongs in each app's own CHANGELOG.md,
in the pull request that takes the change; this file is the kit's history.

## [Unreleased]

### Added

- The "Use it from Claude or Codex" window installs the app's MCP server
  instead of explaining how: for Claude Desktop, Claude Code, Codex and VS Code
  it shows whether the server is installed and running, with Install, Update
  (when it starts another copy of the app) and Uninstall, and a Test that starts
  the server as a client does. The steps by hand stay, folded. It says that
  ChatGPT in the browser can't use it, as it did of Claude. `ConnectWindow`
  takes the same properties as before.
- `mcp::clients`, which does it. Claude Code and Codex through their own
  commands; Claude Desktop and VS Code by editing only the app's entry in their
  file, after a copy to `<file>.mayorana.bak`. A file that is not plain JSON is
  left alone, and Claude Desktop is never changed while it is open: it would
  write its old configuration back. serde_json's `raw_value` feature is on, so
  the rest of the file keeps its order and its text.
- The shared release workflow also builds, signs and publishes
  `installer/<app>.wxs` as `<app>.msi`, beside the setup .exe.
- Company policies in the Windows registry, under
  `HKLM\SOFTWARE\Policies\Mayorana` (every app) or `...\Mayorana\<App>`
  (one app), and the same under HKCU: `DisableUpdateCheck`,
  `DisableSelfUpdate` and `DisableTelemetry` (REG_DWORD 1), and `Licence`
  (REG_SZ), the number every user of the machine gets.

### Fixed

- Install & restart no longer builds a folder name out of the version in
  `latest.json`. A version written as a path (`9.0.0-/../../Documents`)
  passed for a newer release, and the installer then downloaded into that
  folder and deleted it when the install failed. A version holding anything
  but letters, digits, `.`, `-` and `+` is now no release at all, and the
  download folder's name is random.
- On Linux, Install & restart downloads into a folder only this user can
  open, made new each time. The temporary folder is shared by every account
  on the computer, and the old folder's name could be guessed: another
  account that made it first could swap the download after its digest was
  checked, and Linux builds carry no signature to catch that.
- The release workflow waits and tries the upload to mayorana.ch again when
  another app's release is uploading at the same moment, instead of failing:
  the server lets one upload in at a time.
- The local media server reads at most 8 KiB per request line and 100 header
  lines, and closes a connection that sends nothing for a minute. Any program
  on the computer can connect to it, and could make the app hold everything it
  sent.
- api0's licence answer is signed, and the signature is checked whenever the
  saved answer is read and whenever api0 sends one. Editing the saved file was
  enough to show Pro. An answer with a missing or wrong signature counts as no
  answer, so the app asks api0 again. Offline, Pro is counted from when api0
  signed the answer, not from the time saved beside it. Unsigned answers still
  count until 2027-02-01 (`licence::seal::UNSIGNED_UNTIL`), so nobody on Pro
  loses it before the gateway that signs is deployed.
  `licence::seal::PUBLIC_KEY` is empty until the production key pair is made.
  gateway-api0's `LICENCE_SIGNING.md` says how, and what has to ship before that
  date.

### Added

- `Answer::issued_at`, `Answer::signed` and `Answer::signature`, and
  `licence::verified`, which keeps only the part of a saved licence whose
  signature checks.
- The pipeline calendar moves items: drag one to another day, or type its date
  with ✎ or a right-click. `library::set_date` keeps the new day; posts are
  refused, as their date is in their own file. `DragGhost` draws the card
  under the pointer while it is dragged.
- `Item::plans`: the planned ideas an item was made for. `library::fulfil`
  records the idea's id there before it leaves the library, and `upsert` keeps
  it, so what pointed at the idea (a weekly goal in MiniCRM) can follow it to
  the take or post.

## [0.1.8]

### Changed

- A simpler bar across the top: the flag of the model {app} runs on, a gear
  for Settings, then MCP, Feedback and Pro. Nothing else. What {app} switches
  between, and its main buttons, sit in a slim bar under it.
- Light or dark is chosen in Settings, which also shows the installed version.

## [0.1.7]

### Added

- A "Use {app} from Claude or Codex" window: the steps to connect Claude
  Desktop, Claude Code or Codex to {app}, and what to ask it first.

## [0.1.6]

### Added

- Install & restart, on the banner that says a new version is out: one click
  downloads it, checks it, puts it in place of the version you have and opens it
  again, on macOS, Windows and Linux. No disk image to open, drag and throw
  away, no setup to click through. If an update can't finish, {app} says why the
  next time it opens. Where it can't replace itself (an app in a folder you
  can't change), the banner offers the download instead.

## [0.1.5]

### Added

- Install & restart, on the banner that says a new version is out: one click
  downloads it, checks it, puts it in place of the version you have and opens
  it again, on macOS, Windows and Linux. No disk image to open, drag and throw
  away, no setup to click through. If an update can't finish, {app} says why
  the next time it opens. Where it can't replace itself (an app in a folder you
  can't change), the banner offers the download instead.
